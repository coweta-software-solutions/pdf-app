use std::{io::Cursor, sync::Arc};

use axum::{
    extract::{Multipart, State},
    response::Response,
};
use image::{codecs::jpeg::JpegEncoder, ColorType, GenericImageView};
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};
use printpdf::{Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, RawImage, XObjectTransform};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use crate::{
    error::{AppError, AppResult},
    page_selection::PageSelection,
    upload::{is_pdf, is_supported_image, read_multipart, UploadFile},
    AppState, FileDownload, ProgressCallback,
};

const PT_PER_IN: f32 = 72.0;
const MM_PER_IN: f32 = 25.4;
const IMAGE_DPI: f32 = 300.0;
const PDF_IMAGE_DPI: u16 = 144;
const PDF_IMAGE_JPEG_QUALITY: u8 = 75;

pub async fn convert(
    State(state): State<Arc<AppState>>,
    multipart: Multipart,
) -> AppResult<Response> {
    let form = read_multipart(multipart).await?;
    Ok(convert_form(state, form, None).await?.into_response())
}

pub async fn convert_form(
    state: Arc<AppState>,
    form: crate::upload::FormData,
    progress: Option<ProgressCallback>,
) -> AppResult<FileDownload> {
    let target = form.required_field("target")?.to_ascii_lowercase();

    match target.as_str() {
        "pdf" => {
            if form.files.is_empty() {
                return Err(AppError::bad_request("expected at least one file"));
            }
            for file in &form.files {
                if !is_supported_image(&file.bytes) {
                    return Err(AppError::bad_request(
                        "target=pdf requires PNG or JPEG input",
                    ));
                }
            }
            report(&progress, 35, "Decoding image");
            validate_image_pdf_form(&form)?;
            report(&progress, 60, "Building PDF");
            let bytes = state.run_cpu(move || images_to_pdf(&form.files)).await?;
            report(&progress, 95, "Preparing download");
            Ok(file_response("application/pdf", "converted.pdf", bytes))
        }
        "png" | "jpeg" => {
            if form.files.is_empty() {
                return Err(AppError::bad_request("expected at least one file"));
            }
            for file in &form.files {
                if !is_pdf(&file.bytes) {
                    return Err(AppError::bad_request("target=png/jpeg requires PDF input"));
                }
            }
            let pages = PageSelection::parse(form.field("pages").unwrap_or("1"))?;
            let format = if target == "png" {
                RasterFormat::Png
            } else {
                RasterFormat::Jpeg
            };
            pdfs_to_images(state, form.files, pages, format, progress).await
        }
        _ => Err(AppError::bad_request("target must be pdf, png, or jpeg")),
    }
}

fn report(progress: &Option<ProgressCallback>, percent: u8, stage: impl Into<String>) {
    if let Some(progress) = progress {
        progress(percent, stage.into());
    }
}

async fn pdfs_to_images(
    state: Arc<AppState>,
    files: Vec<UploadFile>,
    selection: PageSelection,
    format: RasterFormat,
    progress: Option<ProgressCallback>,
) -> AppResult<FileDownload> {
    report(&progress, 25, "Reading PDF");
    let is_single_file = files.len() == 1;
    let max_pages = state.max_render_pages();
    let pdfium = state.pdfium();
    let render_progress = progress.clone();
    let rendered = state
        .run_cpu(move || {
            render_pdf_files(pdfium, files, selection, max_pages, format, render_progress)
        })
        .await?;

    if is_single_file && rendered.len() == 1 {
        let image = rendered
            .into_iter()
            .next()
            .expect("rendered length was checked");
        report(&progress, 95, "Preparing download");
        return Ok(file_response(
            format.content_type(),
            &format!("page-{:04}.{}", image.page, format.extension()),
            image.bytes,
        ));
    }

    report(&progress, 90, "Packaging images");
    let zip_bytes = state.run_cpu(move || zip_images(rendered)).await?;
    report(&progress, 95, "Preparing download");
    Ok(file_response("application/zip", "pages.zip", zip_bytes))
}

fn render_pdf_files(
    pdfium: Arc<Pdfium>,
    files: Vec<UploadFile>,
    selection: PageSelection,
    max_pages: usize,
    format: RasterFormat,
    progress: Option<ProgressCallback>,
) -> AppResult<Vec<RenderedImage>> {
    let file_count = files.len();
    let mut rendered = Vec::new();

    for (index, file) in files.into_iter().enumerate() {
        let remaining_pages = max_pages.saturating_sub(rendered.len());
        if remaining_pages == 0 {
            return Err(AppError::bad_request(format!(
                "requested more than {max_pages} pages"
            )));
        }

        if file_count > 1 {
            report(
                &progress,
                30,
                format!("Reading PDF {} of {file_count}", index + 1),
            );
        }

        let source_dir = (file_count > 1).then(|| zip_source_dir(index, &file.filename));
        let mut images = render_pdf_pages(
            pdfium.clone(),
            file.bytes,
            selection.clone(),
            remaining_pages,
            format,
            progress.clone(),
        )?;

        if let Some(source_dir) = source_dir {
            for image in &mut images {
                image.zip_path =
                    format!("{source_dir}/page-{:04}.{}", image.page, format.extension());
            }
        }

        rendered.extend(images);
    }

    Ok(rendered)
}

