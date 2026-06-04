mod convert;
mod download;
mod error;
mod image_pdf;
mod job_store;
mod jobs;
mod page_selection;
mod pdf_io;
mod pdf_merge;
mod pdf_ops;
mod pdf_render;
mod pdf_split;
mod progress;
mod state;
mod upload;

use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use tower_http::services::ServeDir;

pub use crate::{
    download::FileDownload,
    error::{AppError, AppResult},
    state::{read_env_u16, read_env_usize, AppState, ProgressCallback},
};

pub fn app(state: Arc<AppState>, max_upload_mb: usize) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/convert", post(convert::convert))
        .route("/merge", post(pdf_ops::merge))
        .route("/split", post(pdf_ops::split))
        .route("/jobs", post(jobs::create))
        .route("/jobs/{id}", get(jobs::status))
        .route("/jobs/{id}/download", get(jobs::download))
        .layer(DefaultBodyLimit::max(max_upload_mb * 1024 * 1024))
        .with_state(state)
        .fallback_service(ServeDir::new("frontend/dist"))
}

async fn health() -> &'static str {
    "ok"
}

#[cfg(test)]
pub(crate) use state::test_pdfium;
