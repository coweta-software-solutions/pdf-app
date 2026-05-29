use axum::extract::Multipart;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct UploadFile {
    pub filename: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct FormData {
    pub files: Vec<UploadFile>,
    pub fields: Vec<(String, String)>,
}

impl FormData {
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn required_field(&self, name: &str) -> AppResult<&str> {
        self.field(name)
            .ok_or_else(|| AppError::bad_request(format!("missing field `{name}`")))
    }

    pub fn one_file(&self) -> AppResult<UploadFile> {
        if self.files.len() != 1 {
            return Err(AppError::bad_request("expected exactly one file"));
        }
        Ok(self.files[0].clone())
    }
}

pub async fn read_multipart(mut multipart: Multipart) -> AppResult<FormData> {
    let mut form = FormData::default();

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }

        let filename = field
            .file_name()
            .map(safe_display_name)
            .unwrap_or_else(|| "upload".to_string());
        let bytes = field.bytes().await?.to_vec();

        if name == "file" || name == "files" || name == "files[]" {
            if bytes.is_empty() {
                return Err(AppError::bad_request("uploaded file is empty"));
            }
            form.files.push(UploadFile { filename, bytes });
        } else {
            let value = String::from_utf8(bytes)
                .map_err(|_| AppError::bad_request(format!("field `{name}` is not valid UTF-8")))?;
            form.fields.push((name, value.trim().to_string()));
        }
    }

    Ok(form)
}

pub fn safe_display_name(name: &str) -> String {
    let basename = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("upload")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect::<String>();

    if basename.is_empty() {
        "upload".to_string()
    } else {
        basename
    }
}

pub fn is_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

pub fn is_supported_image(bytes: &[u8]) -> bool {
    image::guess_format(bytes)
        .map(|format| matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg))
        .unwrap_or(false)
}
