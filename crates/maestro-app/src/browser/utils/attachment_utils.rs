//! Image and document attachment records and loading.
use crate::browser::docx_preview::{DocxError, OpenXmlPackage, Options, decode_text};
use crate::browser::ssf::{self, Ssf, WorkbookError};
use base64::{Engine, engine::general_purpose::STANDARD};
use maestro_models::{Fetch, FetchError, HttpRequest};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fmt};

mod docx;
/// Native PDF text extraction and page rendering.
pub mod pdf;

/// Model-facing attachment classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    /// Original image bytes.
    Image,
    /// Document with optional extracted text.
    Document,
}
/// Original attachment bytes with derived presentation data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
/// A live attachment retained independently of its containing collection.
pub type SharedAttachment = std::sync::Arc<std::sync::RwLock<Attachment>>;

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
    /// Native PDF processing diagnostic.
    Pdf(String),
    /// Presentation package processing failure.
    Pptx(DocxError),
    /// Word package processing failure.
    Docx(DocxError),
    /// Spreadsheet workbook processing failure.
    Excel(WorkbookError),
}
impl fmt::Display for AttachmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch(error) => error.fmt(formatter),
            Self::HttpStatus(status) => {
                write!(formatter, "Failed to fetch attachment: HTTP {status}")
            }
            Self::Unsupported(mime) => write!(formatter, "Unsupported file type: {mime}"),
            Self::Pdf(error) => write!(formatter, "Failed to process PDF: {error}"),
            Self::Pptx(error) => write!(formatter, "Failed to process PPTX: {error}"),
            Self::Docx(error) => write!(formatter, "Failed to process DOCX: {error}"),
            Self::Excel(error) => write!(formatter, "Failed to process Excel: {error}"),
        }
    }
}
impl std::error::Error for AttachmentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Fetch(error) => Some(error),
            Self::Pptx(error) | Self::Docx(error) => Some(error),
            Self::Excel(error) => Some(error),
            _ => None,
        }
    }
}
/// Load original bytes as an image or document attachment.
///
/// # Errors
/// Returns transport/body failures, non-success status, unsupported media types,
/// or document processing failures.
pub async fn load_attachment(
    source: AttachmentSource<'_>,
    file_name: Option<&str>,
    fetch: &Fetch,
) -> Result<Attachment, AttachmentError> {
    let file_name = file_name.filter(|name| !name.is_empty());
    let (bytes, name, mime) = resolve_source(source, file_name, fetch).await?;
    let size = bytes.len();
    let content = STANDARD.encode(&bytes);
    let (mime, extracted_text, preview) =
        if mime == "application/pdf" || name.to_ascii_lowercase().ends_with(".pdf") {
            let (text, preview) = pdf::process_pdf(bytes.into_owned(), &name)?;
            ("application/pdf".into(), Some(text), preview)
        } else {
            let (mime, text) = attachment_text(&bytes, &name, mime)?;
            let preview = text.is_none().then(|| content.clone());
            (mime, text, preview)
        };
    let image = extracted_text.is_none();
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
        mime_type: mime,
        size,
        preview,
        content,
        extracted_text,
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

/// Canonical Word media type.
const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

/// Canonical presentation media type.
const PPTX_MIME: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

/// Extract selected slide text without admitting whole-part XML.
fn process_pptx(bytes: &[u8], file_name: &str) -> Result<String, DocxError> {
    let package = OpenXmlPackage::open(bytes, Options::default())?;
    let mut output = String::new();
    let mut append = |text: &str| append_document_text(&mut output, text);
    append(&format!("<pptx filename=\"{file_name}\">"))?;
    for (position, (name, _)) in pptx_parts(&package, "ppt/slides/slide").iter().enumerate() {
        let source = package.text_exact(name)?.unwrap_or_default();
        let (matched, runs) = pptx_runs(&source);
        if matched {
            append(&format!("\n<slide number=\"{}\">", position + 1))?;
            if !runs.is_empty() {
                append("\n")?;
                append(&runs.join("\n"))?;
            }
            append("\n</slide>")?;
        }
    }
    let notes = pptx_parts(&package, "ppt/notesSlides/notesSlide");
    if !notes.is_empty() {
        append("\n<notes>")?;
        for (name, digits) in notes {
            let source = package.text_exact(name)?.unwrap_or_default();
            let (_, runs) = pptx_runs(&source);
            if !runs.is_empty() {
                append(&format!("\n[Slide {digits} notes]: "))?;
                append(&runs.join(" "))?;
            }
        }
        append("\n</notes>")?;
    }
    append("\n</pptx>")?;
    Ok(output)
}

/// Stable numeric ordering retains admitted names and original digit spelling.
fn pptx_parts<'a>(package: &'a OpenXmlPackage, prefix: &str) -> Vec<(&'a str, &'a str)> {
    let mut parts: Vec<_> = package
        .names()
        .filter_map(|name| {
            let stem = name.strip_suffix(".xml")?;
            let (_, digits) = stem.rsplit_once(prefix)?;
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            Some((name, digits, digits.parse::<f64>().unwrap_or(f64::INFINITY)))
        })
        .collect();
    parts.sort_by(|left, right| left.2.total_cmp(&right.2));
    parts
        .into_iter()
        .map(|(name, digits, _)| (name, digits))
        .collect()
}

