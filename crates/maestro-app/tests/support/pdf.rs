//! Strict PDF oracle records and public loading helpers.
#![cfg(test)]
use super::run;
use base64::{Engine, engine::general_purpose::STANDARD};
use hayro::vello_cpu::Pixmap;
use maestro_app::browser::utils::attachment_utils::{
    Attachment, AttachmentError, AttachmentSource, load_attachment,
};
use maestro_models::{Fetch, HttpResponse};
use serde::{Deserialize, Serialize};
use std::{fmt::Write, path::PathBuf, sync::Arc};

/// One independently recorded document.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Original file selector.
    pub input: DocumentInput,
    /// Native result wrapped by application policy.
    pub expected: DocumentExpected,
}
/// File operand.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentInput {
    /// Original filename.
    pub file: String,
}
/// Selected document result.
#[derive(Deserialize)]
#[serde(tag = "branch", rename_all = "lowercase", deny_unknown_fields)]
pub enum DocumentExpected {
    /// Successful extraction.
    Success {
        /// Exact literal page framing.
        #[serde(rename = "extractedText")]
        extracted_text: String,
        /// Optional decoded first-page baseline.
        preview: Option<Preview>,
    },
    /// Native opening failure.
    Error {
        /// Categorized failure.
        #[serde(rename = "errorKind")]
        error_kind: String,
    },
}
/// Independent PNG observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preview {
    /// Baseline PNG filename.
    pub file: String,
    /// Truncated pixel width.
    pub width: u16,
    /// Truncated pixel height.
    pub height: u16,
}
/// Complete route observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    /// Source metadata.
    pub input: RouteInput,
    /// Returned fields or failure.
    pub expected: RouteExpected,
}
/// Platform or transport metadata.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteInput {
    /// Source kind.
    pub source: String,
    /// File metadata.
    pub name: Option<String>,
    /// Response or platform MIME.
    pub mime: Option<String>,
    /// Literal URL.
    pub url: Option<String>,
    /// Optional filename override.
    pub r#override: Option<String>,
}
/// Returned source-route result.
#[derive(Deserialize)]
#[serde(tag = "branch", rename_all = "lowercase", deny_unknown_fields)]
pub enum RouteExpected {
    /// Complete original fields and native preview.
    Success {
        /// Exact returned attachment except separately decoded preview.
        attachment: Original,
        /// PNG baseline.
        preview: Preview,
    },
    /// Unsupported exact MIME.
    Error {
        /// Recorded error display.
        message: String,
    },
}
/// Expected attachment fields excluding encoded PNG.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Original {
    /// Classification.
    r#type: String,
    /// Selected filename.
    file_name: String,
    /// Canonical MIME.
    mime_type: String,
    /// Original length.
    size: usize,
    /// Original base64 bytes.
    content: String,
    /// Literal page text.
    extracted_text: String,
    /// Checked identifier projection.
    id: String,
}
/// Locate a committed input or independent PNG.
pub fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdf")
        .join(name)
}
/// Read original producer bytes.
pub fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(path(name)).unwrap()
}
/// Read all document observations.
pub fn documents() -> Vec<Document> {
    serde_json::from_str(include_str!("../fixtures/pdf/documents.json")).unwrap()
}
/// Read complete source routes.
pub fn routes() -> Vec<Route> {
    serde_json::from_str(include_str!("../fixtures/pdf/routes.json")).unwrap()
}
/// Load one document through the public owner.
pub fn load(name: &str) -> Result<Attachment, AttachmentError> {
    let data = bytes(name);
    let fetch: Fetch = Arc::new(|_| panic!("local input fetched"));
    run(load_attachment(
        AttachmentSource::Buffer(&data),
        Some(name),
        &fetch,
    ))
}
/// Previews the approved engine rasterizes differently on aarch64. Its SIMD rounding there
/// moves some antialiased edge pixels by at most 2, identically on Linux and macOS, so these
/// documents keep a second exact baseline recorded from the unchanged engine on aarch64.
pub const AARCH64_PREVIEWS: [&str; 3] = [
    "TAMReview.pdf.png",
    "alphatrans.pdf.png",
    "images_1bit_grayscale.pdf.png",
];
/// Name the aarch64 baseline of one preview.
pub fn aarch64_preview(file: &str) -> String {
    format!("{}.aarch64.png", file.trim_end_matches(".png"))
}
/// Name the baseline this target compares a preview with.
fn preview_baseline(file: &str) -> String {
    if cfg!(target_arch = "aarch64") && AARCH64_PREVIEWS.contains(&file) {
        aarch64_preview(file)
    } else {
        file.to_owned()
    }
}
/// Decode the observed standard base64 PNG.
pub fn pixels(encoded: &str) -> Pixmap {
    let data = STANDARD.decode(encoded).unwrap();
    Pixmap::from_png(std::io::Cursor::new(data)).unwrap()
}
/// Compare geometry and actual decoded pixels, not compression bytes.
pub fn check_preview(encoded: Option<&str>, expected: Option<&Preview>) {
    match (encoded, expected) {
        (Some(encoded), Some(expected)) => {
            let actual = pixels(encoded);
            let baseline = Pixmap::from_png(std::io::Cursor::new(bytes(&preview_baseline(
                &expected.file,
            ))))
            .unwrap();
            assert_eq!(
                (actual.width(), actual.height()),
                (expected.width, expected.height)
            );
            assert_eq!(actual.data(), baseline.data());
        }
        (None, None) => {}
        _ => panic!("preview presence differs"),
    }
}
/// Assert one document's exact text, preview or categorized failure.
pub fn check_document(row: &Document) {
    let actual = load(&row.input.file);
    match &row.expected {
        DocumentExpected::Success {
            extracted_text,
            preview,
        } => {
            let attachment = actual.unwrap();
            assert_eq!(attachment.extracted_text.as_ref(), Some(extracted_text));
            check_preview(attachment.preview.as_deref(), preview.as_ref());
        }
        DocumentExpected::Error { error_kind } => {
            assert_eq!(error_kind, "Pdf");
            let error = actual.unwrap_err();
            assert!(matches!(&error, AttachmentError::Pdf(message) if !message.is_empty()));
            assert!(error.to_string().starts_with("Failed to process PDF: "));
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}
/// Build the actual platform source and compare its complete output.
pub fn check_route(row: &Route) {
    let data = bytes("synthetic-wide.pdf");
    let input = &row.input;
    let mime = input.mime.as_deref().unwrap_or_default();
    let platform_mime = mime.to_ascii_lowercase();
    let source = match input.source.as_str() {
        "file" => AttachmentSource::File {
            bytes: &data,
            name: input.name.as_deref().unwrap(),
            mime_type: &platform_mime,
        },
        "blob" => AttachmentSource::Blob {
            bytes: &data,
            mime_type: &platform_mime,
        },
        "buffer" => AttachmentSource::Buffer(&data),
        "url" => AttachmentSource::Url(input.url.as_deref().unwrap()),
        _ => panic!("unknown route source"),
    };
    let url = input.url.clone();
    let mime = mime.to_owned();
    let response_data = data.clone();
    let fetch: Fetch = Arc::new(move |request| {
        assert_eq!(Some(&request.url), url.as_ref());
        assert_eq!(request.method, "GET");
        let data = response_data.clone();
        let mime = mime.clone();
        Box::pin(async move {
            Ok(HttpResponse {
                status: 200,
                status_text: "OK".into(),
                headers: [("content-type".into(), mime)].into(),
                body: Box::pin(futures_util::stream::once(async move { Ok(data) })),
            })
        })
    });
    let actual = run(load_attachment(source, input.r#override.as_deref(), &fetch));
    match &row.expected {
        RouteExpected::Success {
            attachment,
            preview,
        } => {
            let mut actual = actual.unwrap();
            check_preview(actual.preview.as_deref(), Some(preview));
            actual.preview = None;
            let observed = super::attachment_observation::observed(Ok(actual));
            assert_eq!(
                observed["attachment"],
                serde_json::to_value(attachment).unwrap()
            );
        }
        RouteExpected::Error { message } => {
            assert_eq!(format!("Error: {}", actual.unwrap_err()), *message);
        }
    }
}

/// Build a controlled PDF with valid stream lengths and cross references.
pub fn synthetic(content: &str, inherited: bool) -> Vec<u8> {
    let geometry = "/MediaBox [0 0 100.2 250.5] /CropBox [0 0 100.2 250.5]";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!(
            "<< /Type /Pages /Count 1 /Kids [3 0 R] {} >>",
            if inherited { geometry } else { "" }
        ),
        format!(
            "<< /Type /Page /Parent 2 0 R {} /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> /Contents 4 0 R >>",
            if inherited { "" } else { geometry }
        ),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
    ];
    let mut pdf = "%PDF-1.7\n".to_owned();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        write!(pdf, "{} 0 obj\n{object}\nendobj\n", index + 1).unwrap();
    }
    let xref = pdf.len();
    pdf.push_str("xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets {
        writeln!(pdf, "{offset:010} 00000 n ").unwrap();
    }
    write!(
        pdf,
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    )
    .unwrap();
    pdf.into_bytes()
}

/// One decoded-show observation through the public loader.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextShow {
    /// Selecting regression name.
    pub test: String,
    /// Literal filename override.
    name: String,
    /// Real content producer.
    input: TextShowInput,
    /// Independently recorded full page wrapper.
    expected_text: String,
}
/// Authored content or a committed binary PDF.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum TextShowInput {
    /// Build content with valid stream lengths and cross references.
    Content {
        /// PDF content operators.
        content: String,
    },
    /// Read an existing encoded producer.
    File {
        /// Committed PDF filename.
        file: String,
    },
}
/// Read the complete text-show corpus.
pub fn text_shows() -> Vec<TextShow> {
    serde_json::from_str(include_str!("../fixtures/pdf/text-shows.json")).unwrap()
}
/// Enumerate binary inputs consumed by the text-show corpus.
pub fn text_show_files() -> Vec<String> {
    text_shows()
        .into_iter()
        .filter_map(|row| match row.input {
            TextShowInput::File { file } => Some(file),
            TextShowInput::Content { .. } => None,
        })
        .collect()
}
/// Assert each selected row and return the attachments for extra pixel checks.
pub fn check_text_shows(test: &str, count: usize) -> Vec<Attachment> {
    let rows: Vec<_> = text_shows()
        .into_iter()
        .filter(|row| row.test == test)
        .collect();
    assert_eq!(rows.len(), count, "selected corpus row count");
    rows.into_iter()
        .map(|row| {
            let data = match row.input {
                TextShowInput::Content { content } => synthetic(&content, false),
                TextShowInput::File { file } => bytes(&file),
            };
            let fetch: Fetch = Arc::new(|_| panic!("local input fetched"));
            let attachment = run(load_attachment(
                AttachmentSource::Buffer(&data),
                Some(&row.name),
                &fetch,
            ))
            .unwrap();
            assert_eq!(
                attachment.extracted_text.as_deref(),
                Some(row.expected_text.as_str()),
                "{}",
                row.name
            );
            attachment
        })
        .collect()
}
