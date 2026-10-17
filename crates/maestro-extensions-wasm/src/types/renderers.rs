//! Custom-message renderer contracts.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::{CallbackEmitter, Component, CustomMessage, ExtensionResult, Theme};
pub use maestro_request::messages::MessageRenderOptions;
use std::rc::Rc;
/// Renders a custom message or selects the host's fallback by returning no component.
pub type MessageRenderer = Rc<
    dyn for<'a> Fn(
        CustomMessage,
        MessageRenderOptions,
        Theme,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<Option<Component>>,
>;
