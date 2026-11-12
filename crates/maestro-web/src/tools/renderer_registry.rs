//! Insertion-ordered renderer registration and shared tool headers.
use super::types::{ToolRenderer, ToolRendererBinding};
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
use maestro_app::browser::tools::{
    extraction::{ExtractDocumentParams, ExtractDocumentResult},
    javascript_repl::{JavaScriptReplParams, JavaScriptReplToolResult},
};
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
use maestro_models::JsonObject;
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};
thread_local! {
    /// Browser-thread registrations, exposed for ordered enumeration and removal.
    pub static TOOL_RENDERERS: RefCell<indexmap::IndexMap<String, ToolRendererBinding>> = RefCell::new(default_renderers());
}
/// Install the built-in callbacks once; explicit replacement and removal stay effective.
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
fn default_renderers() -> indexmap::IndexMap<String, ToolRendererBinding> {
    let mut renderers = indexmap::IndexMap::new();
    let repl = Rc::new(super::javascript_repl::JAVASCRIPT_REPL_RENDERER);
    let extract = Rc::new(super::extraction::EXTRACT_DOCUMENT_RENDERER);
    renderers.insert(
        "javascript_repl".into(),
        ToolRendererBinding::from_typed::<JsonObject, Value>(repl.clone())
            .with_raw::<JavaScriptReplParams, JavaScriptReplToolResult>(repl),
    );
    renderers.insert(
        "extract_document".into(),
        ToolRendererBinding::from_typed::<JsonObject, Value>(extract.clone())
            .with_raw::<ExtractDocumentParams, ExtractDocumentResult>(extract),
    );
    renderers.insert(
        "bash".into(),
        ToolRendererBinding::from_details_agnostic(Rc::new(
            super::renderers::bash_renderer::BashRenderer,
        )),
    );
    renderers
}
/// Native production builds have no page representation for the built-in cards.
#[cfg(not(any(target_arch = "wasm32", test, feature = "test-support")))]
fn default_renderers() -> indexmap::IndexMap<String, ToolRendererBinding> {
    indexmap::IndexMap::new()
}
/// Register a name, retaining its original position on replacement.
pub fn register_tool_renderer(tool_name: &str, renderer: ToolRendererBinding) {
    let old = TOOL_RENDERERS.with(|all| all.borrow_mut().insert(tool_name.into(), renderer));
    drop(old);
}
/// Remove a tool's page renderer only while `renderer` is still the registered one.
#[cfg(target_arch = "wasm32")]
pub(crate) fn unregister_tool_renderer<P: 'static, D: 'static>(
    tool_name: &str,
    renderer: &Rc<dyn ToolRenderer<P, D, Box<dyn crate::page::DomContent>>>,
) -> bool {
    let removed = TOOL_RENDERERS.with(|all| {
        let mut all = all.borrow_mut();
        let current = all
            .get(tool_name)
            .and_then(ToolRendererBinding::renderer::<P, D>);
        let owned = current.is_some_and(|current| Rc::ptr_eq(&current, renderer));
        owned.then(|| all.shift_remove(tool_name)).flatten()
    });
    let owned = removed.is_some();
    drop(removed);
    owned
}
/// Retain the registered renderer for these native operand types.
#[must_use]
pub fn get_tool_renderer<P: 'static, D: 'static>(
    tool_name: &str,
) -> Option<Rc<dyn ToolRenderer<P, D>>> {
    let binding = TOOL_RENDERERS.with(|all| all.borrow().get(tool_name).cloned());
    binding.and_then(|binding| binding.renderer::<P, D>())
}

/// Visual status shared by plain and collapsible tool headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolHeaderState {
    /// Work has not produced a result yet.
    Inprogress,
    /// Work completed without an error.
    Complete,
    /// The result reports an error.
    Error,
}
/// Join every raw text block in order, without terminal display normalization.
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
pub(crate) fn get_text_output<D>(
    result: Option<&super::types::ToolResultMessageView<'_, D>>,
) -> String {
    result.map_or_else(String::new, |result| {
        maestro_models::UserBlock::text_output(result.content())
    })
}

/// Shared tool header content.
pub mod heading;
pub use heading::{ToolHeaderRefs, ToolHeaderText};
#[cfg(target_arch = "wasm32")]
pub use heading::{render_collapsible_header, render_header};
