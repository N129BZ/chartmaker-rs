use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

fn deserialize_flexible_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match val {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        serde_json::Value::Null => Ok(String::new()),
        _ => Err(serde::de::Error::custom("expected string or number")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandItem {
    #[serde(deserialize_with = "deserialize_flexible_string", default)]
    pub command: String,
    #[serde(deserialize_with = "deserialize_flexible_string", default)]
    pub chart: String,
    #[serde(default)]
    pub rowindex: Option<i32>,
    #[serde(default)]
    pub chartname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedItem {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub rowindex: i32,
    pub filename: String,
    pub dbfilename: String,
    pub payload: String,
}

#[derive(Debug, Clone)]
pub struct MakeJob {
    pub job_id: String,
    pub uid: String,
    pub commandlist: Vec<CommandItem>,
    pub status: String,
    pub cancelled: bool,
    pub cancel_token: CancellationToken,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub completed_items: Vec<CompletedItem>,
    pub current_chart: String,
    pub current_row_index: i32,
    pub total_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserJobState {
    pub job_id: String,
    pub uid: String,
    pub status: String,
    pub position: usize,
    pub commandlist: Vec<CommandItem>,
    pub completed_items: Vec<CompletedItem>,
    pub current_chart: String,
    pub current_row_index: i32,
    pub total_time: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    pub running: bool,
    pub chartdate: String,
    pub current_chart: String,
    pub current_job_id: Option<String>,
    pub queue_length: usize,
}

pub struct JobQueue {
    pub queue: Vec<MakeJob>,
    pub current_job: Option<MakeJob>,
    pub user_jobs: HashMap<String, MakeJob>,
    pub current_chart: String,
    pub chartdate: String,
}

impl JobQueue {
    pub fn new(chartdate: String) -> Self {
        Self {
            queue: Vec::new(),
            current_job: None,
            user_jobs: HashMap::new(),
            current_chart: String::new(),
            chartdate,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.current_job.is_some()
    }

    pub fn enqueue(
        &mut self,
        uid: String,
        commandlist: Vec<CommandItem>,
    ) -> (String, usize, bool) {
        let job_id = format!("job-{}-{}", Utc::now().timestamp_millis(), &uid[..uid.len().min(6)]);
        let cancel_token = CancellationToken::new();

        let job = MakeJob {
            job_id: job_id.clone(),
            uid: uid.clone(),
            commandlist,
            status: "queued".to_string(),
            cancelled: false,
            cancel_token,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            completed_items: Vec::new(),
            current_chart: String::new(),
            current_row_index: -1,
            total_time: String::new(),
        };

        if !uid.is_empty() {
            self.user_jobs.insert(uid, job.clone());
        }

        self.queue.push(job);
        let pos = self.queue.len();
        let was_busy = self.is_busy();

        (job_id, pos, was_busy)
    }

    pub fn cancel_job(&mut self, uid: &str, job_id: Option<&str>) -> (bool, bool, String) {
        // 1. Check if in queue
        if let Some(pos) = self.queue.iter().position(|j| {
            (job_id.is_some() && Some(j.job_id.as_str()) == job_id)
                || (!uid.is_empty() && j.uid == uid)
        }) {
            let mut cancelled_job = self.queue.remove(pos);
            cancelled_job.cancelled = true;
            cancelled_job.status = "cancelled".to_string();
            let jid = cancelled_job.job_id.clone();
            if !cancelled_job.uid.is_empty() {
                self.user_jobs.remove(&cancelled_job.uid);
            }
            return (true, false, jid);
        }

        // 2. Check if currently running
        if let Some(ref mut running) = self.current_job {
            if (job_id.is_some() && Some(running.job_id.as_str()) == job_id)
                || (!uid.is_empty() && running.uid == uid)
            {
                running.cancelled = true;
                running.status = "cancelled".to_string();
                running.cancel_token.cancel();
                let jid = running.job_id.clone();
                if !running.uid.is_empty() {
                    self.user_jobs.remove(&running.uid);
                }
                return (true, true, jid);
            }
        }

        (false, false, String::new())
    }

    pub fn get_status(&self) -> QueueStatus {
        QueueStatus {
            running: self.is_busy(),
            chartdate: self.chartdate.clone(),
            current_chart: self.current_chart.clone(),
            current_job_id: self.current_job.as_ref().map(|j| j.job_id.clone()),
            queue_length: self.queue.len(),
        }
    }

    pub fn get_user_job_state(&self, uid: &str) -> Option<UserJobState> {
        let job = self.user_jobs.get(uid)?;
        let mut position = 0;
        if job.status == "queued" {
            if let Some(idx) = self.queue.iter().position(|q| q.job_id == job.job_id) {
                position = idx + 1;
            }
        }

        Some(UserJobState {
            job_id: job.job_id.clone(),
            uid: job.uid.clone(),
            status: job.status.clone(),
            position,
            commandlist: job.commandlist.clone(),
            completed_items: job.completed_items.clone(),
            current_chart: job.current_chart.clone(),
            current_row_index: job.current_row_index,
            total_time: job.total_time.clone(),
            created_at: job.created_at,
            started_at: job.started_at,
            completed_at: job.completed_at,
        })
    }

    pub fn clear_user_job(&mut self, uid: &str) {
        if let Some(job) = self.user_jobs.get(uid) {
            if job.status == "completed" || job.status == "failed" {
                self.user_jobs.remove(uid);
            }
        }
    }
}

pub type SharedJobQueue = Arc<Mutex<JobQueue>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_command_item_with_numbers() {
        let json_data = r#"{
            "command": 0,
            "chart": 1,
            "rowindex": 1,
            "chartname": "Albuquerque"
        }"#;
        let item: CommandItem = serde_json::from_str(json_data).expect("Should deserialize numbers");
        assert_eq!(item.command, "0");
        assert_eq!(item.chart, "1");
        assert_eq!(item.rowindex, Some(1));
        assert_eq!(item.chartname, "Albuquerque");
    }

    #[test]
    fn test_deserialize_command_item_with_strings() {
        let json_data = r#"{
            "command": "0",
            "chart": "1",
            "chartname": "Albuquerque"
        }"#;
        let item: CommandItem = serde_json::from_str(json_data).expect("Should deserialize strings");
        assert_eq!(item.command, "0");
        assert_eq!(item.chart, "1");
        assert_eq!(item.rowindex, None);
        assert_eq!(item.chartname, "Albuquerque");
    }

    #[test]
    fn test_deserialize_command_package_payload() {
        let json_data = r#"{
            "type": "commandpackage",
            "css": [],
            "payload": "",
            "rowindex": -1,
            "completed": false,
            "items": [],
            "commandlist": [
                {
                    "command": 0,
                    "chart": 0,
                    "rowindex": 1,
                    "chartname": "Albuquerque"
                }
            ],
            "filename": "",
            "getexisting": false,
            "dbfilename": "",
            "uid": "1732140000000-12345"
        }"#;
        let req: crate::server::handlers::MakeRequest =
            serde_json::from_str(json_data).expect("Should deserialize full frontend payload");
        let list = req.commandlist.expect("commandlist present");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].command, "0");
        assert_eq!(list[0].chart, "0");
        assert_eq!(list[0].chartname, "Albuquerque");
    }
}
