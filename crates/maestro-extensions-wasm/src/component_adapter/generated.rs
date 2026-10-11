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
use crate::{AutocompleteItem, FlagOptions, FlagValue};

/// The generated host imports and resources.
#[derive(Clone, Copy)]
struct Generated;

impl Imports for Generated {
    type Subscription = host::Subscription;
    type Emitter = host::CallbackEmitter;
    fn event_on(
        &self,
        channel: &str,
        handler: &Self::Callback,
    ) -> ExtensionResult<Self::Subscription> {
        host::event_on(channel, handler)
    }
    fn unsubscribe(&self, subscription: &Self::Subscription) -> ExtensionResult<()> {
        subscription.unsubscribe()
    }
    fn event_emit(&self, channel: String, data: String) -> ExtensionFuture<'_, ()> {
        Box::pin(host::event_emit(channel, data))
    }
    fn callback_emit(
        &self,
        emitter: &Self::Emitter,
        channel: &str,
        data: &str,
    ) -> ExtensionResult<()> {
        host::callback_emit(emitter, channel, data)
    }
    fn start_event_tail(
        &self,
        emitter: &Self::Emitter,
        tail: &Self::Callback,
    ) -> ExtensionResult<()> {
        host::start_event_tail(emitter, tail)
    }
    type Ui = host::ExtensionUiContext;
    type Registry = host::ModelRegistry;
    fn registry_get_all(&self, resource: &Self::Registry) -> ExtensionResult<String> {
        resource.get_all()
    }
    fn registry_get_available(&self, resource: &Self::Registry) -> ExtensionResult<String> {
        resource.get_available()
    }
    fn registry_find(
        &self,
        resource: &Self::Registry,
        provider: &str,
        model_id: &str,
    ) -> ExtensionResult<Option<String>> {
        resource.find(provider, model_id)
    }
    fn registry_get_api_key_and_headers<'a>(
        &'a self,
        resource: &'a Self::Registry,
        model: String,
    ) -> ExtensionFuture<'a, crate::ResolvedRequestAuth> {
        Box::pin(async move {
            Ok(match resource.get_api_key_and_headers(model).await? {
                host::ResolvedRequestAuth::Ok(auth) => crate::ResolvedRequestAuth::Ok {
                    api_key: auth.api_key,
                    headers: auth.headers.map(|headers| headers.into_iter().collect()),
                },
                host::ResolvedRequestAuth::Error(error) => {
                    crate::ResolvedRequestAuth::Error { error }
                }
            })
        })
    }
    fn ui(&self, context: &Self::Context) -> ExtensionResult<Self::Ui> {
        context.ui()
    }
    fn has_ui(&self, context: &Self::Context) -> ExtensionResult<bool> {
        context.has_ui()
    }
    fn model_registry(&self, context: &Self::Context) -> ExtensionResult<Self::Registry> {
        context.model_registry()
    }
    fn model(&self, context: &Self::Context) -> ExtensionResult<Option<String>> {
        context.model()
    }
    fn is_idle(&self, context: &Self::Context) -> ExtensionResult<bool> {
        context.is_idle()
    }
    fn signal(&self, context: &Self::Context) -> ExtensionResult<Option<Self::Signal>> {
        context.signal()
    }
    fn abort(&self, context: &Self::Context) -> ExtensionResult<()> {
        context.abort()
    }
    fn has_pending_messages(&self, context: &Self::Context) -> ExtensionResult<bool> {
        context.has_pending_messages()
    }
    fn shutdown(&self, context: &Self::Context) -> ExtensionResult<()> {
        context.shutdown()
    }
    fn get_context_usage(
        &self,
        context: &Self::Context,
    ) -> ExtensionResult<Option<crate::ContextUsage>> {
        context.get_context_usage()
    }
    fn get_system_prompt(&self, context: &Self::Context) -> ExtensionResult<String> {
        context.get_system_prompt()
    }
    fn command_ui(&self, context: &Self::CommandContext) -> ExtensionResult<Self::Ui> {
        context.ui()
    }
    fn command_has_ui(&self, context: &Self::CommandContext) -> ExtensionResult<bool> {
        context.has_ui()
    }
    fn command_model_registry(
        &self,
        context: &Self::CommandContext,
    ) -> ExtensionResult<Self::Registry> {
        context.model_registry()
    }
    fn command_model(&self, context: &Self::CommandContext) -> ExtensionResult<Option<String>> {
        context.model()
    }
    fn command_is_idle(&self, context: &Self::CommandContext) -> ExtensionResult<bool> {
        context.is_idle()
    }
    fn command_signal(
        &self,
        context: &Self::CommandContext,
    ) -> ExtensionResult<Option<Self::Signal>> {
        context.signal()
    }
    fn command_abort(&self, context: &Self::CommandContext) -> ExtensionResult<()> {
        context.abort()
    }
    fn command_has_pending_messages(
        &self,
        context: &Self::CommandContext,
    ) -> ExtensionResult<bool> {
        context.has_pending_messages()
    }
    fn command_shutdown(&self, context: &Self::CommandContext) -> ExtensionResult<()> {
        context.shutdown()
    }
    fn command_get_context_usage(
        &self,
        context: &Self::CommandContext,
    ) -> ExtensionResult<Option<crate::ContextUsage>> {
        context.get_context_usage()
    }
    fn command_get_system_prompt(&self, context: &Self::CommandContext) -> ExtensionResult<String> {
        context.get_system_prompt()
    }

    type Update = host::ToolUpdate;
    type Callback = host::Callback;
    type Signal = host::AbortSignal;
    type Context = host::Context;
    type CommandContext = host::CommandContext;
    type ReplacedContext = host::ReplacedSessionContext;

    type Reader = host::ReadonlySessionManager;
    type Node = host::SessionTreeNode;
    fn session_manager(&self, context: &Self::Context) -> ExtensionResult<Self::Reader> {
        context.session_manager()
    }
    fn command_session_manager(
        &self,
        context: &Self::CommandContext,
    ) -> ExtensionResult<Self::Reader> {
        context.session_manager()
    }
    fn reader_get_cwd(&self, resource: &Self::Reader) -> ExtensionResult<String> {
        resource.get_cwd()
    }
    fn reader_get_session_dir(&self, resource: &Self::Reader) -> ExtensionResult<String> {
        resource.get_session_dir()
    }
    fn reader_get_session_id(&self, resource: &Self::Reader) -> ExtensionResult<String> {
        resource.get_session_id()
    }
    fn reader_get_session_file(&self, resource: &Self::Reader) -> ExtensionResult<Option<String>> {
        resource.get_session_file()
    }
    fn reader_get_leaf_id(&self, resource: &Self::Reader) -> ExtensionResult<Option<String>> {
        resource.get_leaf_id()
    }
    fn reader_get_leaf_entry(&self, resource: &Self::Reader) -> ExtensionResult<Option<String>> {
        resource.get_leaf_entry()
    }
    fn reader_get_entry(
        &self,
        resource: &Self::Reader,
        id: &str,
    ) -> ExtensionResult<Option<String>> {
        resource.get_entry(id)
    }
    fn reader_get_label(
        &self,
        resource: &Self::Reader,
        id: &str,
    ) -> ExtensionResult<Option<String>> {
        resource.get_label(id)
    }
    fn reader_get_branch(
        &self,
        resource: &Self::Reader,
        from_id: Option<&str>,
    ) -> ExtensionResult<String> {
        resource.get_branch(from_id)
    }
    fn reader_get_header(&self, resource: &Self::Reader) -> ExtensionResult<Option<String>> {
        resource.get_header()
    }
    fn reader_get_entries(&self, resource: &Self::Reader) -> ExtensionResult<String> {
        resource.get_entries()
    }
    fn reader_get_tree(&self, resource: &Self::Reader) -> ExtensionResult<Vec<Self::Node>> {
        resource.get_tree()
    }
    fn reader_get_session_name(&self, resource: &Self::Reader) -> ExtensionResult<Option<String>> {
        resource.get_session_name()
    }
    fn node_entry(&self, resource: &Self::Node) -> ExtensionResult<String> {
        resource.entry()
    }
    fn node_children(&self, resource: &Self::Node) -> ExtensionResult<Vec<Self::Node>> {
        resource.children()
    }
    fn node_label(&self, resource: &Self::Node) -> ExtensionResult<Option<String>> {
        resource.label()
    }
    fn node_label_timestamp(&self, resource: &Self::Node) -> ExtensionResult<Option<String>> {
        resource.label_timestamp()
    }
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
        completions: Option<&host::Callback>,
        handler: &host::Callback,
    ) -> ExtensionResult<()> {
        host::register_command(name, description, completions, handler)
    }
    fn register_shortcut(
        &self,
        shortcut: &str,
        description: Option<&str>,
        handler: &host::Callback,
    ) -> ExtensionResult<()> {
        host::register_shortcut(shortcut, description, handler)
    }
    fn register_flag(&self, name: &str, options: &FlagOptions) -> ExtensionResult<()> {
        host::register_flag(name, options)
    }
    fn get_flag(&self, name: &str) -> ExtensionResult<Option<FlagValue>> {
        host::get_flag(name)
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

    fn invoke_prepare(
        handler: &host::Callback,
        args: String,
        emitter: &host::CallbackEmitter,
    ) -> ExtensionResult<String> {
        Exports::new(Generated).invoke_prepare(handler.id(), &args, emitter)
    }
    fn invoke_listener(
        handler: &host::Callback,
        data: String,
        emitter: &host::CallbackEmitter,
    ) -> ExtensionResult<()> {
        Exports::new(Generated).invoke_listener(handler.id(), &data, emitter)
    }
    async fn invoke_tail(handler: &host::Callback) -> ExtensionResult<()> {
        Exports::<Generated>::invoke_tail(handler.id()).await
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

    async fn invoke_completion(
        handler: &host::Callback,
        prefix: String,
    ) -> ExtensionResult<Option<Vec<AutocompleteItem>>> {
        Exports::new(Generated)
            .invoke_completion(handler.id(), prefix)
            .await
    }
    async fn invoke_shortcut(handler: &host::Callback, ctx: host::Context) -> ExtensionResult<()> {
        Exports::new(Generated)
            .invoke_shortcut(handler.id(), ctx)
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
