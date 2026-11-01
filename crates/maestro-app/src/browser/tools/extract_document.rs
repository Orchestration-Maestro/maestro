//! Executable document download and text extraction.
use super::prompts::EXTRACT_DOCUMENT_DESCRIPTION;
use crate::browser::utils::attachment_utils::{Attachment, AttachmentSource, load_attachment};
use crate::utils::lock::read;
use futures_util::TryStreamExt;
use maestro_agent::{AgentTool, AgentToolResult, SharedAgentTool};
use maestro_models::SharedHandle;
use maestro_models::{
    BoxFuture, Cancellation, DiagnosticErrorInfo, Fetch, FetchError, HttpRequest, Tool,
    default_fetch,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Required document URL argument.
#[derive(Deserialize, Serialize)]
pub struct ExtractDocumentParams {
    /// Authored absolute document URL.
    pub url: String,
}
/// Extracted document metadata returned with the text block.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractDocumentResult {
    /// Extracted text.
    pub extracted_text: String,
    /// Document format tag.
    pub format: String,
    /// Attachment-selected filename.
    pub file_name: String,
    /// Attachment byte length.
    pub size: usize,
}
/// Executable declaration with its independently owned proxy setting.
pub struct ExtractDocumentTool {
    /// Shared mutable tool for registration.
    pub tool: SharedAgentTool,
    /// Proxy prefix read after a direct CORS failure.
    pub cors_proxy_url: Arc<RwLock<Option<String>>>,
}
/// Native attachment operation retained by the executable callback.
#[cfg(not(target_arch = "wasm32"))]
type LoadAttachment = SharedHandle<
    dyn Fn(Vec<u8>, String) -> BoxFuture<Result<Attachment, DiagnosticErrorInfo>> + Send + Sync,
>;
/// Browser-local attachment operation retained by the executable callback.
#[cfg(target_arch = "wasm32")]
type LoadAttachment =
    SharedHandle<dyn Fn(Vec<u8>, String) -> BoxFuture<Result<Attachment, DiagnosticErrorInfo>>>;
/// Create a tool using the supplied transport or the default HTTP adapter.
#[must_use]
pub fn create_extract_document_tool(fetch: Option<Fetch>) -> ExtractDocumentTool {
    let fetch = fetch.unwrap_or_else(default_fetch);
    let loader_fetch = Arc::clone(&fetch);
    build_tool(
        fetch,
        SharedHandle::new(move |bytes, name| {
            let fetch = Arc::clone(&loader_fetch);
            Box::pin(async move {
                load_attachment(AttachmentSource::Buffer(&bytes), Some(&name), &fetch)
                    .await
                    .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
            })
        }),
    )
}
/// Construct the typed executable with an owned attachment operation.
fn build_tool(fetch: Fetch, load: LoadAttachment) -> ExtractDocumentTool {
    let cors_proxy_url = Arc::new(RwLock::new(None));
    let proxy = Arc::clone(&cors_proxy_url);
    let tool = AgentTool::typed(
        Tool {
            name: "extract_document".into(),
            description: EXTRACT_DOCUMENT_DESCRIPTION.into(),
            parameters: maestro_models::ParameterSchema::authored(
                serde_json::json!({"type":"object","required":["url"],"properties":{"url":{"type":"string","description":"URL of the document to extract text from (PDF, DOCX, XLSX, or PPTX)"}}}),
                [
                    (String::new(), maestro_models::SchemaKind::Object),
                    (
                        "/properties/url".to_owned(),
                        maestro_models::SchemaKind::String,
                    ),
                ],
            ),
        },
        "Extract Document".into(),
        SharedHandle::new(move |_, args: ExtractDocumentParams, signal, _| {
            let (fetch, load, proxy) = (
                Arc::clone(&fetch),
                SharedHandle::clone(&load),
                Arc::clone(&proxy),
            );
            Box::pin(async move { execute(args, signal, &fetch, &load, &proxy).await })
        }),
    );
    ExtractDocumentTool {
        tool: SharedHandle::new(RwLock::new(tool)),
        cors_proxy_url,
    }
}
/// Shared native SDK tool instance.
#[cfg(not(target_arch = "wasm32"))]
pub static EXTRACT_DOCUMENT_TOOL: std::sync::LazyLock<ExtractDocumentTool> =
    std::sync::LazyLock::new(|| create_extract_document_tool(None));
