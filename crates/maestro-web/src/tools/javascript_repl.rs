//! JavaScript execution cards and browser-local executable registration.
use super::{
    renderer_registry::get_text_output,
    renderers::{
        card_content::{Caption, CardOutput, field},
        collapsible_card::{Body, CollapsibleCard, Download, Phase},
        default_renderer::status,
    },
    types::{ToolRenderResult, ToolRenderer, ToolResultMessageView, ToolUiError},
};
use crate::{Attachment, components::console_block::ConsoleVariant};
use maestro_app::{
    JsonObject,
    browser::{
        tools::javascript_repl::{JavaScriptReplParams, JavaScriptReplToolResult},
        utils::attachment_utils::AttachmentType,
    },
};
use serde_json::Value;

#[cfg(target_arch = "wasm32")]
use crate::{
    browser::sandbox_value::BrowserSandboxValue, components::sandboxed_iframe::BrowserSandboxHost,
};
#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Each executable invocation obtains its own hidden sandbox.
    pub static JAVASCRIPT_REPL_TOOL: std::cell::LazyCell<maestro_app::browser::tools::javascript_repl::JavaScriptReplTool<BrowserSandboxValue>> = std::cell::LazyCell::new(|| maestro_app::create_javascript_repl_tool(std::sync::Arc::new(BrowserSandboxHost)));
}
/// Stateless JavaScript execution renderer.
pub struct JavaScriptReplRenderer;
/// Shared JavaScript execution renderer value.
pub const JAVASCRIPT_REPL_RENDERER: JavaScriptReplRenderer = JavaScriptReplRenderer;
impl<C: CardOutput> ToolRenderer<JsonObject, Value, C> for JavaScriptReplRenderer {
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_, Value>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        let fields = params
            .map(|_| {
                Ok::<_, ToolUiError>((
                    field(params, "title")?.map(std::borrow::Cow::into_owned),
                    field(params, "code")?.map(std::borrow::Cow::into_owned),
                ))
            })
            .transpose()?;
        let files = if params.is_some() {
            returned_files(result.as_ref().and_then(ToolResultMessageView::details))?
        } else {
            Vec::new()
        };
        Ok(prepare(fields, result.as_ref(), is_streaming, files))
    }
}
impl<C: CardOutput> ToolRenderer<JavaScriptReplParams, JavaScriptReplToolResult, C>
    for JavaScriptReplRenderer
{
    fn render(
        &self,
        params: Option<&JavaScriptReplParams>,
        result: Option<ToolResultMessageView<'_, JavaScriptReplToolResult>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        let fields = params.map(|p| {
            (
                Some(p.title.clone()).filter(|s| !s.is_empty()),
                Some(p.code.clone()).filter(|s| !s.is_empty()),
            )
        });
        let files = if params.is_some() {
            result
                .as_ref()
                .and_then(ToolResultMessageView::details)
                .and_then(|d| d.files.as_ref())
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(i, f)| attachment(i, &f.file_name, &f.mime_type, f.size, &f.content_base64))
                .collect()
        } else {
            Vec::new()
        };
        Ok(prepare(fields, result.as_ref(), is_streaming, files))
    }
}
/// Select code and console branches for any representation without retaining input borrows.
fn prepare<D, C: CardOutput>(
    fields: Option<(Option<String>, Option<String>)>,
    result: Option<&ToolResultMessageView<'_, D>>,
    streaming: bool,
    attachments: Vec<Download>,
) -> ToolRenderResult<C> {
    let state = status(result, streaming);
    let (caption, body) = fields.map_or_else(
        || {
            (
                Caption::Localized("Preparing JavaScript...", String::new()),
                None,
            )
        },
        |(title, code)| {
            let caption = title.map_or_else(
                || Caption::Localized("Executing JavaScript", String::new()),
                Caption::Literal,
            );
            let output = get_text_output(result);
            let body = Body {
                phase: if result.is_some() {
                    Phase::Result
                } else {
                    Phase::Pending
                },
                diff: None,
                error: None,
                fallback: None,
                url: None,
                code: if result.is_some() {
                    Some((code.unwrap_or_default(), "javascript"))
                } else {
                    code.map(|c| (c, "javascript"))
                },
                console: (!output.is_empty()).then_some((
                    output,
                    if result.is_some_and(ToolResultMessageView::is_error) {
                        ConsoleVariant::Error
                    } else {
                        ConsoleVariant::Default
                    },
                )),
            };
            (caption, Some(body))
        },
    );
    CollapsibleCard {
        state,
        glyph: "Code",
        caption,
        body,
        attachments,
        plain_phase: None,
        error: None,
    }
    .result::<JavaScriptReplRenderer>()
}
/// Project one download without changing its original encoded payload.
fn attachment(index: usize, name: &str, mime: &str, size: usize, content: &str) -> Download {
    let image = mime.starts_with("image/");
    let text = mime.starts_with("text/")
        || matches!(mime, "application/json" | "application/javascript")
        || mime.contains("xml");
    Download {
        text,
        attachment: Attachment {
            id: format!(
                "repl-{}-{index}",
                crate::browser::attachment_content::identity().value()
            ),
            r#type: if image {
                AttachmentType::Image
            } else {
                AttachmentType::Document
            },
            file_name: if name.is_empty() {
                format!("file-{index}")
            } else {
                name.into()
            },
            mime_type: if mime.is_empty() {
                "application/octet-stream".into()
            } else {
                mime.into()
            },
            size,
            content: content.into(),
            preview: image.then(|| content.into()),
            extracted_text: None,
        },
    }
}

/// Project the selected returned-file list once per preparation.
fn returned_files(details: Option<&Value>) -> Result<Vec<Download>, ToolUiError> {
    let Some(files) = details
        .and_then(|d| d.get("files"))
        .filter(|v| maestro_app::is_truthy(v))
    else {
        return Ok(Vec::new());
    };
    let files = files
        .as_array()
        .ok_or_else(|| ToolUiError::Presentation("Returned files must be an array".into()))?;
    Ok(files
        .iter()
        .enumerate()
        .map(|(index, file)| raw_attachment(index, file))
        .collect())
}
/// Read the returned-file boundary without retaining its JSON carrier.
fn raw_attachment(index: usize, file: &Value) -> Download {
    attachment(
        index,
        file.get("fileName")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        file.get("mimeType")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        file.get("size")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or_default(),
        file.get("contentBase64")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
}
