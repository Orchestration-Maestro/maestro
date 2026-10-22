//! Browser tool views.
use crate::tools::types::ToolUiError;
pub mod artifacts;
/// Document-extraction cards.
pub mod extract_document;
/// JavaScript execution cards and browser-local hidden tool registration.
pub mod javascript_repl;

/// Named browser renderer registrations.
pub mod renderer_registry;
/// Typed browser content and borrowing renderer operands.
pub mod types;

/// Browser tool renderer implementations.
pub mod renderers;

pub use renderer_registry::{get_tool_renderer, register_tool_renderer};
use renderers::default_renderer::{DefaultRenderer, status};
use std::cell::Cell;
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
pub fn render_tool<P: 'static, D: 'static>(
    tool_name: &str,
    params: Option<ToolParams<'_, P>>,
    result: Option<&maestro_app::ToolResultMessage<D>>,
    is_streaming: bool,
) -> Result<ToolRenderResult, ToolUiError> {
    dispatch::<P, D, Box<dyn types::ToolContent>>(tool_name, params, result, is_streaming)
}
/// Content representation selected by the shared dispatcher.
///
/// The canvas implementation leaves with the canvas renderer family.
pub(crate) trait ContentKind: Sized + 'static {
    /// The registered handle for these operands.
    fn renderer<P: 'static, D: 'static>(
        binding: &types::ToolRendererBinding,
    ) -> Option<std::rc::Rc<dyn types::ToolRenderer<P, D, Self>>>;
    /// The default renderer's prepared result.
    fn default_result(
        state: renderer_registry::ToolHeaderState,
        params: Option<String>,
        output: Option<String>,
    ) -> ToolRenderResult<Self>;
}
impl ContentKind for Box<dyn types::ToolContent> {
    fn renderer<P: 'static, D: 'static>(
        binding: &types::ToolRendererBinding,
    ) -> Option<std::rc::Rc<dyn types::ToolRenderer<P, D, Self>>> {
        binding.renderer::<P, D>()
    }
    fn default_result(
        state: renderer_registry::ToolHeaderState,
        params: Option<String>,
        output: Option<String>,
    ) -> ToolRenderResult<Self> {
        DefaultRenderer::render_prepared(state, params, output)
    }
}
/// Route original operands to the selected custom or default renderer.
fn dispatch<P: 'static, D: 'static, C: ContentKind>(
    tool_name: &str,
    params: Option<ToolParams<'_, P>>,
    result: Option<&maestro_app::ToolResultMessage<D>>,
    is_streaming: bool,
) -> Result<ToolRenderResult<C>, ToolUiError> {
    if !SHOW_JSON_MODE.get() {
        let binding =
            renderer_registry::TOOL_RENDERERS.with(|all| all.borrow().get(tool_name).cloned());
        if let Some(binding) = binding {
            let renderer = C::renderer::<P, D>(&binding).ok_or_else(|| {
                ToolUiError::Presentation("Renderer operand types do not match".into())
            })?;
            return renderer.render(
                params.as_ref().map(|p| p.value),
                result.map(ToolResultMessageView::from),
                is_streaming,
            );
        }
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
    Ok(C::default_result(state, formatted, output))
}
/// Page-content tool rendering.
#[cfg(target_arch = "wasm32")]
pub mod dom {
    use super::{ContentKind, ToolParams, ToolRenderResult, ToolUiError, dispatch, types};
    use crate::dom::DomContent;
    impl ContentKind for Box<dyn DomContent> {
        fn renderer<P: 'static, D: 'static>(
            binding: &types::ToolRendererBinding,
        ) -> Option<std::rc::Rc<dyn types::ToolRenderer<P, D, Self>>> {
            binding.slot::<P, D, Self>()
        }
        fn default_result(
            state: super::renderer_registry::ToolHeaderState,
            params: Option<String>,
            output: Option<String>,
        ) -> ToolRenderResult<Self> {
            super::renderers::default_renderer::DefaultRenderer::render_dom_prepared(
                state, params, output,
            )
        }
    }
    /// Route original operands to the selected page-content or default renderer.
    /// # Errors
    /// Returns an operand-type mismatch, decoder, formatter or custom renderer failure.
    pub fn render_tool<P: 'static, D: 'static>(
        tool_name: &str,
        params: Option<ToolParams<'_, P>>,
        result: Option<&maestro_app::ToolResultMessage<D>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
        dispatch(tool_name, params, result, is_streaming)
    }
}
