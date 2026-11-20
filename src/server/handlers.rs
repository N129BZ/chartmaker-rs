use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use tower_cookies::Cookies;

use crate::models::app_message::AppMessage;
use crate::pipeline::command::exec_command_async;
use crate::queue::job_queue::CommandItem;
use crate::server::state::AppState;

fn extract_uid(headers: &HeaderMap, cookies: &Cookies, query_uid: Option<String>) -> String {
    if let Some(uid) = query_uid {
        if !uid.is_empty() {
            return uid;
        }
    }
    if let Some(cookie) = cookies.get("chartmaker_uid") {
        return cookie.value().to_string();
    }
    if let Some(auth_hdr) = headers.get("x-user-uid") {
        if let Ok(val) = auth_hdr.to_str() {
            return val.to_string();
        }
    }
    String::new()
}

pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.config)
}

pub async fn get_status(State(state): State<AppState>) -> impl IntoResponse {
    let q = state.queue.lock().await;
    Json(q.get_status())
}

#[derive(Deserialize)]
pub struct JobStateQuery {
    pub uid: Option<String>,
}

pub async fn get_job_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    Query(query): Query<JobStateQuery>,
) -> impl IntoResponse {
    let uid = extract_uid(&headers, &cookies, query.uid);
    if uid.is_empty() {
        return Json(json!({ "job": null }));
    }
    let q = state.queue.lock().await;
    let job_state = q.get_user_job_state(&uid);
    Json(json!({ "job": job_state }))
}

#[derive(Deserialize)]
pub struct ClearJobBody {
    pub uid: Option<String>,
}

pub async fn post_clear_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    Json(body): Json<Option<ClearJobBody>>,
) -> impl IntoResponse {
    let uid = extract_uid(&headers, &cookies, body.and_then(|b| b.uid));
    if !uid.is_empty() {
        let mut q = state.queue.lock().await;
        q.clear_user_job(&uid);
    }
    Json(json!({ "success": true }))
}

pub async fn get_existing_count(State(state): State<AppState>) -> impl IntoResponse {
    let mbtiles_dir = state.appdir.join("mbtiles");
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(mbtiles_dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext != "zip" && ext != "txt" {
                    count += 1;
                }
            }
        }
    }
    Json(json!({ "existingcount": count }))
}

pub async fn post_send_existing(
    State(state): State<AppState>,
    Json(mut message): Json<Value>,
) -> impl IntoResponse {
    let mbtiles_dir = state.appdir.join("mbtiles");
    let chartdate = state.queue.lock().await.chartdate.clone();
    let mut items = Vec::new();

    if let Ok(entries) = fs::read_dir(mbtiles_dir) {
        let mut files: Vec<_> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|f| !f.ends_with(".zip") && !f.ends_with(".txt"))
            .collect();
        files.sort();

        for (idx, file) in files.iter().enumerate() {
            let stem = Path::new(file)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            items.push(json!({
                "type": AppMessage::MT_EXISTING_DB,
                "filename": stem,
                "filedate": chartdate,
                "dbfilename": file,
                "rowindex": idx,
            }));
        }
    }

    message["items"] = json!({ "existingdblist": items });
    message["completed"] = json!(true);
    Json(message)
}

#[derive(Deserialize)]
pub struct MakeRequest {
    pub uid: Option<String>,
    pub commandlist: Option<Vec<CommandItem>>,
}

pub async fn post_make(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    Json(body): Json<MakeRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let list = body.commandlist.unwrap_or_default();
    if list.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No chart commands provided in commandlist" })),
        ));
    }

    let uid = extract_uid(&headers, &cookies, body.uid);
    let (job_id, pos, was_busy) = {
        let mut q = state.queue.lock().await;
        q.enqueue(uid.clone(), list)
    };

    let status = if was_busy { "queued" } else { "accepted" };
    let message = if was_busy {
        format!("Chart processing job queued (position #{})", pos)
    } else {
        "Chart processing initiated".to_string()
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "status": status,
            "jobId": job_id,
            "position": if was_busy { pos } else { 0 },
            "message": message
        })),
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct CancelRequest {
    pub uid: Option<String>,
    pub job_id: Option<String>,
}

pub async fn post_cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    Json(body): Json<CancelRequest>,
) -> impl IntoResponse {
    let uid = extract_uid(&headers, &cookies, body.uid);
    let mut q = state.queue.lock().await;
    let (cancelled, was_running, jid) = q.cancel_job(&uid, body.job_id.as_deref());

    if cancelled {
        let mut cmsg = AppMessage::new(AppMessage::MT_COMMAND_RESPONSE);
        cmsg.uid = uid.clone();
        cmsg.payload = json!("cancelled");
        state.send_to_clients(&uid, cmsg);
    }

    Json(json!({
        "cancelled": cancelled,
        "wasRunning": was_running,
        "jobId": jid
    }))
}

#[derive(Deserialize)]
pub struct DownloadItem {
    pub dbfilename: String,
    pub rowindex: Option<i32>,
}

#[derive(Deserialize)]
pub struct DownloadRequest {
    pub uid: Option<String>,
    pub charts: Option<Vec<DownloadItem>>,
}

pub async fn post_download(
    State(state): State<AppState>,
    Json(body): Json<DownloadRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let charts = body.charts.unwrap_or_default();
    if charts.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No charts selected for download" })),
        ));
    }

    let mbtiles_dir = state.appdir.join("mbtiles");
    let workarea = state.appdir.join("workarea");
    let mut valid_files = Vec::new();

    for item in &charts {
        let path = mbtiles_dir.join(&item.dbfilename);
        if path.exists() {
            valid_files.push(path);
        }
    }

    if valid_files.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Selected chart databases not found on server" })),
        ));
    }

    let tmp_zip = workarea.join(format!(
        "charts_{}_{}.zip",
        chrono::Utc::now().timestamp_millis(),
        std::process::id()
    ));

    let files_quoted = valid_files
        .iter()
        .map(|f| format!("\"{}\"", f.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ");

    let zip_cmd = format!("zip -j -1 \"{}\" {}", tmp_zip.to_string_lossy(), files_quoted);
    if let Err(e) = exec_command_async(&zip_cmd, false, None).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create zip: {}", e) })),
        ));
    }

    let bytes = match fs::read(&tmp_zip) {
        Ok(b) => b,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to read generated zip: {}", e) })),
            ));
        }
    };
    let _ = fs::remove_file(&tmp_zip);

    let headers = [
        (header::CONTENT_TYPE, "application/zip"),
        (
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"Charts.zip\"",
        ),
    ];

    Ok((headers, bytes).into_response())
}
