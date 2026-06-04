use pdfium_render::prelude::Pdfium;

use crate::{
    error::{AppError, AppResult},
    pdf_io,
    progress::report,
    upload::UploadFile,
    ProgressCallback,
};

pub(crate) fn merge_pdfs(
    pdfium: &Pdfium,
    files: Vec<UploadFile>,
    progress: Option<ProgressCallback>,
) -> AppResult<Vec<u8>> {
    if files.is_empty() {
        return Err(AppError::bad_request("no documents to merge"));
    }

    let file_count = files.len();
    let mut merged = pdf_io::create_pdf(pdfium, "merged")?;

    for (index, file) in files.into_iter().enumerate() {
        let document = pdf_io::load_pdf(pdfium, file.bytes)?;
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
    pdf_io::save_pdf(&merged, "merged")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfium_render::prelude::{PdfPagePaperSize, Pdfium};

    fn pdfium_or_skip() -> Option<std::sync::Arc<Pdfium>> {
        let pdfium = crate::test_pdfium();
        if pdfium.is_none() {
            eprintln!("skipping PDFium test: PDF_TOOLS_PDFIUM_PATH is not set");
        }
        pdfium
    }

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
        let Some(pdfium) = pdfium_or_skip() else {
            return;
        };
        let bytes = merge_pdfs(
            &pdfium,
            vec![
                UploadFile {
                    filename: "one.pdf".to_string(),
                    bytes: sample_pdf_bytes(&pdfium, 1),
                },
                UploadFile {
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
    fn merge_rejects_empty_pdf_when_pdfium_is_available() {
        let Some(pdfium) = pdfium_or_skip() else {
            return;
        };

        let empty = pdfium.create_new_pdf().unwrap().save_to_bytes().unwrap();
        let error = merge_pdfs(
            &pdfium,
            vec![UploadFile {
                filename: "empty.pdf".to_string(),
                bytes: empty,
            }],
            None,
        )
        .unwrap_err();

        assert!(error.to_string().contains("no pages"));
    }
}
