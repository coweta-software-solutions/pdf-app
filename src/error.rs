use std::{fmt, io};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    PayloadTooLarge(String),
    ExternalTool(String),
    Internal(String),
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn external_tool(message: impl Into<String>) -> Self {
        Self::ExternalTool(message.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            AppError::PayloadTooLarge(message) => (StatusCode::PAYLOAD_TOO_LARGE, message),
            AppError::ExternalTool(message) => (StatusCode::BAD_GATEWAY, message),
            AppError::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };

        (status, message).into_response()
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::BadRequest(message)
            | AppError::PayloadTooLarge(message)
            | AppError::ExternalTool(message)
            | AppError::Internal(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for AppError {}

impl From<io::Error> for AppError {
    fn from(value: io::Error) -> Self {
        Self::Internal(value.to_string())
    }
}

impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(value: axum::extract::multipart::MultipartError) -> Self {
        if value.status() == StatusCode::PAYLOAD_TOO_LARGE {
            Self::PayloadTooLarge("upload is larger than the configured limit".to_string())
        } else {
            Self::BadRequest(value.to_string())
        }
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(value: tokio::task::JoinError) -> Self {
        Self::Internal(value.to_string())
    }
}

impl From<tokio::sync::AcquireError> for AppError {
    fn from(value: tokio::sync::AcquireError) -> Self {
        Self::Internal(value.to_string())
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(value: zip::result::ZipError) -> Self {
        Self::Internal(value.to_string())
    }
}
