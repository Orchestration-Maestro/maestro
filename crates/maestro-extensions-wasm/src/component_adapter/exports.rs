//! The exports of the extension world, served over any set of host imports.

use std::mem::discriminant;
use std::rc::Rc;

use serde::Serialize;

use super::callbacks;
use super::contexts;
use super::host_api::Host;
use super::imports::Imports;
use crate::bindings::exports::maestro::extension::guest::{Decision, Encoded, EventOutcome};
use crate::loader::Extension;
use crate::types::{
    CommandHandler, EventData, ExtensionAPI, ExtensionHandler, ExtensionResult, WithSession,
};

/// Serves the exports an extension component provides on top of its host.
pub(crate) struct Exports<I: Imports> {
    /// The host the extension runs against.
    pub(super) imports: I,
}

/// The host resources lent to one event callback.
pub(crate) struct Capabilities<I: Imports> {
    /// The context the handler receives.
    pub(crate) ctx: I::Context,
    /// The cancellation signal that accompanies a compaction.
    pub(crate) signal: Option<I::Signal>,
}

/// Encodes an optional document, or the message the encoding failed with.
fn encode<T: Serialize>(value: Option<T>) -> Encoded {
    value.map(|value| encode_value(&value)).transpose()
}

/// The outcome the host receives: the edited event and the decision, each encoded
/// independently so that neither failure hides the other.
pub(crate) fn outcome<E: Serialize, R: Serialize>(
    event: Option<E>,
    decision: ExtensionResult<Option<R>>,
) -> EventOutcome {
    EventOutcome {
        event: encode(event),
        decision: match decision {
            Ok(result) => Decision::Returned(encode(result)),
            Err(message) => Decision::Failed(message),
        },
    }
}

impl<I: Imports> Exports<I> {
    /// Serves exports over `imports`.
    pub(crate) fn new(imports: I) -> Self {
        Self { imports }
    }

    /// Runs the extension factory of `E`; resolves when it has completed.
    pub(crate) async fn start<E: Extension>(&self) -> ExtensionResult<()> {
        E::load(ExtensionAPI::new(Rc::new(Host(self.imports.clone())))).await
    }

    /// Runs the command handler registered under `handler`.
    pub(crate) async fn invoke_command(
        &self,
        handler: u32,
        args: String,
        ctx: I::CommandContext,
    ) -> ExtensionResult<()> {
        let command = callbacks::find::<CommandHandler>(handler)?;
        command(args, contexts::command_context(&self.imports, ctx)).await
    }

    /// Runs the continuation pending under `handler`.
    pub(crate) async fn invoke_with_session(
        &self,
        handler: u32,
        ctx: I::ReplacedContext,
    ) -> ExtensionResult<()> {
        callbacks::take::<WithSession>(handler)?(contexts::replaced_context(&self.imports, ctx))
            .await
    }

    /// Delivers the event `event` to the handler registered under `handler`.
    ///
    /// The outcome carries the event as the handler left it, whether the handler answered or
    /// failed, unless the handler replaced it with an event of another kind, and the handler's
    /// decision. The handler runs at most once.
    ///
    /// # Errors
    /// Returns the decoder's message when the document is not an event, and the message of
    /// the attachment when a before-compaction or before-tree event has no signal; the handler has not run.
    pub(crate) async fn invoke_event(
        &self,
        handler: u32,
        event: String,
        resources: Capabilities<I>,
    ) -> Result<EventOutcome, String> {
        let Capabilities { ctx, signal } = resources;
        let data = EventData::decode(&event).map_err(|error| error.to_string())?;
        let kind = discriminant(&data);
        let signal = signal.map(|signal| contexts::signal(&self.imports, signal));
        let mut event = data.attach(signal)?;
        let decision = match callbacks::find::<ExtensionHandler>(handler) {
            Ok(handler) => handler(&mut event, contexts::context(&self.imports, ctx)).await,
            Err(message) => Err(message),
        };
        let edited = EventData::from(event);
        let edited = (discriminant(&edited) == kind).then_some(edited);
        Ok(outcome(edited, decision))
    }
}

/// Encodes authored data after rejecting nonfinite typed numbers.
pub(super) fn encode_value<T: Serialize>(value: &T) -> Result<String, String> {
    crate::types::finite::encode_value(value)
}
