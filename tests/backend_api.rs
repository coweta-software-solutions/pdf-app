use std::{sync::Arc, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
    response::Response,
};
use image::{ImageBuffer, Rgba};
use pdf_tools_server::{app, AppState};
use pdfium_render::prelude::{PdfPagePaperSize, Pdfium};
use tokio::time::{sleep, Instant};
use tower::ServiceExt;

struct MultipartPart {
    name: String,
    filename: Option<String>,
    bytes: Vec<u8>,
}

fn text_part(name: &str, value: &str) -> MultipartPart {
    MultipartPart {
        name: name.to_string(),
        filename: None,
        bytes: value.as_bytes().to_vec(),
    }
}

fn upload_part(name: &str, filename: &str, bytes: Vec<u8>) -> MultipartPart {
    MultipartPart {
        name: name.to_string(),
        filename: Some(filename.to_string()),
        bytes,
    }
}

fn multipart_request(path: &str, parts: Vec<MultipartPart>) -> Request<Body> {
    let boundary = "pdf-tools-test-boundary";
    let mut body = Vec::new();

    for part in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match part.filename {
            Some(filename) => body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n\r\n",
                    part.name, filename
                )
                .as_bytes(),
            ),
            None => body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                    part.name
                )
                .as_bytes(),
            ),
        }
        body.extend_from_slice(&part.bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap()
}

fn get_request(path: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(path)
        .body(Body::empty())
        .unwrap()
}

fn test_pdfium() -> Option<Arc<Pdfium>> {
    let path = std::env::var("PDF_TOOLS_PDFIUM_PATH").ok()?;
    if path.trim().is_empty() {
        return None;
    }
    Pdfium::bind_to_library(path.trim())
        .ok()
        .map(Pdfium::new)
        .map(Arc::new)
}

fn state_or_skip() -> Option<Arc<AppState>> {
    let pdfium = test_pdfium();
    if pdfium.is_none() {
        eprintln!("skipping backend API test: PDF_TOOLS_PDFIUM_PATH is not set");
    }
    pdfium.map(AppState::for_tests).map(Arc::new)
}

async fn send(state: Arc<AppState>, request: Request<Body>) -> Response {
    app(state, 100).oneshot(request).await.unwrap()
}

