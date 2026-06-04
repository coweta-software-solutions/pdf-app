use pdfium_render::prelude::Pdfium;

use crate::{
    error::{AppError, AppResult},
    page_selection::PageSelection,
    pdf_io,
    progress::report,
    upload::UploadFile,
    ProgressCallback,
};

pub(crate) fn split_pdf(
    pdfium: &Pdfium,
    file: UploadFile,
    selection: PageSelection,
    progress: Option<ProgressCallback>,
) -> AppResult<Vec<u8>> {
    let document = pdf_io::load_pdf(pdfium, file.bytes)?;
    let page_count = pdf_io::page_count(document.pages().len())?;
    if page_count == 0 {
        return Err(AppError::bad_request("cannot split a PDF with no pages"));
    }

    report(&progress, 60, "Extracting pages");
    let pages = pdf_io::resolve_pages(selection, page_count, "cannot split a PDF with no pages")?;
    let mut output = pdf_io::create_pdf(pdfium, "split")?;

    for page in pages {
        let destination = output.pages().len();
        output
            .pages_mut()
            .copy_page_from_document(&document, pdf_io::page_index(page)?, destination)
            .map_err(|err| AppError::Internal(format!("could not copy page {page}: {err}")))?;
    }

    report(&progress, 90, "Writing split PDF");
    pdf_io::save_pdf(&output, "split")
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
    fn extract_pages_keeps_selected_page_when_pdfium_is_available() {
        let Some(pdfium) = pdfium_or_skip() else {
            return;
        };
        let bytes = split_pdf(
            &pdfium,
            UploadFile {
                filename: "source.pdf".to_string(),
                bytes: sample_pdf_bytes(&pdfium, 2),
            },
            PageSelection::Pages(vec![2]),
            None,
        )
        .unwrap();
        let split = pdfium.load_pdf_from_byte_vec(bytes, None).unwrap();
        assert_eq!(split.pages().len(), 1);
    }

    #[test]
    fn extract_pages_accepts_all_when_pdfium_is_available() {
        let Some(pdfium) = pdfium_or_skip() else {
            return;
        };
        let bytes = split_pdf(
            &pdfium,
            UploadFile {
                filename: "source.pdf".to_string(),
                bytes: sample_pdf_bytes(&pdfium, 2),
            },
            PageSelection::All,
            None,
        )
        .unwrap();
        let split = pdfium.load_pdf_from_byte_vec(bytes, None).unwrap();
        assert_eq!(split.pages().len(), 2);
    }
}
