//! The generated imports and exports behind the shared adapter logic: single calls into the
//! bindings, compiled only for the component target.

use std::{future::Future, marker::PhantomData};

use super::tools::ToolCapabilities;
use super::{Capabilities, Exports, Imports, release};
use crate::bindings::exports::maestro::extension::guest::ToolInvocation;
use crate::bindings::exports::maestro::extension::guest::{self, Guest};
use crate::bindings::maestro::extension::host;
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::loader::Extension;
use crate::types::{ExtensionFuture, ExtensionResult};
use crate::{
    AutocompleteItem, ExtensionWidgetOptions, FlagOptions, FlagValue, NotificationType,
    WorkingIndicatorOptions,
};

#[path = "generated_route.rs"]
mod route;

/// The generated host imports and resources.
#[derive(Clone, Copy)]
struct Generated;

impl Imports for Generated {
    type JsonNode = host::JsonNode;
    fn get_all_tools(&self) -> ExtensionResult<super::ToolListing<Self::Callback>> {
        let listing = host::get_all_tools()?;
        Ok((
            listing.metadata,
            listing
                .schemas
                .into_iter()
                .map(|b| (b.tool, b.pointer, b.handler))
                .collect(),
        ))
    }

    fn callback_emit(
        &self,
        emitter: &Self::Emitter,
        channel: &str,
        data: &str,
    ) -> ExtensionResult<()> {
        let (snapshot, tags) = host::callback_emit(emitter, channel, data)?;
        let result = route::walk_snapshot(emitter, snapshot, &tags, data);
        let end = host::callback_end_snapshot(emitter, snapshot);
        result.and(end)
    }