/// Scan literal nonempty text runs monotonically, retaining raw match presence.
fn pptx_runs(mut source: &str) -> (bool, Vec<String>) {
    let mut matched = false;
    let mut runs = Vec::new();
    while let Some(start) = source.find("<a:t") {
        source = &source[start + 4..];
        let Some(end) = source.find('>') else { break };
        source = &source[end + 1..];
        let Some(end) = source.find('<') else { break };
        let text = &source[..end];
        source = &source[end..];
        if !text.is_empty() && source.starts_with("</a:t>") {
            matched = true;
            if text.chars().any(|c| !maestro_text::is_whitespace_scalar(c)) {
                runs.push(decode_text(text));
            }
            source = &source[6..];
        }
    }
    (matched, runs)
}

/// Select document policy before image/text fallback and derive document text.
fn attachment_text(
    bytes: &[u8],
    name: &str,
    mime: String,
) -> Result<(String, Option<String>), AttachmentError> {
    let lower_name = name.to_lowercase();
    if mime == DOCX_MIME
        || name
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("docx"))
    {
        return Ok((
            DOCX_MIME.into(),
            Some(docx::process_docx(bytes, name).map_err(AttachmentError::Docx)?),
        ));
    }
    if mime == PPTX_MIME
        || name
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("pptx"))
    {
        return Ok((
            PPTX_MIME.into(),
            Some(process_pptx(bytes, name).map_err(AttachmentError::Pptx)?),
        ));
    }
    if [".xlsx", ".xls"]
        .iter()
        .any(|suffix| lower_name.ends_with(suffix))
        || [
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/vnd.ms-excel",
        ]
        .contains(&mime.as_str())
    {
        let mime = if mime.starts_with("application/vnd") {
            mime
        } else {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into()
        };
        return Ok((
            mime,
            Some(process_excel(bytes, name).map_err(AttachmentError::Excel)?),
        ));
    }
    if mime.starts_with("image/") {
        return Ok((mime, None));
    }
    if !mime.starts_with("text/")
        && ![
            ".txt", ".md", ".json", ".xml", ".html", ".css", ".js", ".ts", ".jsx", ".tsx", ".yml",
            ".yaml",
        ]
        .iter()
        .any(|suffix| lower_name.ends_with(suffix))
    {
        return Err(AttachmentError::Unsupported(mime));
    }
    let mime = if mime.starts_with("text/") {
        mime
    } else {
        "text/plain".into()
    };
    Ok((mime, Some(maestro_text::decode_utf8(bytes).into_owned())))
}

/// Extract ordered workbook sheets as formatted CSV inside literal wrappers.
fn process_excel(bytes: &[u8], name: &str) -> Result<String, WorkbookError> {
    let mut ssf = Ssf::default();
    let mut workbook = ssf::read(bytes, name, &mut ssf)?;
    let mut output = String::new();
    let mut append =
        |text: &str| append_extracted_text(&mut output, text).map_err(WorkbookError::Output);
    append(&format!("<excel filename=\"{name}\">"))?;
    for (index, (name, sheet)) in workbook.sheets.iter_mut().enumerate() {
        let csv = sheet
            .as_mut()
            .map_or_else(|_| String::new(), |sheet| ssf::sheet_to_csv(sheet, &ssf));
        append(&format!(
            "\n<sheet name=\"{name}\" index=\"{}\">\n",
            index + 1
        ))?;
        append(&csv)?;
        append("\n</sheet>")?;
    }
    append("\n</excel>")?;
    Ok(output)
}

/// Append one extracted document piece under the shared output bound.
fn append_document_text(output: &mut String, text: &str) -> Result<(), DocxError> {
    append_extracted_text(output, text).map_err(DocxError::Output)
}

/// Append extracted output only while it fits the document text bound.
fn append_extracted_text(output: &mut String, text: &str) -> Result<(), String> {
    const LIMIT: usize = 512 << 20;
    if text.len() > LIMIT.saturating_sub(output.len()) {
        return Err("extracted text exceeds 512 MiB".into());
    }
    output
        .try_reserve(text.len())
        .map_err(|error| error.to_string())?;
    output.push_str(text);
    Ok(())
}
