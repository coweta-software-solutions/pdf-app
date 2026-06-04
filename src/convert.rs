use std::sync::Arc;

use axum::{
    extract::{Multipart, State},
    response::Response,
};

use crate::{
    error::{AppError, AppResult},
    image_pdf,
    page_selection::PageSelection,
    pdf_render::{self, RasterFormat},
    progress::report,
    upload::{read_multipart, require_files, require_images, require_pdfs, FormData},
    AppState, FileDownload, ProgressCallback,
};

pub async fn convert(
    State(state): State<Arc<AppState>>,
    multipart: Multipart,
) -> AppResult<Response> {
    let form = read_multipart(multipart).await?;
    Ok(convert_form(state, form, None).await?.into_response())
}

pub async fn convert_form(
    state: Arc<AppState>,
    form: FormData,
    progress: Option<ProgressCallback>,
) -> AppResult<FileDownload> {
    match ConvertTarget::parse(form.required_field("target")?)? {
        ConvertTarget::Pdf => images_to_pdf(state, form, progress).await,
        ConvertTarget::Png => pdfs_to_images(state, form, RasterFormat::Png, progress).await,
        ConvertTarget::Jpeg => pdfs_to_images(state, form, RasterFormat::Jpeg, progress).await,
    }
}

async fn images_to_pdf(
    state: Arc<AppState>,
    form: FormData,
    progress: Option<ProgressCallback>,
) -> AppResult<FileDownload> {
    require_files(&form.files)?;
    require_images(&form.files, "target=pdf requires PNG or JPEG input")?;
    report(&progress, 35, "Decoding image");
    image_pdf::validate_image_pdf_options(&form)?;

    report(&progress, 60, "Building PDF");
    let bytes = state
        .run_cpu(move || image_pdf::images_to_pdf(&form.files))
        .await?;
    report(&progress, 95, "Preparing download");
    Ok(FileDownload::new("application/pdf", "converted.pdf", bytes))
}

async fn pdfs_to_images(
    state: Arc<AppState>,
    form: FormData,
    format: RasterFormat,
    progress: Option<ProgressCallback>,
) -> AppResult<FileDownload> {
    require_files(&form.files)?;
    require_pdfs(&form.files, "target=png/jpeg requires PDF input")?;
    let pages = PageSelection::parse(form.field("pages").unwrap_or("1"))?;

    report(&progress, 25, "Reading PDF");
    let is_single_file = form.files.len() == 1;
    let max_pages = state.max_render_pages();
    let pdfium = state.pdfium();
    let render_progress = progress.clone();
    let rendered = state
        .run_cpu(move || {
            pdf_render::render_pdf_files(
                pdfium,
                form.files,
                pages,
                max_pages,
                format,
                render_progress,
            )
        })
        .await?;

    if is_single_file && rendered.len() == 1 {
        let image = rendered
            .into_iter()
            .next()
            .expect("rendered length was checked");
        report(&progress, 95, "Preparing download");
        return Ok(FileDownload::new(
            format.content_type(),
            format!("page-{:04}.{}", image.page, format.extension()),
            image.bytes,
        ));
    }

    report(&progress, 90, "Packaging images");
    let zip_bytes = state
        .run_cpu(move || pdf_render::zip_images(rendered))
        .await?;
    report(&progress, 95, "Preparing download");
    Ok(FileDownload::new("application/zip", "pages.zip", zip_bytes))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConvertTarget {
    Pdf,
    Png,
    Jpeg,
}

impl ConvertTarget {
    fn parse(target: &str) -> AppResult<Self> {
        match target.to_ascii_lowercase().as_str() {
            "pdf" => Ok(Self::Pdf),
            "png" => Ok(Self::Png),
            "jpeg" => Ok(Self::Jpeg),
            _ => Err(AppError::bad_request("target must be pdf, png, or jpeg")),
        }
    }
}
