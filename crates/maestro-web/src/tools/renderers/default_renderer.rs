//! Default browser tool input and output preparation.
use crate::tools::types::ToolUiError;
#[cfg(target_arch = "wasm32")]
mod dom;
#[cfg(target_arch = "wasm32")]
mod listing;
use super::super::{
    renderer_registry::{ToolHeaderState, get_text_output},
    types::ToolResultMessageView,
};
use crate::tools::types::{ToolRenderResult, ToolRenderer};
use serde_json::Value;

/// Choose the result's status before invoking any input formatter.
pub(crate) fn status<D>(
    result: Option<&ToolResultMessageView<'_, D>>,
    streaming: bool,
) -> ToolHeaderState {
    match result {
        Some(result) if result.is_error() => ToolHeaderState::Error,
        None if streaming => ToolHeaderState::Inprogress,
        Some(_) | None => ToolHeaderState::Complete,
    }
}
/// Format a truthy input, parsing a string once and preserving array structure.
pub(crate) fn format_params(value: &Value) -> Result<Option<String>, ToolUiError> {
    if !maestro_app::is_truthy(value) {
        return Ok(None);
    }
    let parsed = value
        .as_str()
        .and_then(|text| serde_json::from_str::<Value>(text).ok());
    pretty(parsed.as_ref().unwrap_or(value)).map(Some)
}
/// Use the shared canonical JSON writer.
fn pretty(value: &Value) -> Result<String, ToolUiError> {
    maestro_app::pretty_json(value).map_err(|error| ToolUiError::Presentation(error.to_string()))
}
/// One literal listing and its selected language.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) struct Listing {
    /// Original displayed and copied text.
    pub(crate) code: String,
    /// Explicit syntax label.
    pub(crate) language: &'static str,
}
/// Branch-selected header and independently retained subsections.
#[cfg(any(target_arch = "wasm32", test))]
struct Prepared {
    /// Literal, untranslated tool caption.
    caption: &'static str,
    /// Input listing, including an explicitly formatted empty input without a result.
    input: Option<Listing>,
    /// Output listing whenever a result exists.
    output: Option<Listing>,
}
/// Select visible branches after input formatting and raw-output observation.
#[cfg(any(target_arch = "wasm32", test))]
fn prepare(
    params: Option<&str>,
    output: Option<&str>,
    state: ToolHeaderState,
    no_output: &str,
) -> Result<Prepared, ToolUiError> {
    let caption = if output.is_some() {
        "Tool Call"
    } else if params.is_none() {
        "Preparing tool..."
    } else if state == ToolHeaderState::Inprogress && matches!(params, Some("" | "{}" | "null")) {
        "Preparing tool parameters..."
    } else {
        "Tool Call"
    };
    let input = params
        .filter(|text| caption == "Tool Call" && (output.is_none() || !text.is_empty()))
        .map(|code| Listing {
            code: code.into(),
            language: "json",
        });
    let output = output
        .map(|raw| {
            let selected = if raw.is_empty() { no_output } else { raw };
            match serde_json::from_str::<Value>(selected) {
                Ok(value) => pretty(&value).map(|code| Listing {
                    code,
                    language: "json",
                }),
                Err(_) => Ok(Listing {
                    code: selected.into(),
                    language: "text",
                }),
            }
        })
        .transpose()?;
    Ok(Prepared {
        caption,
        input,
        output,
    })
}
#[cfg(test)]
mod tests;

/// Call-owned immutable default content.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
pub(in crate::tools) struct DefaultContent {
    /// Status selected before formatting input.
    state: ToolHeaderState,
    /// Optional already-formatted input.
    params: Option<String>,
    /// Raw joined output; absence means no result yet.
    output: Option<String>,
}
/// Default JSON input and raw text output renderer.
#[derive(Default)]
pub struct DefaultRenderer;
impl<D> ToolRenderer<Value, D> for DefaultRenderer {
    fn render(
        &self,
        params: Option<&Value>,
        result: Option<ToolResultMessageView<'_, D>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        let state = status(result.as_ref(), is_streaming);
        let params = params.map(format_params).transpose()?.flatten();
        let output = result.as_ref().map(|result| get_text_output(Some(result)));
        Ok(Self::render_prepared(state, params, output))
    }
}
impl DefaultRenderer {
    /// Retain formatted inputs and raw output without inspecting native details.
    pub(in crate::tools) fn render_prepared(
        state: ToolHeaderState,
        params: Option<String>,
        output: Option<String>,
    ) -> ToolRenderResult {
        ToolRenderResult {
            content: Box::new(DefaultContent {
                state,
                params,
                output,
            }),
            is_custom: false,
        }
    }
}
