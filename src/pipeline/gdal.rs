use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tokio_util::sync::CancellationToken;

use crate::models::chart::ChartTaskContext;
use crate::pipeline::command::exec_command_async;
use crate::pipeline::normalizer::build_chart_name_array;

/// Orchestrates GDAL operations:
/// 1. Per-area gdalinfo band inspection + gdal_translate to VRT
/// 2. gdalwarp reprojection to EPSG:3857 and cutline clipping
/// 3. gdalbuildvrt combining all clipped tiles into a single chart VRT
/// 4. gdal_translate to MBTILES with Lanczos resampling
/// 5. gdaladdo pyramid overview generation
/// 6. Direct SQLite metadata updates using rusqlite
pub async fn process_images(
    ctx: &ChartTaskContext,
    dbfolder: &Path,
    image_format: &str,
    tile_quality: u8,
    output_grayscale: bool,
    attribution: &str,
    cancel_token: Option<&CancellationToken>,
) -> Result<()> {
    let chartareas = build_chart_name_array(&ctx.dir_1_unzipped)?;
    let shape_folder = &ctx.clipped_shape_folder;
    let d1 = &ctx.dir_1_unzipped;
    let d2 = &ctx.dir_2_expanded;
    let d3 = &ctx.dir_3_clipped;
    let d4 = &ctx.dir_4_mbtiles;
    let cname = &ctx.chartname;

    for dir in &[d2, d3, d4] {
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
    }

    // Derive chart_category from clip folder
    let chart_category = shape_folder
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase().replace(['-', ' '], "_"))
        .unwrap_or_default();

    let cutline_geojson = shape_folder
        .parent()
        .map(|p| p.join("clipshapes.geojson"))
        .unwrap_or_else(|| shape_folder.join("clipshapes.geojson"));

    for area in &chartareas {
        println!("* Processing chart area: {}", area);
        let vrtfile = format!("{}.vrt", area);
        let srcfile = format!("{}.tif", area);
        let sourcetif = d1.join(&srcfile);
        let expanded = d2.join(&vrtfile);
        let clipped = d3.join(&vrtfile);

        // Inspect bands with gdalinfo
        let ginfo_cmd = format!("gdalinfo -json \"{}\"", sourcetif.to_string_lossy());
        let info_str = exec_command_async(&ginfo_cmd, false, cancel_token).await?;
        let info_json: Value = serde_json::from_str(&info_str).unwrap_or(Value::Null);

        let band_count = info_json["bands"].as_array().map(|b| b.len()).unwrap_or(3);
        let expandopt = if band_count == 1 {
            if output_grayscale {
                "-expand gray"
            } else {
                "-expand rgb"
            }
        } else {
            ""
        };

        let trans_cmd = format!(
            "gdal_translate -strict -of vrt -ovr NONE {} \"{}\" \"{}\"",
            expandopt,
            sourcetif.to_string_lossy(),
            expanded.to_string_lossy()
        );
        exec_command_async(&trans_cmd, false, cancel_token).await?;

        let where_clause = format!("chart_category='{}' AND chart_name='{}'", chart_category, area);
        let warp_cmd = format!(
            "gdalwarp -t_srs EPSG:3857 -dstalpha -r lanczos --config GDAL_CACHEMAX 512 \
             -multi -cutline \"{}\" -cwhere \"{}\" -crop_to_cutline \"{}\" \"{}\"",
            cutline_geojson.to_string_lossy(),
            where_clause,
            expanded.to_string_lossy(),
            clipped.to_string_lossy()
        );
        exec_command_async(&warp_cmd, false, cancel_token).await?;
    }

    let mut configargs = "--config GDAL_CACHEMAX 4096 --config GDAL_NUM_THREADS ALL_CPUS \
                          --config OGR_SQLITE_JOURNALMODE MEMORY --config OGR_SQLITE_SYNCHRONOUS OFF "
        .to_string();

    match image_format.to_lowercase().as_str() {
        "webp" => {
            configargs.push_str(&format!(
                "--config WEBP_LOSSLESS_OVERVIEW NO --config WEBP_LEVEL_OVERVIEW {} ",
                tile_quality
            ));
        }
        "jpeg" => {
            configargs.push_str(&format!("--config JPEG_QUALITY_OVERVIEW {} ", tile_quality));
        }
        _ => {}
    }

    if !dbfolder.exists() {
        fs::create_dir_all(dbfolder)?;
    }

    let imgfmt = image_format.to_uppercase();
    let mbtiles_filename = format!("{}.mbtiles", cname.to_lowercase());
    let mbtiles_temp = d4.join(&mbtiles_filename);
    let final_mbtiles = dbfolder.join(&mbtiles_filename);
    let bigvrt = d3.join(format!("{}.vrt", cname));
    let infiles = format!("{}/*.vrt", d3.to_string_lossy());

    println!(">> Building single chart VRT file: {:?}", bigvrt);
    let vrt_cmd = format!("gdalbuildvrt \"{}\" {}", bigvrt.to_string_lossy(), infiles);
    exec_command_async(&vrt_cmd, false, cancel_token).await?;

    println!(">> Creating MBTILES database file at: {:?}", mbtiles_temp);
    let mb_cmd = format!(
        "gdal_translate \"{}\" \"{}\" -of MBTILES \
         --config GDAL_CACHEMAX 4096 --config GDAL_MAX_DATASET_POOL_SIZE 500 \
         --config OGR_SQLITE_JOURNALMODE MEMORY --config OGR_SQLITE_SYNCHRONOUS OFF \
         -co TILE_FORMAT={} -co QUALITY={} -co ZOOM_LEVEL_STRATEGY=UPPER -co RESAMPLING=LANCZOS",
        bigvrt.to_string_lossy(),
        mbtiles_temp.to_string_lossy(),
        imgfmt,
        tile_quality
    );
    exec_command_async(&mb_cmd, false, cancel_token).await?;

    println!(">> Adding gdaladdo overviews to {:?}", mbtiles_temp);
    let ovr_cmd = format!("gdaladdo -r lanczos {} \"{}\"", configargs, mbtiles_temp.to_string_lossy());
    exec_command_async(&ovr_cmd, false, cancel_token).await?;

    // Update MBTILES metadata table directly using rusqlite
    println!(">> Updating MBTILES metadata in {:?}", mbtiles_temp);
    {
        let conn = Connection::open(&mbtiles_temp)
            .with_context(|| format!("Failed to open SQLite database at {:?}", mbtiles_temp))?;
        
        let meta_desc = &ctx.metadata_description;
        let _ = conn.execute(
            "UPDATE metadata SET value = ?1 WHERE name IN ('name', 'description')",
            [meta_desc],
        );
        let _ = conn.execute(
            "INSERT OR REPLACE INTO metadata (name, value) VALUES ('attribution', ?1)",
            [attribution],
        );
    }

    println!(">> Moving finished MBTILES file to: {:?}", final_mbtiles);
    fs::rename(&mbtiles_temp, &final_mbtiles)
        .or_else(|_| fs::copy(&mbtiles_temp, &final_mbtiles).map(|_| ()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&final_mbtiles, fs::Permissions::from_mode(0o666));
    }

    Ok(())
}
