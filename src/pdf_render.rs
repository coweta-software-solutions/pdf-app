use std::{io::Cursor, sync::Arc};

use image::{codecs::jpeg::JpegEncoder, ColorType};
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use crate::{
    error::{AppError, AppResult},
    page_selection::PageSelection,
    pdf_io,
    progress::report,
    upload::UploadFile,
    ProgressCallback,
};

const PT_PER_IN: f32 = 72.0;
const PDF_IMAGE_DPI: u16 = 144;
const PDF_IMAGE_JPEG_QUALITY: u8 = 75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RasterFormat {
    Png,
    Jpeg,
}

pub(crate) struct RenderedImage {
    pub(crate) page: usize,
    pub(crate) zip_path: String,
    pub(crate) bytes: Vec<u8>,
}

impl RasterFormat {
    pub(crate) fn content_type(self) -> &'static str {
        match self {
            RasterFormat::Png => "image/png",
            RasterFormat::Jpeg => "image/jpeg",
        }
    }

    pub(crate) fn extension(self) -> &'static str {
        match self {
            RasterFormat::Png => "png",
            RasterFormat::Jpeg => "jpg",
        }
    }
}

pub(crate) fn render_pdf_files(
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

        let range_start = render_percent(index, 0, file_count);
        let range_end = render_percent(index + 1, 0, file_count);

        if file_count > 1 {
            report(
                &progress,
                range_start,
                format!("Reading PDF {} of {file_count}", index + 1),
            );
        }

        let images = render_pdf_pages(RenderPageJob {
            pdfium: pdfium.clone(),
            bytes: file.bytes,
            selection: selection.clone(),
            max_pages: remaining_pages,
            format,
            progress: progress.clone(),
            progress_range: ProgressRange {
                start: range_start,
                end: range_end,
            },
            file_position: (index + 1, file_count),
        })?;

        for mut image in images {
            let output_index = rendered.len() + 1;
            image.zip_path = format!("page-{output_index:04}.{}", format.extension());
            rendered.push(image);
        }
    }

    Ok(rendered)
}

fn render_pdf_pages(job: RenderPageJob) -> AppResult<Vec<RenderedImage>> {
    let RenderPageJob {
        pdfium,
        bytes,
        selection,
        max_pages,
        format,
        progress,
        progress_range,
        file_position,
    } = job;

    let document = pdf_io::load_pdf(&pdfium, bytes)?;
    let page_count = pdf_io::page_count(document.pages().len())?;
    let selected_pages = resolve_render_pages(selection, page_count, max_pages)?;
    report(
        &progress,
        progress_range.start,
        format!("Preparing {} page(s)", selected_pages.len()),
    );

    let total_pages = selected_pages.len();
    let mut rendered = Vec::with_capacity(total_pages);
    let render_config =
        PdfRenderConfig::new().scale_page_by_factor(PDF_IMAGE_DPI as f32 / PT_PER_IN);

    for (index, page_number) in selected_pages.into_iter().enumerate() {
        report(
            &progress,
            progress_range.page_percent(index, total_pages),
            format!("Rendering page {page_number}"),
        );
        let page = document
            .pages()
            .get(pdf_io::page_index(page_number)?)
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
            progress_range.page_percent(index + 1, total_pages),
            rendered_message(index + 1, total_pages, file_position),
        );
        rendered.push(RenderedImage {
            page: page_number,
            zip_path: format!("page-{:04}.{}", page_number, format.extension()),
            bytes,
        });
    }

    Ok(rendered)
}

struct RenderPageJob {
    pdfium: Arc<Pdfium>,
    bytes: Vec<u8>,
    selection: PageSelection,
    max_pages: usize,
    format: RasterFormat,
    progress: Option<ProgressCallback>,
    progress_range: ProgressRange,
    file_position: (usize, usize),
}