fn render_pdf_pages(
    pdfium: Arc<Pdfium>,
    bytes: Vec<u8>,
    selection: PageSelection,
    max_pages: usize,
    format: RasterFormat,
    progress: Option<ProgressCallback>,
) -> AppResult<Vec<RenderedImage>> {
    let document = pdfium
        .load_pdf_from_byte_vec(bytes, None)
        .map_err(|err| AppError::bad_request(format!("could not read PDF: {err}")))?;
    let page_count = pdfium_page_count(document.pages().len())?;
    let selected_pages = resolve_render_pages(selection, page_count, max_pages)?;
    report(
        &progress,
        35,
        format!("Preparing {} page(s)", selected_pages.len()),
    );

    let total_pages = selected_pages.len();
    let mut rendered = Vec::with_capacity(total_pages);
    let render_config =
        PdfRenderConfig::new().scale_page_by_factor(PDF_IMAGE_DPI as f32 / PT_PER_IN);

    for (index, page_number) in selected_pages.into_iter().enumerate() {
        report(
            &progress,
            40 + ((index * 45) / total_pages) as u8,
            format!("Rendering page {page_number}"),
        );
        let page = document
            .pages()
            .get(page_index(page_number)?)
            .map_err(|err| {
                AppError::bad_request(format!("could not read page {page_number}: {err}"))
            })?;
        let image = page
            .render_with_config(&render_config)
            .and_then(|bitmap| bitmap.as_image())
            .map_err(|err| {
                AppError::Internal(format!("could not render page {page_number}: {err}"))
            })?;
        let bytes = encode_rendered_image(image, format)?;
        report(
            &progress,
            40 + (((index + 1) * 45) / total_pages) as u8,
            format!("Rendered {} of {total_pages} pages", index + 1),
        );
        rendered.push(RenderedImage {
            page: page_number,
            zip_path: format!("page-{:04}.{}", page_number, format.extension()),
            bytes,
        });
    }

    Ok(rendered)
}

fn pdfium_page_count(count: i32) -> AppResult<usize> {
    usize::try_from(count)
        .map_err(|_| AppError::Internal(format!("invalid PDF page count: {count}")))
}

fn page_index(page_number: usize) -> AppResult<i32> {
    let zero_based = page_number
        .checked_sub(1)
        .ok_or_else(|| AppError::bad_request("page numbers start at 1"))?;
    i32::try_from(zero_based)
        .map_err(|_| AppError::bad_request(format!("page {page_number} is out of range")))
}

fn resolve_render_pages(
    selection: PageSelection,
    page_count: usize,
    max_pages: usize,
) -> AppResult<Vec<usize>> {
    let pages = selection.resolve(page_count)?;

    if pages.is_empty() {
        return Err(AppError::bad_request("PDF has no pages to render"));
    }

    if pages.len() > max_pages {
        return Err(AppError::bad_request(format!(
            "requested {} pages; limit is {max_pages}",
            pages.len()
        )));
    }

    if let Some(page) = pages.iter().find(|page| **page > page_count) {
        return Err(AppError::bad_request(format!(
            "page {page} is out of range; PDF has {page_count} pages"
        )));
    }

    Ok(pages)
}

fn encode_rendered_image(image: image::DynamicImage, format: RasterFormat) -> AppResult<Vec<u8>> {
    let mut bytes = Vec::new();
    match format {
        RasterFormat::Png => {
            image
                .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                .map_err(|err| AppError::Internal(format!("could not encode PNG: {err}")))?;
        }
        RasterFormat::Jpeg => {
            let rgb = image.into_rgb8();
            JpegEncoder::new_with_quality(&mut bytes, PDF_IMAGE_JPEG_QUALITY)
                .encode(&rgb, rgb.width(), rgb.height(), ColorType::Rgb8.into())
                .map_err(|err| AppError::Internal(format!("could not encode JPEG: {err}")))?;
        }
    }
    Ok(bytes)
}

