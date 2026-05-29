use axum::{
    http::header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    response::{IntoResponse, Response},
};

pub struct FileDownload {
    pub content_type: String,
    pub filename: String,
    pub bytes: Vec<u8>,
}

impl FileDownload {
    pub fn new(
        content_type: impl Into<String>,
        filename: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            content_type: content_type.into(),
            filename: filename.into(),
            bytes,
        }
    }

    pub fn into_response(self) -> Response {
        let content_disposition = format!("attachment; filename=\"{}\"", self.filename);
        (
            [
                (CONTENT_TYPE, self.content_type),
                (CONTENT_DISPOSITION, content_disposition),
            ],
            self.bytes,
        )
            .into_response()
    }
}
