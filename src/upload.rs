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

    pub fn into_one_file(self) -> AppResult<UploadFile> {
        if self.files.len() != 1 {
            return Err(AppError::bad_request("expected exactly one file"));
        }

        self.files
            .into_iter()
            .next()
            .ok_or_else(|| AppError::bad_request("expected exactly one file"))
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

fn safe_display_name(name: &str) -> String {
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

fn is_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

fn is_supported_image(bytes: &[u8]) -> bool {
    image::guess_format(bytes)
        .map(|format| matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg))
        .unwrap_or(false)
}

pub(crate) fn require_files(files: &[UploadFile]) -> AppResult<()> {
    if files.is_empty() {
        Err(AppError::bad_request("expected at least one file"))
    } else {
        Ok(())
    }
}

pub(crate) fn require_pdfs(files: &[UploadFile], context: &str) -> AppResult<()> {
    require_files(files)?;
    if let Some(file) = files.iter().find(|file| !is_pdf(&file.bytes)) {
        return Err(AppError::bad_request(format!(
            "{context}: `{}`",
            file.filename
        )));
    }
    Ok(())
}

pub(crate) fn require_images(files: &[UploadFile], context: &str) -> AppResult<()> {
    require_files(files)?;
    if let Some(file) = files.iter().find(|file| !is_supported_image(&file.bytes)) {
        return Err(AppError::bad_request(format!(
            "{context}: `{}`",
            file.filename
        )));
    }
    Ok(())
}

pub(crate) fn require_at_least_pdfs(
    files: &[UploadFile],
    min: usize,
    count_message: &str,
) -> AppResult<()> {
    if files.len() < min {
        return Err(AppError::bad_request(count_message));
    }
    require_pdfs(files, "expected PDF input")
}

pub(crate) fn require_one_pdf(form: FormData, context: &str) -> AppResult<UploadFile> {
    let file = form.into_one_file()?;
    if !is_pdf(&file.bytes) {
        return Err(AppError::bad_request(format!(
            "{context}: `{}`",
            file.filename
        )));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(filename: &str, bytes: &[u8]) -> UploadFile {
        UploadFile {
            filename: filename.to_string(),
            bytes: bytes.to_vec(),
        }
    }

    fn sample_image_bytes(format: image::ImageFormat) -> Vec<u8> {
        let image = image::ImageBuffer::from_fn(2, 2, |_, _| image::Rgba([10, 20, 30, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut bytes), format)
            .unwrap();
        bytes
    }

    #[test]
    fn safe_display_name_removes_directories_and_unsafe_characters() {
        assert_eq!(safe_display_name("../foo.pdf"), "foo.pdf");
        assert_eq!(safe_display_name(r"C:\temp\foo.pdf"), "foo.pdf");
        assert_eq!(safe_display_name("my file (1).pdf"), "myfile1.pdf");
        assert_eq!(safe_display_name("..."), "...");
        assert_eq!(safe_display_name("()"), "upload");
    }

    #[test]
    fn require_files_rejects_empty_uploads() {
        assert_eq!(
            require_files(&[]).unwrap_err().to_string(),
            "expected at least one file"
        );
    }

    #[test]
    fn pdf_validation_reports_offending_filename() {
        let error = require_pdfs(
            &[file("notes.txt", b"not a pdf")],
            "target=png/jpeg requires PDF input",
        )
        .unwrap_err();

        assert!(error.to_string().contains("notes.txt"));
    }

    #[test]
    fn image_validation_reports_offending_filename() {
        let error = require_images(
            &[file("notes.txt", b"not an image")],
            "target=pdf requires PNG or JPEG input",
        )
        .unwrap_err();

        assert!(error.to_string().contains("notes.txt"));
    }

    #[test]
    fn require_at_least_pdfs_checks_count_before_type() {
        let error =
            require_at_least_pdfs(&[file("notes.txt", b"not a pdf")], 2, "need two").unwrap_err();

        assert_eq!(error.to_string(), "need two");
    }

    #[test]
    fn require_one_pdf_rejects_multiple_files_and_invalid_type() {
        let multiple = FormData {
            files: vec![file("one.pdf", b"%PDF-one"), file("two.pdf", b"%PDF-two")],
            fields: Vec::new(),
        };
        assert_eq!(
            require_one_pdf(multiple, "split requires a PDF input")
                .unwrap_err()
                .to_string(),
            "expected exactly one file"
        );

        let invalid = FormData {
            files: vec![file("notes.txt", b"not a pdf")],
            fields: Vec::new(),
        };
        let error = require_one_pdf(invalid, "split requires a PDF input").unwrap_err();
        assert!(error.to_string().contains("notes.txt"));
    }

    #[test]
    fn supported_image_detection_accepts_png_and_jpeg_only() {
        assert!(is_supported_image(&sample_image_bytes(
            image::ImageFormat::Png
        )));
        assert!(is_supported_image(&sample_image_bytes(
            image::ImageFormat::Jpeg
        )));
        assert!(!is_supported_image(b"plain text"));
    }

    #[test]
    fn pdf_detection_uses_magic_prefix() {
        assert!(is_pdf(b"%PDF-1.7\n"));
        assert!(!is_pdf(b"not a pdf"));
    }
}
