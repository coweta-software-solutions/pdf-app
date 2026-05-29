use std::sync::Arc;

use axum::{
    extract::{Multipart, State},
    response::Response,
};
use pdfium_render::prelude::{PdfDocument, PdfPageIndex, Pdfium};

use crate::{
    error::{AppError, AppResult},
    page_selection::PageSelection,
    upload::{is_pdf, read_multipart},
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
    form: crate::upload::FormData,
    progress: Option<ProgressCallback>,
) -> AppResult<FileDownload> {
    if form.files.len() < 2 {
        return Err(AppError::bad_request(
            "merge requires at least two PDF files",
        ));
    }
    for file in &form.files {
        if !is_pdf(&file.bytes) {
            return Err(AppError::bad_request(format!(
                "`{}` is not a PDF",
                file.filename
            )));
        }
    }

    report(
        &progress,
        25,
        format!("Preparing {} PDFs", form.files.len()),
    );
    let files = form.files;
    let pdfium = state.pdfium();
    let progress_for_cpu = progress.clone();
    let bytes = state
        .run_cpu(move || merge_pdf_files(&pdfium, files, progress_for_cpu))
        .await?;

    report(&progress, 95, "Preparing download");
    Ok(pdf_response("merged.pdf", bytes))
}

pub async fn split_form(
    state: Arc<AppState>,
    form: crate::upload::FormData,
    progress: Option<ProgressCallback>,
) -> AppResult<FileDownload> {
    let file = form.one_file()?;
    if !is_pdf(&file.bytes) {
        return Err(AppError::bad_request("split requires a PDF input"));
    }
    let pages = PageSelection::parse(form.required_field("pages")?)?;

    report(&progress, 30, "Reading PDF");
    let pdfium = state.pdfium();
    let progress_for_cpu = progress.clone();
    let bytes = state
        .run_cpu(move || {
            report(&progress_for_cpu, 60, "Extracting pages".to_string());
            let split = extract_pdf_pages(&pdfium, file.bytes, pages)?;
            report(&progress_for_cpu, 90, "Writing split PDF");
            Ok(split)
        })
        .await?;

    report(&progress, 95, "Preparing download");
    Ok(pdf_response("split.pdf", bytes))
}

fn report(progress: &Option<ProgressCallback>, percent: u8, stage: impl Into<String>) {
    if let Some(progress) = progress {
        progress(percent, stage.into());
    }
}

fn load_pdf_from_memory<'a>(pdfium: &'a Pdfium, bytes: Vec<u8>) -> AppResult<PdfDocument<'a>> {
    pdfium
        .load_pdf_from_byte_vec(bytes, None)
        .map_err(|err| AppError::bad_request(format!("could not read PDF: {err}")))
}

fn save_pdf(document: &PdfDocument<'_>) -> AppResult<Vec<u8>> {
    document
        .save_to_bytes()
        .map_err(|err| AppError::Internal(format!("could not write PDF: {err}")))
}

fn merge_pdf_files(
    pdfium: &Pdfium,
    files: Vec<crate::upload::UploadFile>,
    progress: Option<ProgressCallback>,
) -> AppResult<Vec<u8>> {
    if files.is_empty() {
        return Err(AppError::bad_request("no documents to merge"));
    }

    let file_count = files.len();
    let mut merged = pdfium
        .create_new_pdf()
        .map_err(|err| AppError::Internal(format!("could not create merged PDF: {err}")))?;
    for (index, file) in files.into_iter().enumerate() {
        let document = load_pdf_from_memory(pdfium, file.bytes)?;
        if document.pages().is_empty() {
            return Err(AppError::bad_request("cannot merge a PDF with no pages"));
        }
        merged.pages_mut().append(&document).map_err(|err| {
            AppError::Internal(format!("could not append `{}`: {err}", file.filename))
        })?;
        let completed = index + 1;
        report(
            &progress,
            30 + ((completed * 50) / file_count) as u8,
            format!("Merged {completed} of {file_count} PDFs"),
        );
    }

    report(&progress, 90, "Writing merged PDF");
    save_pdf(&merged)
}