fn resolve_render_pages(
    selection: PageSelection,
    page_count: usize,
    max_pages: usize,
) -> AppResult<Vec<usize>> {
    let pages = pdf_io::resolve_pages(selection, page_count, "PDF has no pages to render")?;

    if pages.len() > max_pages {
        return Err(AppError::bad_request(format!(
            "requested {} pages; limit is {max_pages}",
            pages.len()
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

pub(crate) fn zip_images(images: Vec<RenderedImage>) -> AppResult<Vec<u8>> {
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

fn render_percent(file_index: usize, page_index: usize, file_count: usize) -> u8 {
    let total_units = file_count.max(1) * 100;
    let completed_units = file_index * 100 + page_index.min(100);
    35 + ((completed_units * 50) / total_units) as u8
}

fn rendered_message(completed: usize, total: usize, file_position: (usize, usize)) -> String {
    let (file_index, file_count) = file_position;
    if file_count > 1 {
        format!("Rendered {completed} of {total} pages in PDF {file_index} of {file_count}")
    } else {
        format!("Rendered {completed} of {total} pages")
    }
}

#[derive(Clone, Copy)]
struct ProgressRange {
    start: u8,
    end: u8,
}

impl ProgressRange {
    fn page_percent(self, completed_pages: usize, total_pages: usize) -> u8 {
        let span = usize::from(self.end.saturating_sub(self.start));
        self.start + ((completed_pages * span) / total_pages.max(1)) as u8
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use pdfium_render::prelude::{PdfPagePaperSize, Pdfium};

    fn pdfium_or_skip() -> Option<Arc<Pdfium>> {
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
    fn raster_formats_define_download_metadata() {
        assert_eq!(RasterFormat::Png.content_type(), "image/png");
        assert_eq!(RasterFormat::Png.extension(), "png");
        assert_eq!(RasterFormat::Jpeg.content_type(), "image/jpeg");
        assert_eq!(RasterFormat::Jpeg.extension(), "jpg");
    }

    #[test]
    fn render_progress_math_is_monotonic_and_bounded() {
        let updates = [
            render_percent(0, 0, 2),
            render_percent(0, 100, 2),
            render_percent(1, 0, 2),
            render_percent(1, 100, 2),
        ];

        assert_eq!(updates[0], 35);
        assert_eq!(updates[3], 85);
        assert!(updates.windows(2).all(|pair| pair[0] <= pair[1]));

        let range = ProgressRange { start: 40, end: 60 };
        assert_eq!(range.page_percent(0, 4), 40);
        assert_eq!(range.page_percent(2, 4), 50);
        assert_eq!(range.page_percent(4, 4), 60);
    }

    #[test]
    fn rendered_messages_include_file_position_only_for_batches() {
        assert_eq!(rendered_message(1, 2, (1, 1)), "Rendered 1 of 2 pages");
        assert_eq!(
            rendered_message(1, 2, (2, 3)),
            "Rendered 1 of 2 pages in PDF 2 of 3"
        );
    }

    #[test]
    fn render_pdf_pages_renders_selected_page_when_pdfium_is_available() {
        let Some(pdfium) = pdfium_or_skip() else {
            return;
        };

        let rendered = render_pdf_pages(RenderPageJob {
            pdfium: pdfium.clone(),
            bytes: sample_pdf_bytes(&pdfium, 2),
            selection: PageSelection::Pages(vec![2]),
            max_pages: 10,
            format: RasterFormat::Png,
            progress: None,
            progress_range: ProgressRange { start: 35, end: 85 },
            file_position: (1, 1),
        })
        .unwrap();

        assert_eq!(rendered.len(), 1);
        assert_eq!(rendered[0].page, 2);
        assert!(rendered[0].bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn render_pdf_pages_renders_jpeg_when_pdfium_is_available() {
        let Some(pdfium) = pdfium_or_skip() else {
            return;
        };

        let rendered = render_pdf_pages(RenderPageJob {
            pdfium: pdfium.clone(),
            bytes: sample_pdf_bytes(&pdfium, 1),
            selection: PageSelection::Pages(vec![1]),
            max_pages: 10,
            format: RasterFormat::Jpeg,
            progress: None,
            progress_range: ProgressRange { start: 35, end: 85 },
            file_position: (1, 1),
        })
        .unwrap();

        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].bytes.starts_with(&[0xff, 0xd8]));
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
    fn zip_images_uses_flat_root_level_paths() {
        let images = vec![
            RenderedImage {
                page: 1,
                zip_path: "page-0001.png".to_string(),
                bytes: b"one".to_vec(),
            },
            RenderedImage {
                page: 2,
                zip_path: "page-0002.png".to_string(),
                bytes: b"two".to_vec(),
            },
        ];
        let zip = zip_images(images).unwrap();

        for path in ["page-0001.png", "page-0002.png"] {
            assert!(zip
                .windows(path.len())
                .any(|window| window == path.as_bytes()));
        }
        assert!(!zip
            .windows("page-0001.png".len() + 1)
            .any(|window| window.ends_with(b"/page-0001.png")));
    }

    #[test]
    fn multi_pdf_render_uses_flat_global_zip_sequence_when_pdfium_is_available() {
        let Some(pdfium) = pdfium_or_skip() else {
            return;
        };

        let rendered = render_pdf_files(
            pdfium.clone(),
            vec![
                UploadFile {
                    filename: "first.pdf".to_string(),
                    bytes: sample_pdf_bytes(&pdfium, 1),
                },
                UploadFile {
                    filename: "second.pdf".to_string(),
                    bytes: sample_pdf_bytes(&pdfium, 1),
                },
            ],
            PageSelection::Pages(vec![1]),
            10,
            RasterFormat::Png,
            None,
        )
        .unwrap();

        let zip_paths = rendered
            .iter()
            .map(|image| image.zip_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(zip_paths, ["page-0001.png", "page-0002.png"]);
        assert!(zip_paths.iter().all(|path| !path.contains('/')));
    }

    #[test]
    fn multi_pdf_render_sequences_selected_pages_when_pdfium_is_available() {
        let Some(pdfium) = pdfium_or_skip() else {
            return;
        };

        let rendered = render_pdf_files(
            pdfium.clone(),
            vec![
                UploadFile {
                    filename: "first.pdf".to_string(),
                    bytes: sample_pdf_bytes(&pdfium, 2),
                },
                UploadFile {
                    filename: "second.pdf".to_string(),
                    bytes: sample_pdf_bytes(&pdfium, 2),
                },
            ],
            PageSelection::Pages(vec![1, 2]),
            10,
            RasterFormat::Png,
            None,
        )
        .unwrap();

        let zip_paths = rendered
            .iter()
            .map(|image| image.zip_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            zip_paths,
            [
                "page-0001.png",
                "page-0002.png",
                "page-0003.png",
                "page-0004.png"
            ]
        );
        assert!(zip_paths.iter().all(|path| !path.contains('/')));
    }

    #[test]
    fn multi_pdf_render_progress_is_monotonic_when_pdfium_is_available() {
        let Some(pdfium) = pdfium_or_skip() else {
            return;
        };

        let updates = Arc::new(Mutex::new(Vec::new()));
        let progress_updates = updates.clone();
        let progress: ProgressCallback = Arc::new(move |percent, _| {
            progress_updates.lock().unwrap().push(percent);
        });

        render_pdf_files(
            pdfium.clone(),
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
            PageSelection::Pages(vec![1]),
            10,
            RasterFormat::Png,
            Some(progress),
        )
        .unwrap();

        let updates = updates.lock().unwrap();
        assert!(updates.windows(2).all(|pair| pair[0] <= pair[1]));
    }
}
