use anyhow::{bail, Context, Result};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

/// Execute a shell command asynchronously with Tokio subprocess,
/// logging stdout/stderr in real time and supporting clean cancellation.
pub async fn exec_command_async(
    command: &str,
    log_stdout: bool,
    cancel_token: Option<&CancellationToken>,
) -> Result<String> {
    let mut cmd = Command::new("bash");
    cmd.arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn command: {}", command))?;

    let stdout = child.stdout.take().context("Failed to open stdout pipe")?;
    let stderr = child.stderr.take().context("Failed to open stderr pipe")?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut output_buf = String::new();

    loop {
        tokio::select! {
            _ = async {
                if let Some(token) = cancel_token {
                    token.cancelled().await;
                } else {
                    futures_util::future::pending::<()>().await;
                }
            } => {
                let _ = child.kill().await;
                bail!("Command aborted by cancellation token: {}", command);
            }
            res = stdout_reader.next_line() => {
                match res {
                    Ok(Some(line)) => {
                        if log_stdout {
                            println!("{}", line);
                        }
                        output_buf.push_str(&line);
                        output_buf.push('\n');
                    }
                    Ok(None) => break,
                    Err(e) => {
                        eprintln!("Error reading stdout: {}", e);
                        break;
                    }
                }
            }
        }
    }

    // Read remaining stderr
    while let Ok(Some(line)) = stderr_reader.next_line().await {
        if log_stdout {
            eprintln!("{}", line);
        }
    }

    let status = child.wait().await?;
    if !status.success() {
        bail!(
            "Command failed with exit code {:?}: {}\nOutput: {}",
            status.code(),
            command,
            output_buf
        );
    }

    Ok(output_buf)
}