async fn body_text(response: Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn body_bytes(response: Response) -> Vec<u8> {
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

fn content_disposition(response: &Response) -> &str {
    response
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
}

fn sample_png_bytes() -> Vec<u8> {
    let image = ImageBuffer::from_fn(12, 12, |x, y| {
        if (x + y) % 2 == 0 {
            Rgba([220, 30, 30, 255])
        } else {
            Rgba([30, 90, 220, 255])
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

fn sample_pdf_bytes(pdfium: &Pdfium, page_count: usize) -> Vec<u8> {
    let mut doc = pdfium.create_new_pdf().unwrap();
    for _ in 0..page_count {
        doc.pages_mut()
            .create_page_at_end(PdfPagePaperSize::a4())
            .unwrap();
    }
    doc.save_to_bytes().unwrap()
}

fn parse_status_lines(body: &str) -> std::collections::HashMap<String, String> {
    body.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

async fn wait_for_job_status(state: Arc<AppState>, id: &str, status: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let response = send(state.clone(), get_request(&format!("/jobs/{id}"))).await;
        let body = body_text(response).await;
        let fields = parse_status_lines(&body);
        if fields.get("status").map(String::as_str) == Some(status) {
            return body;
        }
        assert!(
            Instant::now() < deadline,
            "job did not reach {status}: {body}"
        );
        sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let Some(state) = state_or_skip() else {
        return;
    };

    let response = send(state, get_request("/health")).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "ok");
}

#[tokio::test]
async fn convert_endpoint_validation_errors_are_precise() {
    let Some(state) = state_or_skip() else {
        return;
    };

    let missing_target = send(
        state.clone(),
        multipart_request(
            "/convert",
            vec![upload_part("files", "image.png", sample_png_bytes())],
        ),
    )
    .await;
    assert_eq!(missing_target.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(missing_target)
        .await
        .contains("missing field `target`"));

    let bad_image = send(
        state.clone(),
        multipart_request(
            "/convert",
            vec![
                text_part("target", "pdf"),
                upload_part("files", "notes.txt", b"not an image".to_vec()),
            ],
        ),
    )
    .await;
    assert_eq!(bad_image.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(bad_image).await.contains("notes.txt"));

    let bad_pdf = send(
        state,
        multipart_request(
            "/convert",
            vec![
                text_part("target", "png"),
                upload_part("files", "notes.txt", b"not a pdf".to_vec()),
            ],
        ),
    )
    .await;
    assert_eq!(bad_pdf.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(bad_pdf).await.contains("notes.txt"));
}

#[tokio::test]
async fn merge_and_split_validation_errors_do_not_enter_pdfium_work() {
    let Some(state) = state_or_skip() else {
        return;
    };

    let merge = send(
        state.clone(),
        multipart_request(
            "/merge",
            vec![upload_part("files", "one.pdf", b"%PDF-pretend".to_vec())],
        ),
    )
    .await;
    assert_eq!(merge.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(merge).await.contains("at least two"));

    let split = send(
        state,
        multipart_request(
            "/split",
            vec![upload_part("file", "source.pdf", b"%PDF-pretend".to_vec())],
        ),
    )
    .await;
    assert_eq!(split.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(split).await.contains("missing field `pages`"));
}

#[tokio::test]
async fn convert_png_to_pdf_endpoint_returns_valid_pdf_download() {
    let Some(state) = state_or_skip() else {
        return;
    };
    let pdfium = state.pdfium();

    let response = send(
        state,
        multipart_request(
            "/convert",
            vec![
                text_part("target", "pdf"),
                text_part("layout", "single"),
                upload_part("files", "image.png", sample_png_bytes()),
            ],
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/pdf"
    );
    assert!(content_disposition(&response).contains("converted.pdf"));

    let bytes = body_bytes(response).await;
    let document = pdfium.load_pdf_from_byte_vec(bytes, None).unwrap();
    assert_eq!(document.pages().len(), 1);
}

#[tokio::test]
async fn convert_pdf_to_png_endpoint_returns_png_payload() {
    let Some(state) = state_or_skip() else {
        return;
    };
    let pdfium = state.pdfium();

    let response = send(
        state,
        multipart_request(
            "/convert",
            vec![
                text_part("target", "png"),
                text_part("pages", "1"),
                upload_part("files", "source.pdf", sample_pdf_bytes(&pdfium, 1)),
            ],
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/png"
    );
    assert!(content_disposition(&response).contains("page-0001.png"));
    assert!(body_bytes(response).await.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[tokio::test]
async fn convert_pdf_to_jpeg_endpoint_returns_jpeg_payload() {
    let Some(state) = state_or_skip() else {
        return;
    };
    let pdfium = state.pdfium();

    let response = send(
        state,
        multipart_request(
            "/convert",
            vec![
                text_part("target", "jpeg"),
                text_part("pages", "1"),
                upload_part("files", "source.pdf", sample_pdf_bytes(&pdfium, 1)),
            ],
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/jpeg"
    );
    assert!(content_disposition(&response).contains("page-0001.jpg"));
    assert!(body_bytes(response).await.starts_with(&[0xff, 0xd8]));
}

#[tokio::test]
async fn merge_endpoint_returns_valid_combined_pdf() {
    let Some(state) = state_or_skip() else {
        return;
    };
    let pdfium = state.pdfium();

    let response = send(
        state,
        multipart_request(
            "/merge",
            vec![
                upload_part("files", "one.pdf", sample_pdf_bytes(&pdfium, 1)),
                upload_part("files", "two.pdf", sample_pdf_bytes(&pdfium, 2)),
            ],
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(content_disposition(&response).contains("merged.pdf"));
    let document = pdfium
        .load_pdf_from_byte_vec(body_bytes(response).await, None)
        .unwrap();
    assert_eq!(document.pages().len(), 3);
}

#[tokio::test]
async fn split_endpoint_returns_valid_selected_pdf() {
    let Some(state) = state_or_skip() else {
        return;
    };
    let pdfium = state.pdfium();

    let response = send(
        state,
        multipart_request(
            "/split",
            vec![
                text_part("pages", "2"),
                upload_part("file", "source.pdf", sample_pdf_bytes(&pdfium, 2)),
            ],
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(content_disposition(&response).contains("split.pdf"));
    let document = pdfium
        .load_pdf_from_byte_vec(body_bytes(response).await, None)
        .unwrap();
    assert_eq!(document.pages().len(), 1);
}

#[tokio::test]
async fn jobs_unknown_action_eventually_reports_error() {
    let Some(state) = state_or_skip() else {
        return;
    };

    let response = send(
        state.clone(),
        multipart_request("/jobs", vec![text_part("action", "unknown")]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let id = body_text(response).await;

    let body = wait_for_job_status(state, &id, "error").await;
    let fields = parse_status_lines(&body);
    assert_eq!(
        fields.get("error").map(String::as_str),
        Some("unknown job action")
    );
}

#[tokio::test]
async fn job_status_and_download_errors_are_reported() {
    let Some(state) = state_or_skip() else {
        return;
    };

    let missing = send(state.clone(), get_request("/jobs/missing")).await;
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);

    let id = state.jobs.create();
    let pending_download = send(state.clone(), get_request(&format!("/jobs/{id}/download"))).await;
    assert_eq!(pending_download.status(), StatusCode::CONFLICT);

    state.jobs.fail(&id, "bad\nthing");
    let status = send(state, get_request(&format!("/jobs/{id}"))).await;
    assert_eq!(status.status(), StatusCode::OK);
    let fields = parse_status_lines(&body_text(status).await);
    assert_eq!(fields.get("status").map(String::as_str), Some("error"));
    assert_eq!(fields.get("error").map(String::as_str), Some("bad thing"));
}

#[tokio::test]
async fn job_convert_pdf_download_is_consumed_once() {
    let Some(state) = state_or_skip() else {
        return;
    };

    let response = send(
        state.clone(),
        multipart_request(
            "/jobs",
            vec![
                text_part("action", "convert"),
                text_part("target", "pdf"),
                upload_part("files", "image.png", sample_png_bytes()),
            ],
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let id = body_text(response).await;

    let body = wait_for_job_status(state.clone(), &id, "done").await;
    let fields = parse_status_lines(&body);
    assert_eq!(
        fields.get("filename").map(String::as_str),
        Some("converted.pdf")
    );

    let download = send(state.clone(), get_request(&format!("/jobs/{id}/download"))).await;
    assert_eq!(download.status(), StatusCode::OK);
    assert!(content_disposition(&download).contains("converted.pdf"));

    let second_download = send(state, get_request(&format!("/jobs/{id}/download"))).await;
    assert_eq!(second_download.status(), StatusCode::BAD_REQUEST);
}
