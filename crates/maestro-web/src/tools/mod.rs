//! Browser tool views.
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
use crate::tools::types::ToolUiError;
/// Executable tools: artifact commands, document extraction, JavaScript execution and prompts.
pub mod browser;
pub mod documents;
/// Document-extraction cards.
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
pub mod extraction;
/// JavaScript execution cards and browser-local hidden tool registration.
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
pub mod javascript_repl;

/// Named browser renderer registrations.
pub mod renderer_registry;
/// Typed browser content and borrowing renderer operands.
pub mod types;

/// Browser tool renderer implementations.
pub mod renderers;
#[cfg(test)]
mod tests;
/// Rendered-value projections of prepared tool records for native tests.
#[cfg(all(not(target_arch = "wasm32"), any(test, feature = "test-support")))]
pub(crate) mod view;

pub use renderer_registry::{get_tool_renderer, register_tool_renderer};
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
use renderers::default_renderer::{DefaultRenderer, status};
use std::cell::Cell;
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
use types::{ToolParams, ToolRenderResult, ToolResultMessageView};
thread_local! {
    /// Current dispatch policy, independent of registrations.
    static SHOW_JSON_MODE: Cell<bool> = const { Cell::new(false) };
}
/// Force default rendering without removing any custom registration.
pub fn set_show_json_mode(enabled: bool) {
    SHOW_JSON_MODE.set(enabled);
}
/// Route original operands to the selected custom or default renderer.
/// # Errors
/// Returns an operand-type mismatch, decoder, formatter or custom renderer failure.
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
pub fn render_tool<P: 'static, D: 'static>(
    tool_name: &str,
    params: Option<ToolParams<'_, P>>,
    result: Option<&maestro_models::ToolResultMessage<D>>,
    is_streaming: bool,
) -> Result<ToolRenderResult, ToolUiError> {
    render_with_binding(
        || renderer_registry::TOOL_RENDERERS.with(|all| all.borrow().get(tool_name).cloned()),
        params,
        result,
        is_streaming,
    )
}
/// Dispatch with a retained binding while keeping JSON mode and default rendering shared.
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
fn render_with_binding<P: 'static, D: 'static>(
    binding: impl FnOnce() -> Option<types::ToolRendererBinding>,
    params: Option<ToolParams<'_, P>>,
    result: Option<&maestro_models::ToolResultMessage<D>>,
    is_streaming: bool,
) -> Result<ToolRenderResult, ToolUiError> {
    if !SHOW_JSON_MODE.get()
        && let Some(binding) = binding()
    {
        let renderer = binding.renderer::<P, D>().ok_or_else(|| {
            ToolUiError::Presentation("Renderer operand types do not match".into())
        })?;
        return renderer.render(
            params.as_ref().map(|p| p.value),
            result.map(ToolResultMessageView::from),
            is_streaming,
        );
    }
    let result = result.map(ToolResultMessageView::from);
    let state = status(result.as_ref(), is_streaming);
    let formatted = params
        .map(|params| (params.format)(params.value))
        .transpose()?
        .flatten();
    let output = result
        .as_ref()
        .map(|result| renderer_registry::get_text_output(Some(result)));
    Ok(DefaultRenderer::render_prepared(state, formatted, output))
}
/// Route a model call through its view's renderer, then page registrations or the default.
///
/// An object root borrows its inner map for map registrations; any other root borrows the
/// whole value through the ordinary JSON route.
/// # Errors
/// Returns an operand-type mismatch, decoder, formatter or custom renderer failure.
#[cfg(target_arch = "wasm32")]
pub(crate) fn render_call<D: 'static>(
    view: &crate::page::DomView,
    tool_name: &str,
    arguments: &serde_json::Value,
    result: Option<&maestro_models::ToolResultMessage<D>>,
    is_streaming: bool,
) -> Result<ToolRenderResult, ToolUiError> {
    let binding = || {
        view.tool_renderers.get(tool_name).cloned().or_else(|| {
            renderer_registry::TOOL_RENDERERS.with(|all| all.borrow().get(tool_name).cloned())
        })
    };
    match arguments.as_object() {
        Some(members) => render_with_binding(
            binding,
            Some(ToolParams::object(members)),
            result,
            is_streaming,
        ),
        None => render_with_binding(
            binding,
            Some(ToolParams::json(arguments)),
            result,
            is_streaming,
        ),
    }
}
/// Page-content tool rendering, the same dispatcher as the root.
#[cfg(target_arch = "wasm32")]
pub mod dom {
    pub use super::render_tool;
}
