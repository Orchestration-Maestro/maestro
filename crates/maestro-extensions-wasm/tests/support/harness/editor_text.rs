//! Text replacement shared by the authored editors.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;

use maestro_extensions_wasm::{CallbackEmitter, ExtensionResult, TextHandler};

/// Replaces an editor's text and runs the change handler it held.
pub(super) fn replace(
    text: &RefCell<String>,
    change: &RefCell<Option<TextHandler>>,
    value: &str,
    emitter: CallbackEmitter<'_>,
) -> ExtensionResult<()> {
    text.replace(value.to_owned());
    let handler = change.borrow().clone();
    handler.map_or(Ok(()), |handler| handler(value, emitter))
}
