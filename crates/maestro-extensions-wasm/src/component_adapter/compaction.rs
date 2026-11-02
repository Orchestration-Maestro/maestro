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
use crate::bindings::maestro::extension::host::CompactionData;
use crate::{
    CallbackEmitter, CompactOptions, CompactionComplete, CompactionError, ExtensionResult,
};
/// Admits the two independently retained callbacks as one request.
pub(super) fn compact<I: Imports>(
    imports: &I,
    options: Option<CompactOptions>,
    send: impl FnOnce(
        Option<&CompactionData>,
        Option<&I::Callback>,
        Option<&I::Callback>,
    ) -> ExtensionResult<()>,
) -> ExtensionResult<()> {
    let (custom_instructions, on_complete, on_error) =
        options.map_or((None, None, None), |options| {
            (
                Some(CompactionData {
                    custom_instructions: options.custom_instructions,
                }),
                options.on_complete,
                options.on_error,
            )
        });
    let complete = on_complete.map(|f| staged(imports, f));
    let error = on_error.map(|f| staged(imports, f));
    let result = send(
        custom_instructions.as_ref(),
        complete.as_ref().map(|id| &id.handle),
        error.as_ref().map(|id| &id.handle),
    );
    for id in [complete, error].into_iter().flatten() {
        id.settle(result.is_ok());
    }
    result
}

/// Stages a notification so the host may deliver it before compaction returns.
fn staged<I: Imports>(imports: &I, closure: impl callbacks::Stored) -> Identity<I> {
    let identity = Identity::new(imports);
    identity.stage(closure);
    identity
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
