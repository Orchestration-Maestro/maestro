//! Retained full-screen attachment preview.
mod ui_error;
pub use ui_error::AttachmentUiError;
#[cfg(target_arch = "wasm32")]
mod content;
#[cfg(target_arch = "wasm32")]
mod dom;
pub(crate) mod error;
pub(crate) mod excel;
pub(crate) mod pdf;
/// A separately owned attachment overlay opened in the page.
#[cfg(target_arch = "wasm32")]
pub struct AttachmentOverlay {
    /// The opened element's retained operands.
    state: std::rc::Rc<dom::State>,
}
