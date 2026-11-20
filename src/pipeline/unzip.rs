use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tokio_util::sync::CancellationToken;

use crate::models::chart::ChartTaskContext;
use crate::pipeline::command::exec_command_async;

/// Unzips chart archive into ctx.dir_1_unzipped.
/// If IFR, extracts only ENR_H or ENR_L tifs/tfws.
pub async fn unzip_charts(
    ctx: &ChartTaskContext,
    zip_path: &Path,
    cancel_token: Option<&CancellationToken>,
) -> Result<()> {
    if !ctx.dir_1_unzipped.exists() {
        fs::create_dir_all(&ctx.dir_1_unzipped).context("Failed to create unzipped directory")?;
    }

    let dest = ctx.dir_1_unzipped.to_string_lossy();
    let zip_str = zip_path.to_string_lossy();

    let cmd = if ctx.is_ifr {
        let srchstr = if ctx.chartname == "Enroute_High" {
            "ENR_H"
        } else {
            "ENR_L"
        };
        format!(
            "unzip -q -o -j \"{}\" \"*{}*.tif\" \"*{}*.tfw\" -d \"{}\"",
            zip_str, srchstr, srchstr, dest
        )
    } else {
        format!("unzip -q -o -j \"{}\" -d \"{}\"", zip_str, dest)
    };

    println!(">> Unzipping: {}", cmd);
    exec_command_async(&cmd, false, cancel_token)
        .await
        .with_context(|| format!("Failed to unzip {:?}", zip_path))?;

    Ok(())
}
