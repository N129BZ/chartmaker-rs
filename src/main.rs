pub mod cli;
pub mod config;
pub mod models;
pub mod pipeline;
pub mod queue;
pub mod server;

use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::cli::menu::run_cli_loop;
use crate::config::{is_running_in_docker, load_config};
use crate::models::date::get_best_chart_date;
use crate::queue::job_queue::JobQueue;
use crate::server::app::run_server;
use crate::server::state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    let mut appdir = env::current_dir()?;
    let is_docker = is_running_in_docker();
    if is_docker {
        println!("Running in docker!");
        appdir = PathBuf::from("/chartmaker");
    }

    let (config, _) = load_config(&appdir, is_docker)?;

    let use_web_server = env::var("WEBSERVER")
        .map(|v| v == "true")
        .unwrap_or_else(|_| config.webservermode.unwrap_or(true));

    if use_web_server {
        let dates_file = appdir.join("chartdates.json");
        let best_date = get_best_chart_date(&dates_file)
            .unwrap_or_else(|_| "01-01-2025".to_string());

        let users_file = appdir.join("users.json");
        let mut users = Vec::new();
        if let Ok(content) = fs::read_to_string(&users_file) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(arr) = parsed["users"].as_array() {
                    for u in arr {
                        if let Some(s) = u.as_str() {
                            users.push(s.to_string());
                        }
                    }
                }
            }
        }

        let queue = Arc::new(Mutex::new(JobQueue::new(best_date)));
        let state = AppState::new(config.clone(), appdir, queue, users);

        run_server(state, config.httpport).await?;
    } else {
        run_cli_loop(&config, &appdir).await?;
    }

    Ok(())
}