    forward_resources! { routed functions {
        send_message(message: &str, options: Option<&str>) => host::send_message;
        send_user_message(content: &str, options: Option<&str>) => host::send_user_message;
        set_session_name(name: &str) => host::set_session_name;
        set_label(entry_id: &str, label: Option<&str>) => host::set_label;
        set_active_tools(tool_names: &[String]) => host::set_active_tools;
        set_thinking_level(level: &str) => host::set_thinking_level;
        register_tool(metadata: &str, prepare: Option<&host::Callback>, execute: &host::Callback, schemas: &[(String, &host::Callback)]) => host::register_tool;
        append_entry(custom_type: &str, data: Option<&str>) => host::append_entry;
    } }
    forward_resources! { routed {
        compact(context: &Self::Context, instructions: Option<&host::CompactionData>, complete: Option<&Self::Callback>, error: Option<&Self::Callback>) => compact(instructions, complete, error);
        command_compact(context: &Self::CommandContext, instructions: Option<&host::CompactionData>, complete: Option<&Self::Callback>, error: Option<&Self::Callback>) => compact(instructions, complete, error);
        ui_notify(resource: &Self::Ui, message: &str, kind: Option<NotificationType>) => notify(message, kind);
        ui_set_status(resource: &Self::Ui, key: &str, text: Option<&str>) => set_status(key, text);
        ui_set_working_message(resource: &Self::Ui, message: Option<&str>) => set_working_message(message);
        ui_set_working_visible(resource: &Self::Ui, visible: bool) => set_working_visible(visible);
        ui_set_working_indicator(resource: &Self::Ui, options: Option<WorkingIndicatorOptions>) => set_working_indicator(options.as_ref());
        ui_set_hidden_thinking_label(resource: &Self::Ui, label: Option<&str>) => set_hidden_thinking_label(label);
        ui_set_widget(resource: &Self::Ui, key: &str, content: Option<&[String]>, options: Option<ExtensionWidgetOptions>) => set_widget(key, content, options);
        ui_set_title(resource: &Self::Ui, title: &str) => set_title(title);
        abort(context: &Self::Context) => abort();
        shutdown(context: &Self::Context) => shutdown();
        command_abort(context: &Self::CommandContext) => abort();
        command_shutdown(context: &Self::CommandContext) => shutdown();
        tool_update(update: &Self::Update, partial: &str) => update(partial);
    } }
    forward_resources! { functions {
        get_session_name() -> ExtensionResult<Option<String>> => host::get_session_name;
        get_active_tools() -> ExtensionResult<Vec<String>> => host::get_active_tools;
        get_commands() -> ExtensionResult<String> => host::get_commands;
        get_thinking_level() -> ExtensionResult<String> => host::get_thinking_level;
        event_on(channel: &str, handler: &Self::Callback) -> ExtensionResult<Self::Subscription> => host::event_on;
        start_event_tail(emitter: &Self::Emitter, tail: &Self::Callback) -> ExtensionResult<()> => host::start_event_tail;
        register_message_renderer(custom_type: &str, handler: &host::Callback) -> ExtensionResult<()> => host::register_message_renderer;
        on(event: &str, handler: &host::Callback) -> ExtensionResult<()> => host::on;
        register_command(name: &str, description: Option<&str>, completions: Option<&host::Callback>, handler: &host::Callback) -> ExtensionResult<()> => host::register_command;
        register_shortcut(shortcut: &str, description: Option<&str>, handler: &host::Callback) -> ExtensionResult<()> => host::register_shortcut;
        register_flag(name: &str, options: &FlagOptions) -> ExtensionResult<()> => host::register_flag;
        get_flag(name: &str) -> ExtensionResult<Option<FlagValue>> => host::get_flag;
    } }
    forward_resources! {
        json_kind(node: &Self::JsonNode, path: &str) -> ExtensionResult<Option<host::JsonKind>> => kind(path);
        json_read(node: &Self::JsonNode, path: &str) -> ExtensionResult<Option<String>> => read(path);
        json_keys(node: &Self::JsonNode, path: &str) -> ExtensionResult<Vec<String>> => keys(path);
        json_set(node: &Self::JsonNode, path: &str, key: &str, value: &str) -> ExtensionResult<()> => set(path, key, value);
        json_delete(node: &Self::JsonNode, path: &str, key: &str) -> ExtensionResult<()> => delete(path, key);
        json_splice(node: &Self::JsonNode, path: &str, range: (u32, u32), values: &[String]) -> ExtensionResult<Vec<String>> => splice(path, range, values);
        callback_id(callback: &Self::Callback) -> u32 => id();
        unsubscribe(subscription: &Self::Subscription) -> ExtensionResult<()> => unsubscribe();
        registry_get_all(resource: &Self::Registry) -> ExtensionResult<String> => get_all();
        bash_output(output: &Self::BashOutput, bytes: &[u8]) -> ExtensionResult<()> => on_data(bytes);
        aborted(signal: &host::AbortSignal) -> bool => aborted();
        writer_reader(resource: &Self::Writer) -> Self::Reader => reader();
        writer_set_session_file(resource: &Self::Writer, path: &str) -> ExtensionResult<()> => set_session_file(path);
        writer_new_session(resource: &Self::Writer, options: Option<&str>) -> ExtensionResult<Option<String>> => new_session(options);
        writer_is_persisted(resource: &Self::Writer) -> ExtensionResult<bool> => is_persisted();
        writer_get_children(resource: &Self::Writer, parent_id: &str) -> ExtensionResult<String> => get_children(parent_id);
        writer_build_session_context(resource: &Self::Writer) -> ExtensionResult<String> => build_session_context();
        writer_append_message(resource: &Self::Writer, message: &str) -> ExtensionResult<String> => append_message(message);
        writer_append_thinking_level_change(resource: &Self::Writer, level: &str) -> ExtensionResult<String> => append_thinking_level_change(level);
        writer_append_model_change(resource: &Self::Writer, provider: &str, model_id: &str) -> ExtensionResult<String> => append_model_change(provider, model_id);
        writer_append_compaction(resource: &Self::Writer, compaction: &str, from_hook: Option<bool>) -> ExtensionResult<String> => append_compaction(compaction, from_hook);
        writer_append_custom_entry(resource: &Self::Writer, custom_type: &str, data: Option<&str>) -> ExtensionResult<String> => append_custom_entry(custom_type, data);
        writer_append_session_info(resource: &Self::Writer, name: &str) -> ExtensionResult<String> => append_session_info(name);
        writer_append_custom_message_entry(resource: &Self::Writer, message: &str) -> ExtensionResult<String> => append_custom_message_entry(message);
        writer_append_label_change(resource: &Self::Writer, target_id: &str, label: Option<&str>) -> ExtensionResult<String> => append_label_change(target_id, label);
        writer_branch(resource: &Self::Writer, from_id: &str) -> ExtensionResult<()> => branch(from_id);
        writer_reset_leaf(resource: &Self::Writer) -> ExtensionResult<()> => reset_leaf();
        writer_branch_with_summary(resource: &Self::Writer, from_id: Option<&str>, summary: &str, metadata: (Option<&str>, Option<bool>)) -> ExtensionResult<String> => branch_with_summary(from_id, summary, metadata);
        writer_create_branched_session(resource: &Self::Writer, leaf_id: &str) -> ExtensionResult<Option<String>> => create_branched_session(leaf_id);
        registry_get_available(resource: &Self::Registry) -> ExtensionResult<String> => get_available();
        registry_find(resource: &Self::Registry, provider: &str, model_id: &str) -> ExtensionResult<Option<String>> => find(provider, model_id);
        ui(context: &Self::Context) -> ExtensionResult<Self::Ui> => ui();
        has_ui(context: &Self::Context) -> ExtensionResult<bool> => has_ui();
        model_registry(context: &Self::Context) -> ExtensionResult<Self::Registry> => model_registry();
        model(context: &Self::Context) -> ExtensionResult<Option<String>> => model();
        is_idle(context: &Self::Context) -> ExtensionResult<bool> => is_idle();
        signal(context: &Self::Context) -> ExtensionResult<Option<Self::Signal>> => signal();
        has_pending_messages(context: &Self::Context) -> ExtensionResult<bool> => has_pending_messages();
        get_context_usage(context: &Self::Context) -> ExtensionResult<Option<crate::ContextUsage>> => get_context_usage();
        get_system_prompt(context: &Self::Context) -> ExtensionResult<String> => get_system_prompt();
        command_ui(context: &Self::CommandContext) -> ExtensionResult<Self::Ui> => ui();
        command_has_ui(context: &Self::CommandContext) -> ExtensionResult<bool> => has_ui();
        command_model_registry(context: &Self::CommandContext) -> ExtensionResult<Self::Registry> => model_registry();
        command_model(context: &Self::CommandContext) -> ExtensionResult<Option<String>> => model();
        command_is_idle(context: &Self::CommandContext) -> ExtensionResult<bool> => is_idle();
        command_signal(context: &Self::CommandContext) -> ExtensionResult<Option<Self::Signal>> => signal();
        command_has_pending_messages(context: &Self::CommandContext) -> ExtensionResult<bool> => has_pending_messages();
        command_get_context_usage(context: &Self::CommandContext) -> ExtensionResult<Option<crate::ContextUsage>> => get_context_usage();
        command_get_system_prompt(context: &Self::CommandContext) -> ExtensionResult<String> => get_system_prompt();
        session_manager(context: &Self::Context) -> ExtensionResult<Self::Reader> => session_manager();
        command_session_manager(context: &Self::CommandContext) -> ExtensionResult<Self::Reader> => session_manager();
        reader_get_cwd(resource: &Self::Reader) -> ExtensionResult<String> => get_cwd();
        reader_get_session_dir(resource: &Self::Reader) -> ExtensionResult<String> => get_session_dir();
        reader_get_session_id(resource: &Self::Reader) -> ExtensionResult<String> => get_session_id();
        reader_get_session_file(resource: &Self::Reader) -> ExtensionResult<Option<String>> => get_session_file();
        reader_get_leaf_id(resource: &Self::Reader) -> ExtensionResult<Option<String>> => get_leaf_id();
        reader_get_leaf_entry(resource: &Self::Reader) -> ExtensionResult<Option<String>> => get_leaf_entry();
        reader_get_entry(resource: &Self::Reader, id: &str) -> ExtensionResult<Option<String>> => get_entry(id);
        reader_get_label(resource: &Self::Reader, id: &str) -> ExtensionResult<Option<String>> => get_label(id);
        reader_get_branch(resource: &Self::Reader, from_id: Option<&str>) -> ExtensionResult<String> => get_branch(from_id);
        reader_get_header(resource: &Self::Reader) -> ExtensionResult<Option<String>> => get_header();
        reader_get_entries(resource: &Self::Reader) -> ExtensionResult<String> => get_entries();
        reader_get_tree(resource: &Self::Reader) -> ExtensionResult<Vec<Self::Node>> => get_tree();
        reader_get_session_name(resource: &Self::Reader) -> ExtensionResult<Option<String>> => get_session_name();
        node_entry(resource: &Self::Node) -> ExtensionResult<String> => entry();
        node_children(resource: &Self::Node) -> ExtensionResult<Vec<Self::Node>> => children();
        node_label(resource: &Self::Node) -> ExtensionResult<Option<String>> => label();
        node_label_timestamp(resource: &Self::Node) -> ExtensionResult<Option<String>> => label_timestamp();
        cwd(context: &host::Context) -> ExtensionResult<String> => cwd();
        command_cwd(context: &host::CommandContext) -> ExtensionResult<String> => cwd();
        command(context: &host::ReplacedSessionContext) -> host::CommandContext => command();
    }
    fn fork<'a>(
        &'a self,
        context: &'a Self::CommandContext,
        entry_id: &'a str,
        data: Option<String>,
        with_session: Option<&'a Self::Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(context.fork(entry_id.to_owned(), data, with_session))
    }
    fn navigate_tree<'a>(
        &'a self,
        context: &'a Self::CommandContext,
        target_id: &'a str,
        options: Option<String>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(context.navigate_tree(target_id.to_owned(), options))
    }
    fn switch_session<'a>(
        &'a self,
        context: &'a Self::CommandContext,
        path: &'a str,
        options: Option<()>,
        with_session: Option<&'a Self::Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(context.switch_session(
            path.to_owned(),
            options.map(|()| host::SwitchSessionOptions { with_session }),
        ))
    }
    fn reload<'a>(&'a self, context: &'a Self::CommandContext) -> ExtensionFuture<'a, ()> {
        Box::pin(context.reload())
    }
    fn replaced_send_message<'a>(
        &'a self,
        context: &'a Self::ReplacedContext,
        message: String,
        options: Option<String>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(context.send_message(message, options))
    }
    fn replaced_send_user_message<'a>(
        &'a self,
        target: &'a Self::ReplacedContext,
        content: String,
        options: Option<String>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(target.send_user_message(content, options))
    }

    type Writer = host::SessionManager;

    fn set_model(&self, model: String) -> ExtensionFuture<'_, bool> {
        Box::pin(host::set_model(model))
    }

    type Subscription = host::Subscription;
    type Emitter = host::CallbackEmitter;
    fn event_emit(&self, channel: String, data: String) -> ExtensionFuture<'_, ()> {
        Box::pin(host::event_emit(channel, data))
    }
    type Ui = host::ExtensionUiContext;
    type Registry = host::ModelRegistry;

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

    type BashOutput = host::BashOutput;
    type Update = host::ToolUpdate;
    type Callback = host::Callback;
    type Theme = host::Theme;
    type Signal = host::AbortSignal;
    type Context = host::Context;
    type CommandContext = host::CommandContext;
    type ReplacedContext = host::ReplacedSessionContext;

    type Reader = host::ReadonlySessionManager;
    type Node = host::SessionTreeNode;

    fn new_callback(&self) -> (u32, host::Callback) {
        let handle = host::Callback::new();
        (handle.id(), handle)
    }

    fn wait_for_idle<'a>(&'a self, context: &'a host::CommandContext) -> ExtensionFuture<'a, ()> {
        Box::pin(context.wait_for_idle())
    }

    fn new_session<'a>(
        &'a self,
        context: &'a host::CommandContext,
        session_data: Option<NewSessionCommandData>,
        setup: Option<&'a host::Callback>,
        with_session: Option<&'a host::Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(context.new_session(session_data, setup, with_session))
    }
}

/// Entry type of an extension component: serves the generated exports for `E`.
#[doc(hidden)]
pub struct Glue<E>(PhantomData<E>);

impl<E: Extension> Guest for Glue<E> {
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
        let request = super::renderers::RenderRequest {
            message: &request.message,
            expanded: request.expanded,
            theme: request.theme,
            emitter,
        };
        std::future::ready(
            Exports::new(Generated)
                .invoke_renderer(handler.id(), request)
                .map(|view| super::renderers::retain(&Generated, view)),
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
