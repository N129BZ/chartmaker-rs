use anyhow::Result;
use std::fs;
use std::path::Path;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::config::Configuration;
use crate::models::chart::ChartTaskContext;
use crate::pipeline::downloader::download_charts;
use crate::pipeline::gdal::process_images;
use crate::pipeline::normalizer::normalize_chart_names;
use crate::pipeline::unzip::unzip_charts;

pub fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let remaining_secs = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, remaining_secs)
}

/// Executes the full end-to-end processing pipeline for a single chart task
pub async fn run_processing(
    ctx: &ChartTaskContext,
    config: &Configuration,
    appdir: &Path,
    cancel_token: Option<&CancellationToken>,
) -> Result<String> {
    let start = Instant::now();
    println!("=== Starting chart pipeline for {} ===", ctx.chartname);

    let chartcache_dir = appdir.join("chartcache");
    let mbtiles_dir = appdir.join("mbtiles");

    // 1. Download
    let zip_path = download_charts(ctx, &chartcache_dir, cancel_token).await?;

    // 2. Unzip
    unzip_charts(ctx, &zip_path, cancel_token).await?;

    // 3. Normalize file names
    normalize_chart_names(&ctx.dir_1_unzipped)?;

    // 4. GDAL Transformations & MBTILES creation
    process_images(
        ctx,
        &mbtiles_dir,
        "png",
        config.tileimagequality,
        config.outputgrayscale,
        &config.attribution,
        cancel_token,
    )
    .await?;

    // 5. Clean up process folders if configured
    if config.cleanprocessfolders && ctx.chartfolder.exists() {
        println!("Cleaning up temporary folder: {:?}", ctx.chartfolder);
        let _ = fs::remove_dir_all(&ctx.chartfolder);
    }

    let elapsed = start.elapsed();
    let duration_str = format_duration(elapsed);
    println!(
        "=== Completed chart {} in {} ===",
        ctx.chartname, duration_str
    );

    Ok(duration_str)
}
