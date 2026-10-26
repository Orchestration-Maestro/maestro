//! Generated guest exports forward to the same adapter bodies as native controls.
use super::{Generated, host, route, streams};
use crate::bindings::exports::maestro::extension::guest::{self, Guest, ToolInvocation};
use crate::component_adapter::{Exports, release, renderers, tools::ToolCapabilities};
use crate::loader::Extension;
use crate::{AutocompleteItem, ExtensionResult};
use std::{future::Future, marker::PhantomData};

/// Entry type of an extension component: serves the generated exports for `E`.
#[doc(hidden)]
pub struct Glue<E>(PhantomData<E>);

impl<E: Extension> Guest for Glue<E> {
    fn invoke_stream(
        handler: &host::Callback,
        value: &host::Callback,
        operation: host::StreamOperation,
    ) -> impl Future<Output = ExtensionResult<host::StreamSelection>> {
        std::future::ready(streams::invoke(handler.id(), value.id(), operation))
    }

    fn describe_schema(
        handler: &host::Callback,
    ) -> Result<host::SchemaShape, host::DiagnosticErrorInfo> {
        Exports::<Generated>::describe_schema(handler.id()).map_err(crate::schema_error::to_wire)
    }
    fn retain_schema(
        handler: &host::Callback,
        retained: host::Callback,
    ) -> Result<(), host::DiagnosticErrorInfo> {
        Exports::new(Generated)
            .retain_schema(handler.id(), retained)
            .map_err(crate::schema_error::to_wire)
    }
    fn invoke_schema(
        handler: &host::Callback,
        operation: host::SchemaOperation,
        node: &host::JsonNode,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = Result<String, host::DiagnosticErrorInfo>> {
        std::future::ready(
            Exports::new(Generated)
                .invoke_schema(handler.id(), operation, node, emitter)
                .map_err(crate::schema_error::to_wire),
        )
    }
    fn invoke_renderer(
        handler: &host::Callback,
        request: guest::RenderRequest,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<Option<host::Callback>>> {
        let request = renderers::RenderRequest {
            message: &request.message,
            expanded: request.expanded,
            theme: request.theme,
            emitter,
        };
        std::future::ready(
            Exports::new(Generated)
                .invoke_renderer(handler.id(), request)
                .map(|view| renderers::retain(&Generated, view)),
        )
    }
    fn component_render(
        handler: &host::Callback,
        width: u32,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<Vec<String>>> {
        std::future::ready(Exports::new(Generated).invoke_component(
            handler.id(),
            emitter,
            |view, emitter| view.render(width, emitter),
        ))
    }
    fn component_handle_input(
        handler: &host::Callback,
        data: String,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_component(
            handler.id(),
            emitter,
            |view, emitter| view.handle_input(&data, emitter),
        ))
    }
    fn component_wants_key_release(
        handler: &host::Callback,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<bool>> {
        std::future::ready(Exports::new(Generated).invoke_component(
            handler.id(),
            emitter,
            crate::Component::wants_key_release,
        ))
    }
    fn component_invalidate(
        handler: &host::Callback,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_component(
            handler.id(),
            emitter,
            crate::Component::invalidate,
        ))
    }

    async fn invoke_setup(
        handler: &host::Callback,
        writer: host::SessionManager,
    ) -> ExtensionResult<()> {
        Exports::new(Generated)
            .invoke_setup(handler.id(), writer)
            .await
    }
    fn invoke_compaction_complete(
        handler: &host::Callback,
        result: String,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_compaction_complete(
            handler.id(),
            &result,
            emitter,
        ))
    }
    fn invoke_compaction_error(
        handler: &host::Callback,
        error: String,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_compaction_error(
            handler.id(),
            error,
            emitter,
        ))
    }

    fn invoke_abort(
        handler: &host::Callback,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_abort(handler.id(), emitter))
    }

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
    ) -> impl Future<Output = ExtensionResult<String>> {
        std::future::ready(Exports::new(Generated).invoke_prepare(handler.id(), &args, emitter))
    }
    fn invoke_listener(
        handler: &host::Callback,
        data: String,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_listener(handler.id(), &data, emitter))
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
        resources: host::Capabilities,
    ) -> Result<host::EventOutcome, String> {
        route::event(handler.id(), event, resources).await
    }
    async fn bash_exec(
        handler: &host::Callback,
        request: host::BashRequest,
        resources: host::BashCapabilities,
    ) -> ExtensionResult<Option<f64>> {
        route::bash(handler.id(), request, resources).await
    }
}
