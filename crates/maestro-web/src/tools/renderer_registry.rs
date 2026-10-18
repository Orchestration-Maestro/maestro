//! Insertion-ordered renderer registration and shared tool headers.
use super::types::{ToolRenderer, ToolRendererBinding, ToolResultMessageView};
use std::{cell::RefCell, rc::Rc};
thread_local! {
    /// Browser-thread registrations, exposed for ordered enumeration and removal.
    pub static TOOL_RENDERERS: RefCell<indexmap::IndexMap<String, ToolRendererBinding>> = RefCell::default();
}
/// Register a name, retaining its original position on replacement.
pub fn register_tool_renderer(tool_name: &str, renderer: ToolRendererBinding) {
    let old = TOOL_RENDERERS.with(|all| all.borrow_mut().insert(tool_name.into(), renderer));
    drop(old);
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
pub(crate) fn get_text_output<D>(result: Option<&ToolResultMessageView<'_, D>>) -> String {
    result.map_or_else(String::new, |result| {
        maestro_app::UserBlock::text_output(result.content())
    })
}

/// Shared tool header content.
pub mod header;
pub use header::{ToolHeaderText, render_collapsible_header, render_header};