fn extract_pdf_pages(
    pdfium: &Pdfium,
    bytes: Vec<u8>,
    selection: PageSelection,
) -> AppResult<Vec<u8>> {
    let document = load_pdf_from_memory(pdfium, bytes)?;
    let page_count = pdfium_page_count(document.pages().len())?;
    if page_count == 0 {
        return Err(AppError::bad_request("cannot split a PDF with no pages"));
    }
    let pages = selection.resolve(page_count)?;

    for page in &pages {
        if *page > page_count {
            return Err(AppError::bad_request(format!(
                "page {page} is out of range; PDF has {page_count} pages"
            )));
        }
    }

    let mut output = pdfium
        .create_new_pdf()
        .map_err(|err| AppError::Internal(format!("could not create split PDF: {err}")))?;
    for page in pages {
        let destination = output.pages().len();
        output
            .pages_mut()
            .copy_page_from_document(&document, page_to_index(page)?, destination)
            .map_err(|err| AppError::Internal(format!("could not copy page {page}: {err}")))?;
    }

    save_pdf(&output)
}

fn pdfium_page_count(count: PdfPageIndex) -> AppResult<usize> {
    usize::try_from(count)
        .map_err(|_| AppError::Internal(format!("invalid PDF page count: {count}")))
}

fn page_to_index(page: usize) -> AppResult<PdfPageIndex> {
    let zero_based = page
        .checked_sub(1)
        .ok_or_else(|| AppError::bad_request("page numbers start at 1"))?;
    PdfPageIndex::try_from(zero_based)
        .map_err(|_| AppError::bad_request(format!("page {page} is out of range")))
}

fn pdf_response(filename: &str, bytes: Vec<u8>) -> FileDownload {
    FileDownload::new("application/pdf", filename, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfium_render::prelude::PdfPagePaperSize;

    fn sample_pdf_bytes(pdfium: &Pdfium, page_count: usize) -> Vec<u8> {
        let mut doc = pdfium.create_new_pdf().unwrap();
        for _ in 0..page_count {
            doc.pages_mut()
                .create_page_at_end(PdfPagePaperSize::a4())
                .unwrap();
        }
        doc.save_to_bytes().unwrap()
    }

    #[test]
    fn merge_documents_preserves_page_count_when_pdfium_is_available() {
        let Some(pdfium) = crate::test_pdfium() else {
            return;
        };
        let bytes = merge_pdf_files(
            &pdfium,
            vec![
                crate::upload::UploadFile {
                    filename: "one.pdf".to_string(),
                    bytes: sample_pdf_bytes(&pdfium, 1),
                },
                crate::upload::UploadFile {
                    filename: "two.pdf".to_string(),
                    bytes: sample_pdf_bytes(&pdfium, 1),
                },
            ],
            None,
        )
        .unwrap();
        let merged = pdfium.load_pdf_from_byte_vec(bytes, None).unwrap();
        assert_eq!(merged.pages().len(), 2);
    }

    #[test]
    fn extract_pages_keeps_selected_page_when_pdfium_is_available() {
        let Some(pdfium) = crate::test_pdfium() else {
            return;
        };
        let bytes = extract_pdf_pages(
            &pdfium,
            sample_pdf_bytes(&pdfium, 2),
            PageSelection::Pages(vec![2]),
        )
        .unwrap();
        let split = pdfium.load_pdf_from_byte_vec(bytes, None).unwrap();
        assert_eq!(split.pages().len(), 1);
    }

    #[test]
    fn extract_pages_accepts_all_when_pdfium_is_available() {
        let Some(pdfium) = crate::test_pdfium() else {
            return;
        };
        let bytes =
            extract_pdf_pages(&pdfium, sample_pdf_bytes(&pdfium, 2), PageSelection::All).unwrap();
        let split = pdfium.load_pdf_from_byte_vec(bytes, None).unwrap();
        assert_eq!(split.pages().len(), 2);
    }

    #[test]
    fn page_expr_rejects_zero() {
        assert!(PageSelection::parse("0").is_err());
    }

    #[test]
    fn page_expr_expands_and_sorts_ranges() {
        assert_eq!(
            PageSelection::parse("3,1-2").unwrap(),
            PageSelection::Pages(vec![1, 2, 3])
        );
    }

    #[test]
    fn page_expr_accepts_all() {
        assert_eq!(PageSelection::parse("all").unwrap(), PageSelection::All);
    }
}
