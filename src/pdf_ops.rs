use std::sync::Arc;

use axum::{
    extract::{Multipart, State},
    response::Response,
};

use crate::{
    error::AppResult,
    page_selection::PageSelection,
    pdf_merge, pdf_split,
    progress::report,
    upload::{read_multipart, require_at_least_pdfs, require_one_pdf, FormData},
    AppState, FileDownload, ProgressCallback,
};

pub async fn merge(
    State(state): State<Arc<AppState>>,
    multipart: Multipart,
) -> AppResult<Response> {
    let form = read_multipart(multipart).await?;
    Ok(merge_form(state, form, None).await?.into_response())
}

pub async fn split(
    State(state): State<Arc<AppState>>,
    multipart: Multipart,
) -> AppResult<Response> {
    let form = read_multipart(multipart).await?;
    Ok(split_form(state, form, None).await?.into_response())
}

pub async fn merge_form(
    state: Arc<AppState>,
    form: FormData,
    progress: Option<ProgressCallback>,
) -> AppResult<FileDownload> {
    require_at_least_pdfs(&form.files, 2, "merge requires at least two PDF files")?;

    report(
        &progress,
        25,
        format!("Preparing {} PDFs", form.files.len()),
    );
    let files = form.files;
    let pdfium = state.pdfium();
    let progress_for_cpu = progress.clone();
    let bytes = state
        .run_cpu(move || pdf_merge::merge_pdfs(&pdfium, files, progress_for_cpu))
        .await?;

    report(&progress, 95, "Preparing download");
    Ok(pdf_response("merged.pdf", bytes))
}

pub async fn split_form(
    state: Arc<AppState>,
    form: FormData,
    progress: Option<ProgressCallback>,
) -> AppResult<FileDownload> {
    let pages = PageSelection::parse(form.required_field("pages")?)?;
    let file = require_one_pdf(form, "split requires a PDF input")?;

    report(&progress, 30, "Reading PDF");
    let pdfium = state.pdfium();
    let progress_for_cpu = progress.clone();
    let bytes = state
        .run_cpu(move || pdf_split::split_pdf(&pdfium, file, pages, progress_for_cpu))
        .await?;

    report(&progress, 95, "Preparing download");
    Ok(pdf_response("split.pdf", bytes))
}

fn pdf_response(filename: &str, bytes: Vec<u8>) -> FileDownload {
    FileDownload::new("application/pdf", filename, bytes)
}
