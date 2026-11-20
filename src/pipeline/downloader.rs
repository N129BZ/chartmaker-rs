use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

use crate::models::chart::ChartTaskContext;
use crate::models::date::parse_chart_date_string;
use crate::pipeline::command::exec_command_async;

/// Manages chart cache, checks for existing/outdated cycle zip files,
/// deletes older cycles, and downloads missing charts.
pub async fn download_charts(
    ctx: &ChartTaskContext,
    chartcache_dir: &Path,
    cancel_token: Option<&CancellationToken>,
) -> Result<PathBuf> {
    if !chartcache_dir.exists() {
        fs::create_dir_all(chartcache_dir).context("Failed to create chartcache dir")?;
    }

    let escaped_name = &ctx.chartworkname;
    let target_date_str = &ctx.chartdate;
    let target_timestamp = parse_chart_date_string(target_date_str)
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
        .unwrap_or(0);

    // Look for previous-cycle zip files in chartcache to clean up
    if let Ok(entries) = fs::read_dir(chartcache_dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let prefix = format!("{}-", escaped_name);
            if file_name.starts_with(&prefix) && file_name.ends_with(".zip") {
                let date_part = &file_name[prefix.len()..file_name.len() - 4];
                if let Some(cached_date) = parse_chart_date_string(date_part) {
                    let cached_timestamp = cached_date
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc()
                        .timestamp();
                    if target_timestamp > cached_timestamp {
                        println!(
                            "Purging outdated cached chart: {} (newer cycle: {})",
                            file_name, target_date_str
                        );
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    let zip_filename = format!("{}-{}.zip", escaped_name, target_date_str);
    let cached_zip_path = chartcache_dir.join(&zip_filename);

    if cached_zip_path.exists() {
        println!("Using cached chart: {:?}", cached_zip_path);
        return Ok(cached_zip_path);
    }

    println!("Downloading chart from: {}", ctx.charturl);
    println!("Destination: {:?}", cached_zip_path);

    // Download using curl via Tokio subprocess for robust progress and proxy compatibility
    let download_cmd = format!(
        "curl -fSL \"{}\" -o \"{}\"",
        ctx.charturl,
        cached_zip_path.to_string_lossy()
    );

    exec_command_async(&download_cmd, true, cancel_token)
        .await
        .with_context(|| format!("Failed to download chart from {}", ctx.charturl))?;

    Ok(cached_zip_path)
}
