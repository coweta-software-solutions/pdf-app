use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{
    convert,
    error::{AppError, AppResult},
    pdf_ops,
    upload::read_multipart,
    AppState, ProgressCallback,
};

pub async fn create(
    State(state): State<Arc<AppState>>,
    multipart: Multipart,
) -> AppResult<Response> {
    let form = read_multipart(multipart).await?;
    let action = form.required_field("action")?.to_string();
    let job_id = state.jobs.create();

    let worker_state = state.clone();
    let worker_job_id = job_id.clone();
    tokio::spawn(async move {
        worker_state.jobs.update(&worker_job_id, 10, "Queued");
        let progress_state = worker_state.clone();
        let progress_job_id = worker_job_id.clone();
        let progress: ProgressCallback = Arc::new(move |percent, stage| {
            progress_state.jobs.update(&progress_job_id, percent, stage);
        });

        let result = match action.as_str() {
            "convert" => convert::convert_form(worker_state.clone(), form, Some(progress)).await,
            "merge" => pdf_ops::merge_form(worker_state.clone(), form, Some(progress)).await,
            "split" => pdf_ops::split_form(worker_state.clone(), form, Some(progress)).await,
            _ => Err(AppError::bad_request("unknown job action")),
        };

        match result {
            Ok(file) => worker_state.jobs.complete(&worker_job_id, file),
            Err(error) => worker_state.jobs.fail(&worker_job_id, error.to_string()),
        }
    });

    Ok((StatusCode::ACCEPTED, job_id).into_response())
}

pub async fn status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let snapshot = state
        .jobs
        .snapshot(&id)
        .ok_or_else(|| AppError::bad_request("job not found"))?;

    let mut lines = vec![
        format!("status={}", line_value(&snapshot.status)),
        format!("percent={}", snapshot.percent),
        format!("stage={}", line_value(&snapshot.stage)),
    ];

    if let Some(filename) = snapshot.filename {
        lines.push(format!("filename={}", line_value(&filename)));
    }
    if let Some(content_type) = snapshot.content_type {
        lines.push(format!("content_type={}", line_value(&content_type)));
    }
    if let Some(error) = snapshot.error {
        lines.push(format!("error={}", line_value(&error)));
    }

    Ok((StatusCode::OK, lines.join("\n")).into_response())
}

pub async fn download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let snapshot = state
        .jobs
        .snapshot(&id)
        .ok_or_else(|| AppError::bad_request("job not found"))?;

    if snapshot.status != "done" {
        return Ok((StatusCode::CONFLICT, "job is not ready").into_response());
    }

    state
        .jobs
        .download(&id)
        .map(|file| file.into_response())
        .ok_or_else(|| AppError::bad_request("job result is missing"))
}

fn line_value(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}
