use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Configuration {
    pub attribution: String,
    pub vfrdownloadtemplate: String,
    pub vfrindividualtemplate: String,
    pub ifrdownloadtemplate: String,
    #[serde(default)]
    pub usvfrwallplanningtemplate: Option<String>,

    #[serde(default)]
    pub renameworkarea: bool,
    #[serde(default)]
    pub logtofile: bool,
    #[serde(default = "default_true")]
    pub cleanprocessfolders: bool,

    #[serde(default = "default_quality")]
    pub tileimagequality: u8,
    #[serde(default)]
    pub outputgrayscale: bool,
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "default_timezone")]
    pub timezone: String,

    #[serde(default = "default_port")]
    pub httpport: u16,
    #[serde(default = "default_ping_interval")]
    pub pinginterval: u64,
    #[serde(default)]
    pub useauthentication: bool,
    #[serde(default = "default_zip_name")]
    pub downloadzipfilename: String,

    pub fullchartlist: Vec<[String; 3]>,
    pub areachartlist: Vec<String>,

    #[serde(default)]
    pub webservermode: Option<bool>,
}

fn default_true() -> bool { true }
fn default_quality() -> u8 { 80 }
fn default_timezone() -> String { "America/Chicago".to_string() }
fn default_port() -> u16 { 8500 }
fn default_ping_interval() -> u64 { 30000 }
fn default_zip_name() -> String { "Charts.zip".to_string() }

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ConfigWrapper {
    configuration: Configuration,
}

pub fn is_running_in_docker() -> bool {
    if let Ok(cgroup) = fs::read_to_string("/proc/1/cgroup") {
        if cgroup.contains("/docker/") {
            return true;
        }
    }
    if Path::new("/.dockerenv").exists() {
        return true;
    }
    if std::env::var("DOCKER").map(|v| v == "true").unwrap_or(false) {
        return true;
    }
    false
}

pub fn load_config(appdir: &Path, is_docker: bool) -> Result<(Configuration, PathBuf)> {
    let mut config_path = appdir.join("config.yml");
    if is_docker {
        let external_config_dir = appdir.join("config");
        let external_config_path = external_config_dir.join("config.yml");
        if !external_config_dir.exists() {
            fs::create_dir_all(&external_config_dir)
                .context("Failed to create external config dir")?;
        }
        if !external_config_path.exists() && config_path.exists() {
            let _ = fs::copy(&config_path, &external_config_path);
        }
        config_path = external_config_path;
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read configuration file at {:?}", config_path))?;
    let wrapper: ConfigWrapper = serde_yaml::from_str(&content)
        .with_context(|| format!("Invalid yaml in {:?}", config_path))?;

    Ok((wrapper.configuration, config_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_real_config() {
        let (config, _) = load_config(Path::new("."), false).expect("load config");
        assert_eq!(config.httpport, 8500);
        assert_eq!(config.fullchartlist.len(), 7);
        assert_eq!(config.areachartlist.len(), 53);
    }
}
