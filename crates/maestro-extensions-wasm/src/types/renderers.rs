//! Custom-message renderer contracts.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::{CallbackEmitter, Component, CustomMessage, ExtensionResult, Theme};
use std::rc::Rc;
/// Options supplied to a custom-message renderer.
#[derive(Clone, Copy)]
pub struct MessageRenderOptions {
    /// Whether expanded rendering is requested.
    pub expanded: bool,
}
/// Renders a custom message or selects the host's fallback by returning no component.
pub type MessageRenderer = Rc<
    dyn for<'a> Fn(
        CustomMessage,
        MessageRenderOptions,
        Theme,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<Option<Component>>,
>;
