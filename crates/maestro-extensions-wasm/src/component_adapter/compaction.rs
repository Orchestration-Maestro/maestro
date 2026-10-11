//! Fire-and-return compaction and retained notification callbacks.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{
    Exports, Imports,
    callbacks::{self, Identity},
    event_bus::Emitter,
};
use crate::{
    CallbackEmitter, CompactOptions, CompactionComplete, CompactionError, ExtensionResult,
};
/// Admits the two independently retained callbacks as one request.
pub(super) fn compact<I: Imports>(
    imports: &I,
    options: Option<CompactOptions>,
    send: impl FnOnce(Option<&str>, Option<&I::Callback>, Option<&I::Callback>) -> ExtensionResult<()>,
) -> ExtensionResult<()> {
    let CompactOptions {
        custom_instructions,
        on_complete,
        on_error,
    } = options.unwrap_or_default();
    let complete = on_complete.map(|f| (Identity::new(imports), f));
    let error = on_error.map(|f| (Identity::new(imports), f));
    send(
        custom_instructions.as_deref(),
        complete.as_ref().map(|(id, _)| &id.handle),
        error.as_ref().map(|(id, _)| &id.handle),
    )?;
    if let Some((id, f)) = complete {
        id.keep(f);
    }
    if let Some((id, f)) = error {
        id.keep(f);
    }
    Ok(())
}
impl<I: Imports> Exports<I> {
    /// Delivers a success without consuming the registered callback.
    pub(crate) fn invoke_compaction_complete(
        &self,
        handler: u32,
        result: &str,
        emitter: &I::Emitter,
    ) -> ExtensionResult<()> {
        let callback = callbacks::find::<CompactionComplete>(handler)?;
        let port = Emitter {
            imports: &self.imports,
            handle: emitter,
        };
        callback(super::session::decode(result)?, CallbackEmitter::new(&port))
    }
    /// Delivers a failure without consuming the registered callback.
    pub(crate) fn invoke_compaction_error(
        &self,
        handler: u32,
        error: String,
        emitter: &I::Emitter,
    ) -> ExtensionResult<()> {
        let callback = callbacks::find::<CompactionError>(handler)?;
        let port = Emitter {
            imports: &self.imports,
            handle: emitter,
        };
        callback(error, CallbackEmitter::new(&port))
    }
}
