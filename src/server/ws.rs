use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::HeaderMap,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tower_cookies::Cookies;

use crate::models::app_message::AppMessage;
use crate::queue::job_queue::CommandItem;
use crate::server::state::AppState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
) -> impl IntoResponse {
    let client_uid = cookies
        .get("chartmaker_uid")
        .map(|c| c.value().to_string())
        .unwrap_or_else(|| {
            headers
                .get("x-user-uid")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("")
                .to_string()
        });

    ws.on_upgrade(move |socket| handle_socket(socket, state, client_uid))
}

async fn handle_socket(stream: WebSocket, state: AppState, mut client_uid: String) {
    let (mut sender, mut receiver) = stream.split();
    let mut rx = state.tx.subscribe();

    if client_uid.is_empty() {
        client_uid = format!("{}-{}", chrono::Utc::now().timestamp_millis(), std::process::id());
    }

    // Send connection confirmation
    let conn_msg = json!({
        "type": "connection",
        "uid": client_uid
    });
    let _ = sender.send(Message::Text(conn_msg.to_string())).await;

    // Send sync state if user has an existing job
    {
        let q = state.queue.lock().await;
        if let Some(user_state) = q.get_user_job_state(&client_uid) {
            let mut sync_msg = AppMessage::new(AppMessage::MT_SYNC_STATE);
            sync_msg.uid = client_uid.clone();
            sync_msg.payload = json!(user_state);
            let _ = sender
                .send(Message::Text(serde_json::to_string(&sync_msg).unwrap()))
                .await;
        }
    }

    // Spawn task to forward broadcast messages to this client
    let my_uid = client_uid.clone();
    let mut send_task = tokio::spawn(async move {
        while let Ok((target_uid, app_msg)) = rx.recv().await {
            if target_uid.is_empty() || target_uid == my_uid {
                if let Ok(text) = serde_json::to_string(&app_msg) {
                    if sender.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Receive messages from this client
    let recv_state = state.clone();
    let recv_uid = client_uid.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(val) = serde_json::from_str::<Value>(&text) {
                    let msg_type = val["type"].as_str().unwrap_or("");
                    match msg_type {
                        "clearjob" => {
                            let mut q = recv_state.queue.lock().await;
                            q.clear_user_job(&recv_uid);
                        }
                        "canceljob" => {
                            let job_id = val["jobId"].as_str();
                            let mut q = recv_state.queue.lock().await;
                            let (cancelled, _, _jid) = q.cancel_job(&recv_uid, job_id);
                            if cancelled {
                                let mut cmsg = AppMessage::new(AppMessage::MT_COMMAND_RESPONSE);
                                cmsg.uid = recv_uid.clone();
                                cmsg.payload = json!("cancelled");
                                recv_state.send_to_clients(&recv_uid, cmsg);
                            }
                        }
                        _ => {
                            if let Ok(list) =
                                serde_json::from_value::<Vec<CommandItem>>(val["commandlist"].clone())
                            {
                                if !list.is_empty() {
                                    let mut q = recv_state.queue.lock().await;
                                    q.enqueue(recv_uid.clone(), list);
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    // If either task ends, abort the other
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}
