//! Component adapter: forwards registrations to the generated imports and serves the
//! generated exports from the private closure table.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod callbacks;
mod contexts;
mod host_api;

use std::marker::PhantomData;
use std::rc::Rc;

use crate::bindings::exports::maestro::extension::guest::{self, Guest};
use crate::bindings::maestro::extension::events as wire;
use crate::bindings::maestro::extension::host;
use crate::loader::Extension;
use crate::types::{
    CommandHandler, ExtensionAPI, ExtensionEvent, ExtensionHandler, SessionBeforeCompactEvent,
    SessionEvent, WithSession,
};

/// Entry type of an extension component: serves the generated exports for `E`.
#[doc(hidden)]
pub struct Glue<E>(PhantomData<E>);

/// Runs the handler registered under `handler` against an event the host delivered.
async fn run(
    handler: &host::Callback,
    event: &mut ExtensionEvent,
    ctx: host::Context,
) -> guest::Decision {
    let handler = callbacks::find::<ExtensionHandler>(handler)?;
    let result = handler(event, contexts::context(ctx)).await?;
    Ok(result.map(crate::types::ExtensionEventResult::into_wire))
}

impl<E: Extension> Guest for Glue<E> {
    async fn start() -> Result<(), String> {
        E::load(ExtensionAPI::new(Rc::new(host_api::GeneratedHost))).await
    }

    fn release(handler: &host::Callback) {
        callbacks::release(handler.id());
    }

    async fn invoke_command(
        handler: &host::Callback,
        args: String,
        ctx: host::CommandContext,
    ) -> Result<(), String> {
        let command = callbacks::find::<CommandHandler>(handler)?;
        command(args, contexts::command_context(ctx)).await
    }

    async fn invoke_with_session(
        handler: &host::Callback,
        ctx: host::ReplacedSessionContext,
    ) -> Result<(), String> {
        callbacks::take::<WithSession>(handler)?(contexts::replaced_context(ctx)).await
    }

    async fn invoke_session_before_compact(
        handler: &host::Callback,
        event: wire::SessionBeforeCompactEventData,
        signal: host::AbortSignal,
        ctx: host::Context,
    ) -> guest::Decision {
        let event = SessionBeforeCompactEvent {
            data: event,
            signal: contexts::signal(signal),
        };
        run(
            handler,
            &mut ExtensionEvent::Session(SessionEvent::BeforeCompact(event)),
            ctx,
        )
        .await
    }

    async fn invoke_input(
        handler: &host::Callback,
        event: wire::InputEvent,
        ctx: host::Context,
    ) -> guest::Decision {
        run(handler, &mut ExtensionEvent::Input(event), ctx).await
    }
}
