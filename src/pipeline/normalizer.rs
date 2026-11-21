use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Normalizes chart filenames in parity with original make.js:
/// 1. Converts to lower-case
/// 2. Replaces spaces, dashes with underscores, removes apostrophes
/// 3. Removes _sec, _tac, _chart suffixes
/// 4. Replaces "u.s." with "us"
pub fn normalize_file_name(file: &str) -> String {
    let lower = file.to_lowercase();
    let with_underscores = lower
        .replace(' ', "_")
        .replace('\'', "")
        .replace('-', "_");
    with_underscores
        .replace("_sec", "")
        .replace("_tac", "")
        .replace("_chart", "")
        .replace("u.s.", "us")
}

/// Normalizes all file names in dir_1_unzipped:
/// - Cleans out non-geospatial files (.htm, .pdf, .zip, planning charts)
/// - Normalizes .tif, .tfw, and .tfwx file names
pub fn normalize_chart_names(dir_1_unzipped: &Path) -> Result<()> {
    if !dir_1_unzipped.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir_1_unzipped)?.flatten() {
        let path = entry.path();
        if path.is_file() {
            let original_name = entry.file_name().to_string_lossy().to_string();
            let lower_name = original_name.to_lowercase();

            // Clean out unwanted ancillary files matching make.js cleanup logic
            if lower_name.ends_with(".htm")
                || lower_name.ends_with(".pdf")
                || lower_name.ends_with(".zip")
                || lower_name == "caribbean planning chart.tif"
                || lower_name == "caribbean_planning_chart.tif"
            {
                let _ = fs::remove_file(&path);
                continue;
            }

            if lower_name.ends_with(".tif")
                || lower_name.ends_with(".tfw")
                || lower_name.ends_with(".tfwx")
            {
                let normalized = normalize_file_name(&original_name);
                if original_name != normalized {
                    let new_path = dir_1_unzipped.join(&normalized);
                    fs::rename(&path, &new_path).with_context(|| {
                        format!("Failed to rename {:?} to {:?}", path, new_path)
                    })?;
                }
            }
        }
    }

    Ok(())
}

/// Discovers unique base chart area names (e.g. ["denver", "chicago"]) from unzipped .tif files
pub fn build_chart_name_array(dir_1_unzipped: &Path) -> Result<Vec<String>> {
    let mut names = BTreeSet::new();
    if !dir_1_unzipped.exists() {
        return Ok(Vec::new());
    }

    for entry in fs::read_dir(dir_1_unzipped)?.flatten() {
        let path = entry.path();
        if path.is_file() {
            let fname = entry.file_name().to_string_lossy().to_string();
            let lower = fname.to_lowercase();
            if lower.ends_with(".tif")
                && !lower.contains("fly")
                && !lower.contains("new_york_vfr_plannings_")
                && !lower.contains("caribbean_planning")
            {
                if let Some(stem) = path.file_stem() {
                    let stem_str = stem.to_string_lossy().to_lowercase();
                    names.insert(stem_str);
                }
            }
        }
    }

    Ok(names.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_filename() {
        assert_eq!(normalize_file_name("Brownsville SEC.tif"), "brownsville.tif");
        assert_eq!(normalize_file_name("Brownsville SEC.tfw"), "brownsville.tfw");
        assert_eq!(normalize_file_name("Atlanta TAC.tif"), "atlanta.tif");
        assert_eq!(normalize_file_name("Denver_SEC.tif"), "denver.tif");
        assert_eq!(normalize_file_name("Chicago.tif"), "chicago.tif");
        assert_eq!(normalize_file_name("New York SEC.tif"), "new_york.tif");
        assert_eq!(
            normalize_file_name("Grand Canyon Air Tour Operators.tif"),
            "grand_canyon_air_tour_operators.tif"
        );
        assert_eq!(normalize_file_name("ENR_H01.tif"), "enr_h01.tif");
        assert_eq!(normalize_file_name("ENR_L01.tif"), "enr_l01.tif");
        assert_eq!(normalize_file_name("U.S. Wall Plan.tif"), "us_wall_plan.tif");
        assert_eq!(normalize_file_name("Cape Lisburne SEC.tif"), "cape_lisburne.tif");
        assert_eq!(normalize_file_name("Klamath Falls SEC.tif"), "klamath_falls.tif");
    }

    #[test]
    fn test_normalize_chart_names_directory() {
        let temp_dir = std::env::temp_dir().join(format!("chart_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        fs::write(temp_dir.join("Brownsville SEC.tif"), b"tif data").unwrap();
        fs::write(temp_dir.join("Brownsville SEC.tfw"), b"tfw data").unwrap();
        fs::write(temp_dir.join("Brownsville SEC.htm"), b"htm data").unwrap();

        normalize_chart_names(&temp_dir).unwrap();

        assert!(temp_dir.join("brownsville.tif").exists());
        assert!(temp_dir.join("brownsville.tfw").exists());
        assert!(!temp_dir.join("Brownsville SEC.htm").exists());
        assert!(!temp_dir.join("Brownsville SEC.tif").exists());

        let names = build_chart_name_array(&temp_dir).unwrap();
        assert_eq!(names, vec!["brownsville".to_string()]);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
