//! Document-extraction cards without document execution or parsing.
use super::{
    renderer_registry::get_text_output,
    renderers::{
        card_content::{Caption, CardOutput, field},
        collapsible_card::{Body, CollapsibleCard, Phase},
        default_renderer::status,
    },
    types::{ToolRenderResult, ToolRenderer, ToolResultMessageView, ToolUiError},
};
use crate::components::console_block::ConsoleVariant;
use maestro_app::{
    JsonObject,
    browser::tools::extract_document::{ExtractDocumentParams, ExtractDocumentResult},
};
use serde_json::Value;

/// Stateless document-extraction renderer.
pub struct ExtractDocumentRenderer;
/// Shared document-extraction renderer value.
pub const EXTRACT_DOCUMENT_RENDERER: ExtractDocumentRenderer = ExtractDocumentRenderer;
impl<C: CardOutput> ToolRenderer<JsonObject, Value, C> for ExtractDocumentRenderer {
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_, Value>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        let url = if let Some(params) = params {
            if result.is_some() {
                field(Some(params), "url")?.map(std::borrow::Cow::into_owned)
            } else {
                Some(match params.get("url").filter(|v| !v.is_null()) {
                    Some(Value::String(s)) => s.clone(),
                    Some(value) => maestro_app::compact_json(value)
                        .map_err(|e| ToolUiError::Presentation(e.to_string()))?,
                    None => String::new(),
                })
            }
        } else {
            None
        };
        let caption = if params.is_some() {
            result
                .as_ref()
                .map(|r| caption(r.details(), r.is_error()))
                .transpose()?
        } else {
            None
        };
        Ok(prepare(
            params.is_some(),
            url,
            caption,
            result.as_ref(),
            status(result.as_ref(), is_streaming),
        ))
    }
}
impl<C: CardOutput> ToolRenderer<ExtractDocumentParams, ExtractDocumentResult, C>
    for ExtractDocumentRenderer
{
    fn render(
        &self,
        params: Option<&ExtractDocumentParams>,
        result: Option<ToolResultMessageView<'_, ExtractDocumentResult>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        use num_traits::ToPrimitive as _;
        let url = params
            .map(|p| p.url.clone())
            .filter(|s| result.is_none() || !s.is_empty());
        let caption = if params.is_some() {
            result.as_ref().map(|r| match r.details() {
                Some(d) if r.is_error() => failure(&d.file_name),
                Some(d) => success(
                    &d.file_name,
                    &d.format,
                    d.size.to_f64().unwrap_or(f64::INFINITY),
                ),
                None => generic(r.is_error()).into(),
            })
        } else {
            None
        };
        Ok(prepare(
            params.is_some(),
            url,
            caption,
            result.as_ref(),
            status(result.as_ref(), is_streaming),
        ))
    }
}
/// Read only metadata selected by the reached result branch.
fn caption(details: Option<&Value>, error: bool) -> Result<String, ToolUiError> {
    let Some(details) = details.filter(|d| maestro_app::is_truthy(d)) else {
        return Ok(generic(error).into());
    };
    let name = details
        .get("fileName")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if error {
        return Ok(failure(name));
    }
    let format = details
        .get("format")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolUiError::Presentation("Document format must be text".into()))?;
    let size = details
        .get("size")
        .and_then(Value::as_f64)
        .ok_or_else(|| ToolUiError::Presentation("Document size must be a number".into()))?;
    Ok(success(name, format, size))
}
/// Fixed result caption when metadata is absent.
fn generic(error: bool) -> &'static str {
    if error {
        "Failed to extract document"
    } else {
        "Extracted text from document"
    }
}
/// Error metadata reads neither format nor size.
fn failure(name: &str) -> String {
    format!(
        "Failed to extract {}",
        if name.is_empty() { "document" } else { name }
    )
}
/// Preserve Unicode uppercase and fixed decimal rounding in metadata.
fn success(name: &str, format: &str, size: f64) -> String {
    format!(
        "Extracted text from {name} ({}, {}KB)",
        format.to_uppercase(),
        ryu_js::Buffer::new().format_to_fixed(size / 1024., 1)
    )
}
/// Select literal output and status for any representation without retaining native result borrows.
fn prepare<D, C: CardOutput>(
    params: bool,
    url: Option<String>,
    caption: Option<String>,
    result: Option<&ToolResultMessageView<'_, D>>,
    state: super::renderer_registry::ToolHeaderState,
) -> ToolRenderResult<C> {
    let caption = caption.unwrap_or_else(|| {
        if params {
            "Extracting document..."
        } else {
            "Preparing extraction..."
        }
        .into()
    });
    let body = params.then(|| {
        let output = get_text_output(result);
        let error = result.is_some_and(ToolResultMessageView::is_error);
        let (code, console) = match (error, output.is_empty()) {
            (_, true) => (None, None),
            (true, false) => (None, Some((output, ConsoleVariant::Error))),
            (false, false) => (Some((output, "plaintext")), None),
        };
        Body {
            phase: if result.is_some() {
                Phase::Result
            } else {
                Phase::Pending
            },
            diff: None,
            error: None,
            fallback: None,
            url,
            code,
            console,
        }
    });
    CollapsibleCard {
        state,
        glyph: "FileText",
        caption: Caption::Literal(caption),
        body,
        attachments: Vec::new(),
        plain_phase: None,
        error: None,
    }
    .result::<ExtractDocumentRenderer>()
}