fn images_to_pdf(files: &[UploadFile]) -> AppResult<Vec<u8>> {
    let mut doc = PdfDocument::new("PDF Tools output");
    let mut pages = Vec::with_capacity(files.len());

    for file in files {
        let dynamic = image::load_from_memory(&file.bytes)
            .map_err(|err| AppError::bad_request(format!("could not decode image: {err}")))?;
        let (image_w, image_h) = dynamic.dimensions();

        let mut warnings = Vec::new();
        let raw = RawImage::decode_from_bytes(&file.bytes, &mut warnings).map_err(|err| {
            AppError::bad_request(format!("could not decode image for PDF: {err}"))
        })?;

        let image_id = doc.add_image(&raw);
        let page_w_pt = px_to_pt(image_w);
        let page_h_pt = px_to_pt(image_h);
        pages.push(PdfPage::new(
            pt_to_mm(page_w_pt),
            pt_to_mm(page_h_pt),
            vec![Op::UseXobject {
                id: image_id,
                transform: XObjectTransform {
                    dpi: Some(IMAGE_DPI),
                    ..Default::default()
                },
            }],
        ));
    }

    let mut warnings = Vec::new();
    Ok(doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut warnings))
}

fn zip_images(images: Vec<RenderedImage>) -> AppResult<Vec<u8>> {
    let estimated_size = images.iter().map(|image| image.bytes.len() + 128).sum();
    let mut cursor = Cursor::new(Vec::with_capacity(estimated_size));
    let mut zip = ZipWriter::new(&mut cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for image in images {
        zip.start_file(image.zip_path, options)?;
        std::io::Write::write_all(&mut zip, &image.bytes)?;
    }
    zip.finish()?;
    Ok(cursor.into_inner())
}

fn zip_source_dir(index: usize, filename: &str) -> String {
    let stem = filename
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(filename)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect::<String>();
    let stem = if stem.is_empty() { "pdf" } else { &stem };

    format!("{:02}-{stem}", index + 1)
}

fn file_response(content_type: &'static str, filename: &str, bytes: Vec<u8>) -> FileDownload {
    FileDownload::new(content_type, filename, bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RasterFormat {
    Png,
    Jpeg,
}

struct RenderedImage {
    page: usize,
    zip_path: String,
    bytes: Vec<u8>,
}

impl RasterFormat {
    fn content_type(self) -> &'static str {
        match self {
            RasterFormat::Png => "image/png",
            RasterFormat::Jpeg => "image/jpeg",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            RasterFormat::Png => "png",
            RasterFormat::Jpeg => "jpg",
        }
    }
}

fn validate_image_pdf_form(form: &crate::upload::FormData) -> AppResult<()> {
    match form.field("layout").unwrap_or("single") {
        "single" => Ok(()),
        _ => Err(AppError::bad_request("layout must be single")),
    }
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
    use image::{ImageBuffer, Rgba};

    fn sample_png_bytes() -> Vec<u8> {
        sample_png_bytes_with_size(12, 12)
    }

    fn sample_png_bytes_with_size(width: u32, height: u32) -> Vec<u8> {
        let image = ImageBuffer::from_fn(width, height, |x, y| {
            if (x + y) % 2 == 0 {
                Rgba([220, 30, 30, 255])
            } else {
                Rgba([30, 90, 220, 255])
            }
        });
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn sample_pdf_bytes(pdfium: &Pdfium, page_count: usize) -> Vec<u8> {
        let source_bytes = images_to_pdf(&[UploadFile {
            filename: "image.png".to_string(),
            bytes: sample_png_bytes(),
        }])
        .unwrap();
        let source = pdfium.load_pdf_from_byte_vec(source_bytes, None).unwrap();
        let mut output = pdfium.create_new_pdf().unwrap();
        for _ in 0..page_count {
            output.pages_mut().append(&source).unwrap();
        }
        output.save_to_bytes().unwrap()
    }

    #[test]
    fn images_to_pdf_creates_one_original_size_page_per_image_when_pdfium_is_available() {
        let Some(pdfium) = crate::test_pdfium() else {
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
    fn render_pdf_pages_renders_selected_page_when_pdfium_is_available() {
        let Some(pdfium) = crate::test_pdfium() else {
            return;
        };

        let rendered = render_pdf_pages(
            pdfium.clone(),
            sample_pdf_bytes(&pdfium, 2),
            PageSelection::Pages(vec![2]),
            10,
            RasterFormat::Png,
            None,
        )
        .unwrap();

        assert_eq!(rendered.len(), 1);
        assert_eq!(rendered[0].page, 2);
        assert!(rendered[0].bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn zip_images_stores_precompressed_images_by_page_number() {
        let zip = zip_images(vec![RenderedImage {
            page: 2,
            zip_path: "page-0002.png".to_string(),
            bytes: b"already-compressed".to_vec(),
        }])
        .unwrap();

        assert_eq!(u16::from_le_bytes([zip[8], zip[9]]), 0);
        assert!(zip
            .windows("page-0002.png".len())
            .any(|window| window == b"page-0002.png"));
    }

    #[test]
    fn zip_source_dir_prefixes_and_sanitizes_pdf_name() {
        assert_eq!(zip_source_dir(1, "merged (2).pdf"), "02-merged2");
        assert_eq!(zip_source_dir(0, "..."), "01-pdf");
    }
}