#[cfg(target_arch = "wasm32")]
std::thread_local! {
    /// Shared browser-local SDK tool instance.
    pub static EXTRACT_DOCUMENT_TOOL: ExtractDocumentTool = create_extract_document_tool(None);
}
/// Download then project the loaded attachment.
async fn execute(
    args: ExtractDocumentParams,
    signal: Option<Cancellation>,
    fetch: &Fetch,
    load: &LoadAttachment,
    proxy: &RwLock<Option<String>>,
) -> Result<AgentToolResult<ExtractDocumentResult>, DiagnosticErrorInfo> {
    if signal.as_ref().is_some_and(Cancellation::is_aborted) {
        return Err(DiagnosticErrorInfo::error("Extract document aborted"));
    }
    let url = args.url.trim_matches(maestro_text::is_whitespace_scalar);
    if url.is_empty() {
        return Err(DiagnosticErrorInfo::error("URL is required"));
    }
    if url::Url::parse(url).is_err() {
        return Err(DiagnosticErrorInfo::error(format!("Invalid URL: {url}")));
    }
    let bytes = match download(url, fetch, signal.as_ref()).await {
        Ok(bytes) => bytes,
        Err(error) => {
            if !crate::browser::utils::proxy_utils::is_cors_error(&error) {
                return Err(error);
            }
            let proxy = read(proxy).clone();
            let Some(proxy) = proxy.filter(|value| !value.is_empty()) else {
                return Err(DiagnosticErrorInfo::error(NO_PROXY));
            };
            download(&format!("{proxy}{}", crate::utils::uri::encode_uri_component(url)), fetch, signal.as_ref()).await.map_err(|error| DiagnosticErrorInfo::error(format!("TELL USER: Unable to fetch the document due to CORS restrictions.\n\nTried with proxy but it also failed: {}\n\n{MANUAL_ATTACHMENT}", error.message)))?
        }
    };
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let mut name = path
        .rsplit('/')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or("document")
        .to_owned();
    if url.starts_with("https://arxiv.org/") {
        name.push_str(".pdf");
    }
    let attachment = load(bytes, name).await?;
    let text = attachment.extracted_text.filter(|text| !text.is_empty()).ok_or_else(|| DiagnosticErrorInfo::error("Document format not supported. Supported formats:\n- PDF (.pdf)\n- Word (.docx)\n- Excel (.xlsx, .xls)\n- PowerPoint (.pptx)"))?;
    Ok(AgentToolResult::text_result(
        text.clone(),
        Some(ExtractDocumentResult {
            extracted_text: text,
            format: attachment_format(&attachment.mime_type).into(),
            file_name: attachment.file_name,
            size: attachment.size,
        }),
    ))
}

/// Remediation shared by download and proxy failures.
const MANUAL_ATTACHMENT: &str = "INSTRUCT USER: Please download the file manually and attach it to your message using the attachment button (paperclip icon) in the message input area. I can then extract the text from the attached file.";

/// Enforce the declared or fully collected byte limit.
fn check_size(size: f64) -> Result<(), DiagnosticErrorInfo> {
    if size > 50.0 * 1024.0 * 1024.0 {
        return Err(DiagnosticErrorInfo::error(format!(
            "Document is too large ({}MB). Maximum supported size is 50MB.",
            ryu_js::Buffer::new().format_to_fixed(size / 1024.0 / 1024.0, 1)
        )));
    }
    Ok(())
}

/// Collect one download attempt without applying retry policy.
async fn download_work(
    url: &str,
    fetch: &Fetch,
    signal: Option<&Cancellation>,
) -> Result<Vec<u8>, DiagnosticErrorInfo> {
    let response = fetch(HttpRequest {
        method: "GET".into(),
        url: url.into(),
        headers: [].into(),
        body: vec![],
        signal: signal.cloned(),
    })
    .await
    .map_err(fetch_error)?;
    if !(200..300).contains(&response.status) {
        return Err(DiagnosticErrorInfo::error(format!(
            "TELL USER: Unable to download the document ({} {}). The site likely blocks automated downloads.\n\n{MANUAL_ATTACHMENT}",
            response.status, response.status_text
        )));
    }
    if let Some(size) = response
        .headers
        .get("content-length")
        .and_then(|text| crate::browser::utils::parse_decimal_prefix(text).ok())
    {
        check_size(size)?;
    }
    let bytes = response.body.try_concat().await.map_err(fetch_error)?;
    check_size(num_traits::ToPrimitive::to_f64(&bytes.len()).unwrap_or(f64::INFINITY))?;
    Ok(bytes)
}
/// Keep connection diagnostics owned across the tool boundary.
fn fetch_error(error: FetchError) -> DiagnosticErrorInfo {
    match error {
        FetchError::Connection(cause) => cause,
        other => DiagnosticErrorInfo::error(other.to_string()),
    }
}

/// Configuration guidance when no proxy is available.
const NO_PROXY: &str = r#"TELL USER: Unable to fetch the document due to CORS restrictions (the server blocks requests from browser extensions).

To fix this, you need to configure a CORS proxy in Maestro settings:
1. Open Maestro settings
2. Find "CORS Proxy URL" setting
3. Enter a proxy URL like: https://corsproxy.io/?
4. Save and try again

Alternatively, download the file manually and attach it to your message using the attachment button (paperclip icon)."#;

/// Select the emitted format from canonical attachment MIME text.
fn attachment_format(mime: &str) -> &'static str {
    match mime {
        value if value.contains("pdf") => "pdf",
        value if value.contains("wordprocessingml") => "docx",
        value if value.contains("spreadsheetml") || value.contains("ms-excel") => "xlsx",
        value if value.contains("presentationml") => "pptx",
        _ => "unknown",
    }
}

/// Race the entire download, including its body, against cancellation only.
async fn download(
    url: &str,
    fetch: &Fetch,
    signal: Option<&Cancellation>,
) -> Result<Vec<u8>, DiagnosticErrorInfo> {
    use maestro_models::providers::http::{Raced, race};
    match race(download_work(url, fetch, signal), None, signal).await {
        Raced::Done(result) => result,
        Raced::Cancelled | Raced::TimedOut => Err(fetch_error(FetchError::Aborted)),
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests;
