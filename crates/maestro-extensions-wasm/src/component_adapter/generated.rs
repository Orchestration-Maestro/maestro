//! The generated imports and exports behind the shared adapter logic: single calls into the
//! bindings, compiled only for the component target.

use std::marker::PhantomData;

use super::tools::ToolCapabilities;
use super::{Capabilities, Exports, Imports, release};
use crate::bindings::exports::maestro::extension::guest::ToolInvocation;
use crate::bindings::exports::maestro::extension::guest::{self, Guest};
use crate::bindings::maestro::extension::host;
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::loader::Extension;
use crate::types::{ExtensionFuture, ExtensionResult};

/// The generated host imports and resources.
#[derive(Clone, Copy)]
struct Generated;

impl Imports for Generated {
    type Update = host::ToolUpdate;
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

    fn register_tool(
        &self,
        metadata: &str,
        prepare: Option<&host::Callback>,
        execute: &host::Callback,
    ) -> ExtensionResult<()> {
        host::register_tool(metadata, prepare, execute)
    }
    fn tool_update(&self, update: &host::ToolUpdate, partial: &str) -> ExtensionResult<()> {
        update.update(partial)
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

    fn invoke_prepare(handler: &host::Callback, args: String) -> ExtensionResult<String> {
        Exports::<Generated>::invoke_prepare(handler.id(), &args)
    }
    async fn invoke_tool(
        handler: &host::Callback,
        invocation: ToolInvocation,
        resources: guest::ToolCapabilities,
    ) -> ExtensionResult<String> {
        let guest::ToolCapabilities {
            ctx,
            signal,
            update,
        } = resources;
        Exports::new(Generated)
            .invoke_tool(
                handler.id(),
                invocation,
                ToolCapabilities {
                    ctx,
                    signal,
                    update,
                },
            )
            .await
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

    async fn invoke_event(
        handler: &host::Callback,
        event: String,
        resources: guest::Capabilities,
    ) -> Result<guest::EventOutcome, String> {
        let guest::Capabilities { ctx, signal } = resources;
        Exports::new(Generated)
            .invoke_event(handler.id(), event, Capabilities { ctx, signal })
            .await
    }
}
