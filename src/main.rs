mod convert;
mod download;
mod error;
mod job_store;
mod jobs;
mod pdf_ops;
mod state;
mod upload;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

pub use crate::{
    download::FileDownload,
    state::{AppState, ProgressCallback},
};
use crate::{
    error::AppResult,
    state::{read_env_u16, read_env_usize},
};

#[tokio::main]
async fn main() -> AppResult<()> {
    let state = Arc::new(AppState::from_env()?);
    let max_upload_mb = read_env_usize("MAX_UPLOAD_MB", 100);
    let port = read_env_u16("PORT", 3000);

    let app = Router::new()
        .route("/health", get(health))
        .route("/convert", post(convert::convert))
        .route("/merge", post(pdf_ops::merge))
        .route("/split", post(pdf_ops::split))
        .route("/jobs", post(jobs::create))
        .route("/jobs/{id}", get(jobs::status))
        .route("/jobs/{id}/download", get(jobs::download))
        .layer(DefaultBodyLimit::max(max_upload_mb * 1024 * 1024))
        .with_state(state)
        .fallback_service(ServeDir::new("frontend/dist"));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

#[cfg(test)]
pub(crate) use state::test_pdfium;
