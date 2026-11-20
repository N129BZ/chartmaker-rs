use std::path::PathBuf;
use tokio::sync::broadcast;

use crate::config::Configuration;
use crate::models::app_message::AppMessage;
use crate::queue::job_queue::SharedJobQueue;

#[derive(Clone)]
pub struct AppState {
    pub config: Configuration,
    pub appdir: PathBuf,
    pub queue: SharedJobQueue,
    pub tx: broadcast::Sender<(String, AppMessage)>,
    pub users: Vec<String>,
}

impl AppState {
    pub fn new(
        config: Configuration,
        appdir: PathBuf,
        queue: SharedJobQueue,
        users: Vec<String>,
    ) -> Self {
        let (tx, _rx) = broadcast::channel(100);
        Self {
            config,
            appdir,
            queue,
            tx,
            users,
        }
    }

    pub fn send_to_clients(&self, uid: &str, msg: AppMessage) {
        let _ = self.tx.send((uid.to_string(), msg));
    }
}
