//! The generated imports and exports behind the shared adapter logic: single calls into the
//! bindings, compiled only for the component target.

use std::marker::PhantomData;

use super::{Exports, Imports, release};
use crate::bindings::exports::maestro::extension::guest::{
    Guest, InputOutcome, SessionBeforeCompactOutcome,
};
use crate::bindings::maestro::extension::events as wire;
use crate::bindings::maestro::extension::host;
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::loader::Extension;
use crate::types::{ExtensionFuture, ExtensionResult};

/// The generated host imports and resources.
#[derive(Clone, Copy)]
struct Generated;

impl Imports for Generated {
    type Callback = host::Callback;
    type Signal = host::AbortSignal;
    type Context = host::Context;
    type CommandContext = host::CommandContext;
    type ReplacedContext = host::ReplacedSessionContext;

    fn new_callback(&self) -> (u32, host::Callback) {
        let handle = host::Callback::new();
        (handle.id(), handle)
    }

    fn on(&self, event: &str, handler: &host::Callback) -> ExtensionResult<()> {
        host::on(event, handler)
    }

    fn register_command(
        &self,
        name: &str,
        description: Option<&str>,
        handler: &host::Callback,
    ) -> ExtensionResult<()> {
        host::register_command(name, description, handler)
    }

    fn append_entry(&self, custom_type: &str, data: Option<&str>) -> ExtensionResult<()> {
        host::append_entry(custom_type, data)
    }

    fn aborted(&self, signal: &host::AbortSignal) -> bool {
        signal.aborted()
    }

    fn cwd(&self, context: &host::Context) -> ExtensionResult<String> {
        context.cwd()
    }

    fn command_cwd(&self, context: &host::CommandContext) -> ExtensionResult<String> {
        context.cwd()
    }

    fn wait_for_idle<'a>(&'a self, context: &'a host::CommandContext) -> ExtensionFuture<'a, ()> {
        Box::pin(context.wait_for_idle())
    }

    fn new_session<'a>(
        &'a self,
        context: &'a host::CommandContext,
        data: NewSessionCommandData,
        with_session: Option<&'a host::Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(context.new_session(data, with_session))
    }

    fn command(&self, context: &host::ReplacedSessionContext) -> host::CommandContext {
        context.command()
    }
}

/// Entry type of an extension component: serves the generated exports for `E`.
#[doc(hidden)]
pub struct Glue<E>(PhantomData<E>);

impl<E: Extension> Guest for Glue<E> {
    async fn start() -> ExtensionResult<()> {
        Exports::new(Generated).start::<E>().await
    }

    fn release(handler: &host::Callback) {
        release(handler.id());
    }

    async fn invoke_command(
        handler: &host::Callback,
        args: String,
        ctx: host::CommandContext,
    ) -> ExtensionResult<()> {
        Exports::new(Generated)
            .invoke_command(handler.id(), args, ctx)
            .await
    }

    async fn invoke_with_session(
        handler: &host::Callback,
        ctx: host::ReplacedSessionContext,
    ) -> ExtensionResult<()> {
        Exports::new(Generated)
            .invoke_with_session(handler.id(), ctx)
            .await
    }

    async fn invoke_session_before_compact(
        handler: &host::Callback,
        event: wire::SessionBeforeCompactEventData,
        signal: host::AbortSignal,
        ctx: host::Context,
    ) -> SessionBeforeCompactOutcome {
        Exports::new(Generated)
            .invoke_session_before_compact(handler.id(), event, signal, ctx)
            .await
    }

    async fn invoke_input(
        handler: &host::Callback,
        event: wire::InputEvent,
        ctx: host::Context,
    ) -> InputOutcome {
        Exports::new(Generated)
            .invoke_input(handler.id(), event, ctx)
            .await
    }
}
