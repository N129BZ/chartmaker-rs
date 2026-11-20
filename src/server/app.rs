use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_cookies::CookieManagerLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::server::handlers;
use crate::server::state::AppState;
use crate::server::worker::start_queue_worker;
use crate::server::ws::ws_handler;

pub fn build_router(state: AppState) -> Router {
    let dist_dir = state.appdir.join("dist");

    Router::new()
        // API and Queue endpoints
        .route("/api/config", get(handlers::get_config))
        .route("/status", get(handlers::get_status))
        .route("/job-state", get(handlers::get_job_state))
        .route("/clearjob", post(handlers::post_clear_job))
        .route("/existingcount", get(handlers::get_existing_count))
        .route("/sendexisting", post(handlers::post_send_existing))
        .route("/make", post(handlers::post_make))
        .route("/cancel", post(handlers::post_cancel))
        .route("/download", post(handlers::post_download))
        // WebSocket endpoint
        .route("/ws", get(ws_handler))
        // Serve static files from dist/ with fallback to index.html
        .fallback_service(
            ServeDir::new(&dist_dir).fallback(tower_http::services::ServeFile::new(
                dist_dir.join("index.html"),
            )),
        )
        .layer(CookieManagerLayer::new())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn run_server(state: AppState, port: u16) -> anyhow::Result<()> {
    // Spawn the background worker task to process queued chart generation jobs
    tokio::spawn(start_queue_worker(state.clone()));

    let app = build_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Chartmaker Web Server (Rust) listening on http://0.0.0.0:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
