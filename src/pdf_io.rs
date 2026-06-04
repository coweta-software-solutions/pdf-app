use pdfium_render::prelude::{PdfDocument, PdfPageIndex, Pdfium};

use crate::{
    error::{AppError, AppResult},
    page_selection::PageSelection,
};

pub(crate) fn load_pdf<'a>(pdfium: &'a Pdfium, bytes: Vec<u8>) -> AppResult<PdfDocument<'a>> {
    pdfium
        .load_pdf_from_byte_vec(bytes, None)
        .map_err(|err| AppError::bad_request(format!("could not read PDF: {err}")))
}

pub(crate) fn create_pdf<'a>(pdfium: &'a Pdfium, action: &str) -> AppResult<PdfDocument<'a>> {
    pdfium
        .create_new_pdf()
        .map_err(|err| AppError::Internal(format!("could not create {action} PDF: {err}")))
}

pub(crate) fn save_pdf(document: &PdfDocument<'_>, action: &str) -> AppResult<Vec<u8>> {
    document
        .save_to_bytes()
        .map_err(|err| AppError::Internal(format!("could not save {action} PDF: {err}")))
}

pub(crate) fn page_count(count: PdfPageIndex) -> AppResult<usize> {
    usize::try_from(count)
        .map_err(|_| AppError::Internal(format!("invalid PDF page count: {count}")))
}

pub(crate) fn page_index(page_number: usize) -> AppResult<PdfPageIndex> {
    let zero_based = page_number
        .checked_sub(1)
        .ok_or_else(|| AppError::bad_request("page numbers start at 1"))?;

    PdfPageIndex::try_from(zero_based)
        .map_err(|_| AppError::bad_request(format!("page {page_number} is out of range")))
}

pub(crate) fn resolve_pages(
    selection: PageSelection,
    page_count: usize,
    empty_message: &str,
) -> AppResult<Vec<usize>> {
    let pages = selection.resolve(page_count)?;

    if pages.is_empty() {
        return Err(AppError::bad_request(empty_message));
    }

    if let Some(page) = pages.iter().find(|page| **page > page_count) {
        return Err(AppError::bad_request(format!(
            "page {page} is out of range; PDF has {page_count} pages"
        )));
    }

    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_index_rejects_zero() {
        assert!(page_index(0).is_err());
    }

    #[test]
    fn page_index_converts_to_zero_based_pdfium_index() {
        assert_eq!(page_index(1).unwrap(), 0);
        assert_eq!(page_index(3).unwrap(), 2);
    }

    #[test]
    fn resolve_pages_rejects_empty_and_out_of_range_selections() {
        assert_eq!(
            resolve_pages(PageSelection::All, 0, "empty")
                .unwrap_err()
                .to_string(),
            "empty"
        );
        assert_eq!(
            resolve_pages(PageSelection::Pages(vec![]), 2, "empty")
                .unwrap_err()
                .to_string(),
            "no page numbers specified"
        );
        assert_eq!(
            resolve_pages(PageSelection::Pages(vec![3]), 2, "empty")
                .unwrap_err()
                .to_string(),
            "page 3 is out of range; PDF has 2 pages"
        );
    }
}
