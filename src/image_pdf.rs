use image::GenericImageView;
use printpdf::{Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, RawImage, XObjectTransform};

use crate::{
    error::{AppError, AppResult},
    upload::{FormData, UploadFile},
};

const PT_PER_IN: f32 = 72.0;
const MM_PER_IN: f32 = 25.4;
const IMAGE_DPI: f32 = 300.0;

pub(crate) fn validate_image_pdf_options(form: &FormData) -> AppResult<()> {
    match form.field("layout").unwrap_or("single") {
        "single" => Ok(()),
        _ => Err(AppError::bad_request("layout must be single")),
    }
}

pub(crate) fn images_to_pdf(files: &[UploadFile]) -> AppResult<Vec<u8>> {
    let mut doc = PdfDocument::new("PDF Tools output");
    let pages = files
        .iter()
        .map(|file| image_page_from_upload(&mut doc, file))
        .collect::<AppResult<Vec<_>>>()?;

    let mut warnings = Vec::new();
    Ok(doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut warnings))
}

fn image_page_from_upload(doc: &mut PdfDocument, file: &UploadFile) -> AppResult<PdfPage> {
    let (image_w, image_h) = image_dimensions(file)?;
    let raw = decode_pdf_image(file)?;
    let image_id = doc.add_image(&raw);
    let page_w_pt = px_to_pt(image_w);
    let page_h_pt = px_to_pt(image_h);

    Ok(PdfPage::new(
        pt_to_mm(page_w_pt),
        pt_to_mm(page_h_pt),
        vec![Op::UseXobject {
            id: image_id,
            transform: XObjectTransform {
                dpi: Some(IMAGE_DPI),
                ..Default::default()
            },
        }],
    ))
}

fn image_dimensions(file: &UploadFile) -> AppResult<(u32, u32)> {
    image::load_from_memory(&file.bytes)
        .map(|image| image.dimensions())
        .map_err(|err| {
            AppError::bad_request(format!("could not decode `{}`: {err}", file.filename))
        })
}

fn decode_pdf_image(file: &UploadFile) -> AppResult<RawImage> {
    let mut warnings = Vec::new();
    RawImage::decode_from_bytes(&file.bytes, &mut warnings).map_err(|err| {
        AppError::bad_request(format!(
            "could not decode `{}` for PDF: {err}",
            file.filename
        ))
    })
}

fn px_to_pt(px: u32) -> f32 {
    px as f32 * PT_PER_IN / IMAGE_DPI
}

fn pt_to_mm(pt: f32) -> Mm {
    Mm(pt * MM_PER_IN / PT_PER_IN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pdfium_or_skip() -> Option<std::sync::Arc<pdfium_render::prelude::Pdfium>> {
        let pdfium = crate::test_pdfium();
        if pdfium.is_none() {
            eprintln!("skipping PDFium test: PDF_TOOLS_PDFIUM_PATH is not set");
        }
        pdfium
    }

    fn sample_png_bytes_with_size(width: u32, height: u32) -> Vec<u8> {
        let image = image::ImageBuffer::from_fn(width, height, |x, y| {
            if (x + y) % 2 == 0 {
                image::Rgba([220, 30, 30, 255])
            } else {
                image::Rgba([30, 90, 220, 255])
            }
        });
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    #[test]
    fn images_to_pdf_creates_one_original_size_page_per_image_when_pdfium_is_available() {
        let Some(pdfium) = pdfium_or_skip() else {
            return;
        };

        let bytes = images_to_pdf(&[
            UploadFile {
                filename: "wide.png".to_string(),
                bytes: sample_png_bytes_with_size(20, 10),
            },
            UploadFile {
                filename: "tall.png".to_string(),
                bytes: sample_png_bytes_with_size(10, 30),
            },
        ])
        .unwrap();

        let document = pdfium.load_pdf_from_byte_vec(bytes, None).unwrap();

        assert_eq!(document.pages().len(), 2);
        let first = document.pages().get(0).unwrap();
        let second = document.pages().get(1).unwrap();
        assert!((first.width().value - px_to_pt(20)).abs() < 0.01);
        assert!((first.height().value - px_to_pt(10)).abs() < 0.01);
        assert!((second.width().value - px_to_pt(10)).abs() < 0.01);
        assert!((second.height().value - px_to_pt(30)).abs() < 0.01);
    }

    #[test]
    fn image_pdf_options_accept_missing_and_single_layout() {
        assert!(validate_image_pdf_options(&FormData::default()).is_ok());
        assert!(validate_image_pdf_options(&FormData {
            files: Vec::new(),
            fields: vec![("layout".to_string(), "single".to_string())],
        })
        .is_ok());
    }

    #[test]
    fn image_pdf_options_reject_unsupported_layout() {
        let form = FormData {
            files: Vec::new(),
            fields: vec![("layout".to_string(), "grid".to_string())],
        };

        assert_eq!(
            validate_image_pdf_options(&form).unwrap_err().to_string(),
            "layout must be single"
        );
    }

    #[test]
    fn image_dimension_errors_include_filename() {
        let error = image_dimensions(&UploadFile {
            filename: "notes.txt".to_string(),
            bytes: b"not an image".to_vec(),
        })
        .unwrap_err();

        assert!(error.to_string().contains("notes.txt"));
    }
}
