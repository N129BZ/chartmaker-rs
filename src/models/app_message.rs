use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub css: Vec<String>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub rowindex: i32,
    #[serde(default)]
    pub completed: bool,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub items: serde_json::Value,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub filename: String,
    #[serde(default)]
    pub getexisting: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dbfilename: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub uid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub filedate: String,
}

impl AppMessage {
    pub const MT_INFO: &'static str = "info";
    pub const MT_TIMING: &'static str = "timing";
    pub const MT_COMMAND_RESPONSE: &'static str = "commandresponse";
    pub const MT_RUNNING: &'static str = "running";
    pub const MT_COMPLETE: &'static str = "complete";
    pub const MT_COMMAND: &'static str = "command";
    pub const MT_SETTINGS: &'static str = "settings";
    pub const MT_DOWNLOAD: &'static str = "download";
    pub const MT_CONNECTION: &'static str = "connection";
    pub const MT_COMMAND_PACKAGE: &'static str = "commandpackage";
    pub const MT_EXISTING_DB: &'static str = "existingdb";
    pub const MT_SYNC_STATE: &'static str = "syncstate";

    pub fn new(msg_type: &str) -> Self {
        Self {
            msg_type: msg_type.to_string(),
            css: Vec::new(),
            payload: serde_json::Value::Null,
            rowindex: -1,
            completed: false,
            items: serde_json::Value::Null,
            filename: String::new(),
            getexisting: false,
            dbfilename: String::new(),
            uid: String::new(),
            filedate: String::new(),
        }
    }
}
