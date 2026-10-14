//! Image and text attachment records and loading.
use base64::{Engine, engine::general_purpose::STANDARD};
use maestro_models::{Fetch, FetchError, HttpRequest};
use serde::Serialize;
use std::{borrow::Cow, fmt};

/// Model-facing attachment classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    /// Original image bytes.
    Image,
    /// Document with optional extracted text.
    Document,
}
/// Original attachment bytes with derived presentation data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// Generated identifier.
    pub id: String,
    /// Content classification.
    pub r#type: AttachmentType,
    /// Supplied or detected filename.
    pub file_name: String,
    /// Selected media type.
    pub mime_type: String,
    /// Original byte length.
    pub size: usize,
    /// Standard padded base64 of original bytes.
    pub content: String,
    /// Decoded document text, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted_text: Option<String>,
    /// Base64 image preview, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}
/// A URL or bytes already read by a frontend.
pub enum AttachmentSource<'a> {
    /// Fetch the literal URL.
    Url(&'a str),
    /// File bytes and platform metadata.
    File {
        /// Original bytes.
        bytes: &'a [u8],
        /// File name.
        name: &'a str,
        /// Platform media type.
        mime_type: &'a str,
    },
    /// Blob bytes and platform media type.
    Blob {
        /// Original bytes.
        bytes: &'a [u8],
        /// Platform media type.
        mime_type: &'a str,
    },
    /// Bare original bytes.
    Buffer(&'a [u8]),
}
/// Attachment loading failure.
#[derive(Debug)]
pub enum AttachmentError {
    /// Transport or response-body failure.
    Fetch(FetchError),
    /// Non-success response status.
    HttpStatus(u16),
    /// Media type not supported by this loader.
    Unsupported(String),
}
impl fmt::Display for AttachmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch(error) => error.fmt(formatter),
            Self::HttpStatus(status) => {
                write!(formatter, "Failed to fetch attachment: HTTP {status}")
            }
            Self::Unsupported(mime) => write!(formatter, "Unsupported file type: {mime}"),
        }
    }
}
impl std::error::Error for AttachmentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Fetch(error) => Some(error),
            _ => None,
        }
    }
}
/// Load original bytes as an image or text attachment using the supplied transport.
///
/// # Errors
/// Returns transport/body failures, non-success status, or an unsupported media type.
/// Reserved document formats are unsupported until their extractors are available.
pub async fn load_attachment(
    source: AttachmentSource<'_>,
    file_name: Option<&str>,
    fetch: &Fetch,
) -> Result<Attachment, AttachmentError> {
    let file_name = file_name.filter(|name| !name.is_empty());
    let (bytes, name, mime) = resolve_source(source, file_name, fetch).await?;
    let lower_name = name.to_lowercase();
    if [".pdf", ".docx", ".pptx", ".xlsx", ".xls"]
        .iter()
        .any(|suffix| lower_name.ends_with(suffix))
        || [
            "application/pdf",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/vnd.ms-excel",
        ]
        .contains(&mime.as_str())
    {
        return Err(AttachmentError::Unsupported(mime));
    }
    let text = mime.starts_with("text/")
        || [
            ".txt", ".md", ".json", ".xml", ".html", ".css", ".js", ".ts", ".jsx", ".tsx", ".yml",
            ".yaml",
        ]
        .iter()
        .any(|suffix| lower_name.ends_with(suffix));
    let image = mime.starts_with("image/");
    if !image && !text {
        return Err(AttachmentError::Unsupported(mime));
    }
    let content = STANDARD.encode(&bytes);
    Ok(Attachment {
        id: format!(
            "{name}_{}_{}",
            ryu_js::Buffer::new().format(maestro_models::timestamp_now()),
            ryu_js::Buffer::new().format(rand::random::<f64>())
        ),
        r#type: if image {
            AttachmentType::Image
        } else {
            AttachmentType::Document
        },
        file_name: name,
        mime_type: if image || mime.starts_with("text/") {
            mime
        } else {
            "text/plain".into()
        },
        size: bytes.len(),
        preview: image.then(|| content.clone()),
        content,
        extracted_text: (!image).then(|| maestro_text::decode_utf8(&bytes).into_owned()),
    })
}
/// Resolve bytes and metadata without applying format policy.
async fn resolve_source<'a>(
    source: AttachmentSource<'a>,
    name: Option<&str>,
    fetch: &Fetch,
) -> Result<(Cow<'a, [u8]>, String, String), AttachmentError> {
    use futures_util::TryStreamExt;
    let (bytes, detected, mime) = match source {
        AttachmentSource::Url(url) => {
            let response = fetch(HttpRequest {
                method: "GET".into(),
                url: url.into(),
                headers: [].into(),
                body: vec![],
                signal: None,
            })
            .await
            .map_err(AttachmentError::Fetch)?;
            if !(200..300).contains(&response.status) {
                return Err(AttachmentError::HttpStatus(response.status));
            }
            let bytes = response
                .body
                .try_concat()
                .await
                .map_err(AttachmentError::Fetch)?;
            let detected = url
                .rsplit('/')
                .next()
                .filter(|part| !part.is_empty())
                .unwrap_or("document");
            (
                Cow::Owned(bytes),
                detected,
                response
                    .headers
                    .get("content-type")
                    .cloned()
                    .unwrap_or_default(),
            )
        }
        AttachmentSource::File {
            bytes,
            name,
            mime_type,
        } => (Cow::Borrowed(bytes), name, mime_type.into()),
        AttachmentSource::Blob { bytes, mime_type } => {
            (Cow::Borrowed(bytes), "unnamed", mime_type.into())
        }
        AttachmentSource::Buffer(bytes) => (Cow::Borrowed(bytes), "unnamed", String::new()),
    };
    Ok((
        bytes,
        name.unwrap_or(detected).into(),
        if mime.is_empty() {
            "application/octet-stream".into()
        } else {
            mime
        },
    ))
}
