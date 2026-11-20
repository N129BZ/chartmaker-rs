use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Strips known suffixes (_SEC, _TAC, _FLY, _HEL, _GCO) from tif filename
pub fn normalize_file_name(file: &str) -> String {
    let mut f = file.to_string();
    let suffixes = ["_SEC", "_sec", "_TAC", "_tac", "_FLY", "_fly", "_HEL", "_hel", "_GCO", "_gco"];
    for suf in suffixes {
        f = f.replace(suf, "");
    }
    f
}

/// Normalizes all file names in dir_1_unzipped
pub fn normalize_chart_names(dir_1_unzipped: &Path) -> Result<()> {
    if !dir_1_unzipped.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir_1_unzipped)?.flatten() {
        let original_name = entry.file_name().to_string_lossy().to_string();
        let normalized = normalize_file_name(&original_name);
        if original_name != normalized {
            let old_path = entry.path();
            let new_path = dir_1_unzipped.join(&normalized);
            fs::rename(&old_path, &new_path)
                .with_context(|| format!("Failed to rename {:?} to {:?}", old_path, new_path))?;
        }
    }

    Ok(())
}

/// Discovers unique base chart area names (e.g. ["Denver", "Chicago"]) from unzipped .tif files
pub fn build_chart_name_array(dir_1_unzipped: &Path) -> Result<Vec<String>> {
    let mut names = BTreeSet::new();
    if !dir_1_unzipped.exists() {
        return Ok(Vec::new());
    }

    for entry in fs::read_dir(dir_1_unzipped)?.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.to_string_lossy().to_lowercase() == "tif" {
                    if let Some(stem) = path.file_stem() {
                        names.insert(stem.to_string_lossy().to_string());
                    }
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
        assert_eq!(normalize_file_name("Denver_SEC.tif"), "Denver.tif");
        assert_eq!(normalize_file_name("Atlanta_TAC.tif"), "Atlanta.tif");
        assert_eq!(normalize_file_name("Chicago.tif"), "Chicago.tif");
    }
}
