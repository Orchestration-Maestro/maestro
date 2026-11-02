//! The generated imports and exports behind the shared adapter logic: single calls into the
//! bindings, compiled only for the component target.

use std::marker::PhantomData;

use super::Imports;
use super::imports::Dialog;
use crate::bindings::maestro::extension::host;
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::types::{ExtensionFuture, ExtensionResult};
use crate::{
    ExtensionWidgetOptions, FlagOptions, FlagValue, NotificationType, WorkingIndicatorOptions,
};

#[path = "generated_oauth.rs"]
mod oauth;
#[path = "generated_route.rs"]
mod route;
#[path = "generated_streams.rs"]
mod streams;
pub(crate) use streams::new_event_stream;

#[path = "tools/builtin.rs"]
#[macro_use]
mod builtin;

#[path = "generated_glue.rs"]
mod glue;
#[path = "generated_model_streams.rs"]
pub(crate) mod model_streams;

#[path = "generated_widgets.rs"]
mod widgets;

#[path = "generated_overlays.rs"]
#[macro_use]
mod overlays;

#[path = "generated_widget_events.rs"]
mod widget_events;

#[path = "generated_writer.rs"]
mod writer;

#[path = "generated_keys.rs"]
mod keys;

#[path = "generated_editors.rs"]
mod editors;
#[path = "generated_provider_streams.rs"]
pub(crate) mod provider_streams;

/// The generated host imports and resources.
#[derive(Clone, Copy)]
struct Generated;

#[path = "generated_native.rs"]
mod native;

impl Imports for Generated {
    builtin_tool_forwarding!();

    type OAuthProvider = host::OauthProvider;
    fn credentials_oauth_providers(
        &self,
        owner: &Self::Credentials,
    ) -> ExtensionResult<Vec<Self::OAuthProvider>> {
        owner.get_oauth_providers()
    }
    fn oauth_info(&self, provider: &Self::OAuthProvider) -> ExtensionResult<String> {
        provider.info()
    }
    fn oauth_login<'a>(
        &'a self,
        provider: &'a Self::OAuthProvider,
        handler: &'a Self::Callback,
        (options, signal): ((bool, bool, bool), Option<&'a Self::Signal>),
    ) -> ExtensionFuture<'a, String> {
        Box::pin(provider.login(handler, options, signal))
    }
    fn credentials_login<'a>(
        &'a self,
        owner: &'a Self::Credentials,
        provider: String,
        handler: &'a Self::Callback,
        (options, signal): ((bool, bool, bool), Option<&'a Self::Signal>),
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(owner.login(provider, handler, options, signal))
    }
    fn oauth_refresh<'a>(
        &'a self,
        provider: &'a Self::OAuthProvider,
        credentials: String,
    ) -> ExtensionFuture<'a, String> {
        Box::pin(provider.refresh_token(credentials))
    }
    fn oauth_models(&self, provider: &Self::OAuthProvider, data: &str) -> ExtensionResult<String> {
        route::text(&provider.modify_models(data))
    }
    fn oauth_key(
        &self,
        provider: &Self::OAuthProvider,
        credentials: &str,
    ) -> ExtensionResult<String> {
        route::text(&provider.get_api_key(credentials))
    }

    fn watch(&self, path: String, listener: &Self::Callback) -> ExtensionResult<Self::Watcher> {
        host::watch(&path, listener)
    }
    forward_resources! { routed {
        registry_refresh(resource: &Self::Registry) => refresh();
        registry_register_provider(resource: &Self::Registry, name: &str, config: &str, callbacks: super::imports::ProviderRegistration<'_, Self::Callback>) => register_provider(name, config, callbacks);
        registry_unregister_provider(resource: &Self::Registry, name: &str) => unregister_provider(name);
    } }
    forward_resources! { routed functions {
        register_provider(name: &str, config: &str, callbacks: super::imports::ProviderRegistration<'_, Self::Callback>) => host::register_provider;
        unregister_provider(name: &str) => host::unregister_provider;
    } }
    fn registry_get_available(&self, owner: &Self::Registry) -> ExtensionResult<String> {
        route::text(&owner.get_available())
    }
    fn registry_has_configured_auth(
        &self,
        owner: &Self::Registry,
        model: &str,
    ) -> ExtensionResult<bool> {
        route::boolean(&owner.has_configured_auth(model))
    }
    fn registry_get_provider_auth_status(
        &self,
        owner: &Self::Registry,
        provider: &str,
    ) -> ExtensionResult<String> {
        route::text(&owner.get_provider_auth_status(provider))
    }
    type LockUpdate = host::AuthLockUpdate;
    fn lock_update(
        &self,
        update: &Self::LockUpdate,
        current: Option<&str>,
    ) -> ExtensionResult<Option<String>> {
        update.update(current)
    }
    fn lock_update_async<'a>(
        &'a self,
        update: &'a Self::LockUpdate,
        current: Option<String>,
    ) -> ExtensionFuture<'a, Option<String>> {
        Box::pin(async move { update.update_async(current).await })
    }
    fn auth_storage_from_backend(
        &self,
        backend: &Self::Callback,
    ) -> ExtensionResult<Self::Credentials> {
        route::credentials(&host::auth_storage_from_backend(backend))
    }
    fn credentials_set_fallback(
        &self,
        owner: &Self::Credentials,
        resolver: &Self::Callback,
    ) -> ExtensionResult<()> {
        route::drive(&owner.set_fallback_resolver(resolver))
    }
    fn credentials_has_auth(
        &self,
        owner: &Self::Credentials,
        provider: &str,
    ) -> ExtensionResult<bool> {
        route::boolean(&owner.has_auth(provider))
    }
    fn credentials_get_auth_status(
        &self,
        owner: &Self::Credentials,
        provider: &str,
    ) -> ExtensionResult<String> {
        route::text(&owner.get_auth_status(provider))
    }
    type Credentials = host::AuthStorage;
    forward_resources! { routed values {
        credentials_get(resource: &Self::Credentials, provider: &str) -> Option<String> => get(provider) via route::optional_text;
        credentials_list(resource: &Self::Credentials) -> Vec<String> => list() via route::lines;
        credentials_get_all(resource: &Self::Credentials) -> String => get_all() via route::text;
        registry_is_using_oauth(resource: &Self::Registry, model: &str) -> bool => is_using_oauth(model) via route::boolean;
    } }
    forward_resources! { routed value functions {
        create_model_registry(auth: &Self::Credentials, path: Option<&str>) -> Self::Registry => host::create_model_registry => route::registry;
        in_memory_model_registry(auth: &Self::Credentials) -> Self::Registry => host::in_memory_model_registry => route::registry;
    } }
    forward_resources! { routed {
        credentials_reload(resource: &Self::Credentials) => reload();
        credentials_set(resource: &Self::Credentials, provider: &str, credential: &str) => set(provider, credential);
        credentials_remove(resource: &Self::Credentials, provider: &str) => remove(provider);
        credentials_logout(resource: &Self::Credentials, provider: &str) => logout(provider);
    } }
    forward_resources! { functions {
        create_auth_storage(path: Option<&str>) -> ExtensionResult<Self::Credentials> => host::create_auth_storage;
        auth_storage_in_memory(data: &str) -> ExtensionResult<Self::Credentials> => host::auth_storage_in_memory;
    } }

    type JsonNode = host::JsonNode;
    fn get_all_tools(&self) -> ExtensionResult<super::ToolListing<Self::Callback>> {
        let listing = host::get_all_tools()?;
        Ok((
            listing.metadata,
            listing
                .schemas
                .into_iter()
                .map(|b| (b.tool, b.pointer, b.handler, b.shape, b.local))
                .collect(),
        ))
    }

    fn ui_get_editor_component(
        &self,
        ui: &host::ExtensionUiContext,
    ) -> ExtensionResult<super::CurrentFactory> {
        Ok(ui.get_editor_component()?.map(|current| match current {
            host::CurrentEditorFactory::Authored(id) => Ok(id),
            host::CurrentEditorFactory::Host(factory) => {
                Err(std::rc::Rc::new(factory) as std::rc::Rc<dyn std::any::Any>)
            }
        }))
    }

    fn schema_invoke(
        &self,
        callback: &Self::Callback,
        operation: host::SchemaOperation,
        node: &Self::JsonNode,
        path: &str,
    ) -> crate::schema_code::SchemaResult<String> {
        let invocation = callback.schema_invoke(operation, node, path);
        route::drive(&invocation).map_err(crate::schema_code::DiagnosticErrorInfo::error)?;
        invocation
            .schema_result()
            .map_err(crate::schema_error::from_wire)
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
        ui_paste_to_editor(resource: &Self::Ui, text: &str) => paste_to_editor(text);
        ui_set_editor_text(resource: &Self::Ui, text: &str) => set_editor_text(text);
        ui_set_title(resource: &Self::Ui, title: &str) => set_title(title);
        ui_set_widget_factory(resource: &Self::Ui, key: &str, factory: &Self::Callback, options: Option<ExtensionWidgetOptions>) => set_widget_factory(key, factory, options);
        ui_set_header(resource: &Self::Ui, factory: Option<&Self::Callback>) => set_header(factory);
        ui_set_footer(resource: &Self::Ui, factory: Option<&Self::Callback>) => set_footer(factory);
        ui_set_editor_component(resource: &Self::Ui, factory: Option<&Self::Callback>) => set_editor_component(factory);
        ui_add_autocomplete_provider(resource: &Self::Ui, factory: &Self::Callback) => add_autocomplete_provider(factory);
        ui_set_tools_expanded(resource: &Self::Ui, expanded: bool) => set_tools_expanded(expanded);
        abort(context: &Self::Context) => abort();
        shutdown(context: &Self::Context) => shutdown();
        command_abort(context: &Self::CommandContext) => abort();
        command_shutdown(context: &Self::CommandContext) => shutdown();
        tool_update(update: &Self::Update, partial: &str) => update(partial);
        signal_abort(signal: &Self::Signal) => abort();
    } }
    forward_resources! { functions {
        native_format_size(bytes: f64) -> ExtensionResult<String> => host::native_format_size;
        native_frontmatter(content: &str) -> ExtensionResult<host::FrontmatterDocument> => host::native_frontmatter;
        native_yaml(content: &str) -> ExtensionResult<Vec<host::FrontmatterToken>> => host::native_yaml;
        native_helper(operation: &str, data: &str) -> ExtensionResult<String> => host::native_helper;
        get_session_name() -> ExtensionResult<Option<String>> => host::get_session_name;
        get_active_tools() -> ExtensionResult<Vec<String>> => host::get_active_tools;
        get_commands() -> ExtensionResult<String> => host::get_commands;
        get_thinking_level() -> ExtensionResult<String> => host::get_thinking_level;
        event_on(channel: &str, handler: &Self::Callback) -> ExtensionResult<Self::Subscription> => host::event_on;
        signal_subscribe(signal: &Self::Signal, listener: &Self::Callback) -> ExtensionResult<Self::Subscription> => host::signal_subscribe;
        new_signal() -> ExtensionResult<Self::Signal> => host::new_signal;
        start_event_tail(emitter: &Self::Emitter, tail: &Self::Callback) -> ExtensionResult<()> => host::start_event_tail;
        register_message_renderer(custom_type: &str, handler: &host::Callback) -> ExtensionResult<()> => host::register_message_renderer;
        on(event: &str, handler: &host::Callback) -> ExtensionResult<()> => host::on;
        register_command(name: &str, description: Option<&str>, completions: Option<&host::Callback>, handler: &host::Callback) -> ExtensionResult<()> => host::register_command;
        register_shortcut(shortcut: &str, description: Option<&str>, handler: &host::Callback) -> ExtensionResult<()> => host::register_shortcut;
        contribute_load_time(contributions: &str) -> ExtensionResult<()> => host::contribute_load_time;
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
        registry_auth_storage(resource: &Self::Registry) -> ExtensionResult<Self::Credentials> => auth_storage();
        credentials_set_runtime_api_key(resource: &Self::Credentials, provider: &str, key: &str) -> ExtensionResult<()> => set_runtime_api_key(provider, key);
        credentials_remove_runtime_api_key(resource: &Self::Credentials, provider: &str) -> ExtensionResult<()> => remove_runtime_api_key(provider);
        credentials_has(resource: &Self::Credentials, provider: &str) -> ExtensionResult<bool> => has(provider);
        credentials_drain_errors(resource: &Self::Credentials) -> ExtensionResult<String> => drain_errors();
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
        registry_find(resource: &Self::Registry, provider: &str, model_id: &str) -> ExtensionResult<Option<String>> => find(provider, model_id);
        registry_get_error(resource: &Self::Registry) -> ExtensionResult<Option<String>> => get_error();
        registry_get_provider_display_name(resource: &Self::Registry, provider: &str) -> ExtensionResult<String> => get_provider_display_name(provider);
        ui(context: &Self::Context) -> ExtensionResult<Self::Ui> => ui();
        theme_name(theme: &Self::Theme) -> ExtensionResult<Option<String>> => name();
        theme_source_path(theme: &Self::Theme) -> ExtensionResult<Option<String>> => source_path();
        theme_source_info(theme: &Self::Theme) -> ExtensionResult<Option<String>> => source_info();
        theme_set_source_info(theme: &Self::Theme, source: Option<&str>) -> ExtensionResult<()> => set_source_info(source);
        theme_fg(theme: &Self::Theme, color: &str, text: &str) -> ExtensionResult<String> => fg(color, text);
        theme_bg(theme: &Self::Theme, color: &str, text: &str) -> ExtensionResult<String> => bg(color, text);
        theme_bold(theme: &Self::Theme, text: &str) -> ExtensionResult<String> => bold(text);
        theme_italic(theme: &Self::Theme, text: &str) -> ExtensionResult<String> => italic(text);
        theme_underline(theme: &Self::Theme, text: &str) -> ExtensionResult<String> => underline(text);
        theme_inverse(theme: &Self::Theme, text: &str) -> ExtensionResult<String> => inverse(text);
        theme_strikethrough(theme: &Self::Theme, text: &str) -> ExtensionResult<String> => strikethrough(text);
        theme_get_fg_ansi(theme: &Self::Theme, color: &str) -> ExtensionResult<String> => get_fg_ansi(color);
        theme_get_bg_ansi(theme: &Self::Theme, color: &str) -> ExtensionResult<String> => get_bg_ansi(color);
        theme_get_color_mode(theme: &Self::Theme) -> ExtensionResult<crate::ColorMode> => get_color_mode();
        theme_thinking_border(theme: &Self::Theme, level: &str, text: &str) -> ExtensionResult<String> => thinking_border(level, text);
        theme_bash_mode_border(theme: &Self::Theme, text: &str) -> ExtensionResult<String> => bash_mode_border(text);
        ui_get_editor_text(resource: &Self::Ui) -> ExtensionResult<String> => get_editor_text();
        ui_on_terminal_input(resource: &Self::Ui, handler: &Self::Callback) -> ExtensionResult<Self::Subscription> => on_terminal_input(handler);
        ui_theme(resource: &Self::Ui) -> ExtensionResult<Self::Theme> => theme();
        ui_get_all_themes(resource: &Self::Ui) -> ExtensionResult<Vec<crate::ThemeInfo>> => get_all_themes();
        ui_get_theme(resource: &Self::Ui, name: &str) -> ExtensionResult<Option<Self::Theme>> => get_theme(name);
        ui_set_theme_name(resource: &Self::Ui, name: &str) -> ExtensionResult<crate::ThemeSetResult> => set_theme_name(name);
        ui_set_theme(resource: &Self::Ui, theme: &Self::Theme) -> ExtensionResult<crate::ThemeSetResult> => set_theme(theme);
        ui_get_tools_expanded(resource: &Self::Ui) -> ExtensionResult<bool> => get_tools_expanded();
        invalidate(invalidator: &Self::Invalidator) -> ExtensionResult<()> => invalidate();
        footer_get_git_branch(footer: &Self::FooterData) -> ExtensionResult<Option<String>> => get_git_branch();
        footer_get_extension_statuses(footer: &Self::FooterData) -> ExtensionResult<Vec<(String, String)>> => get_extension_statuses();
        footer_get_available_provider_count(footer: &Self::FooterData) -> ExtensionResult<f64> => get_available_provider_count();
        footer_on_branch_change(footer: &Self::FooterData, listener: &Self::Callback) -> ExtensionResult<Self::Subscription> => on_branch_change(listener);
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
    type Writer = host::SessionManager;

    forward_resources! { asynchronous functions {
        call<'a>(operation: &'a str, data: String, signal: Option<&'a Self::Signal>) -> String => host::call[operation.to_owned(), data, signal];
        set_model<'a>(model: String) -> bool => host::set_model[model];
        event_emit<'a>(channel: String, data: String) -> () => host::event_emit[channel, data];
    } }

    type Subscription = host::Subscription;
    type Emitter = host::CallbackEmitter;
    type Ui = host::ExtensionUiContext;
    overlay_forwarding!();
    type Tui = host::Tui;
    fn tui_full_redraws(&self, tui: &host::Tui) -> ExtensionResult<u64> {
        tui.full_redraws()
    }
    type Invalidator = host::ToolInvalidator;
    fn register_tool(
        &self,
        metadata: &str,
        callbacks: super::imports::ToolCallbacks<'_, host::Callback>,
        schemas: &[(String, &host::Callback)],
    ) -> ExtensionResult<()> {
        let (prepare, execute) = (callbacks.prepare, callbacks.execute);
        let renderers = (callbacks.render_call, callbacks.render_result);
        route::drive(&host::register_tool(
            metadata,
            prepare,
            execute,
            schemas,
            renderers.0,
            renderers.1,
        ))
    }
    type FooterData = host::FooterData;

    forward_resources! { asynchronous {
        ui_select<'a>(resource: Self::Ui, title: String, options: Vec<String>, dialog: Dialog<'a, Self::Signal>) -> Option<String> => select(title, options, dialog.signal, dialog.timeout);
        ui_confirm<'a>(resource: Self::Ui, title: String, message: String, dialog: Dialog<'a, Self::Signal>) -> bool => confirm(title, message, dialog.signal, dialog.timeout);
        ui_input<'a>(resource: Self::Ui, title: String, placeholder: Option<String>, dialog: Dialog<'a, Self::Signal>) -> Option<String> => input(title, placeholder, dialog.signal, dialog.timeout);
        ui_editor<'a>(resource: Self::Ui, title: String, prefill: Option<String>) -> Option<String> => editor(title, prefill);
        fork<'a>(context: Self::CommandContext, entry_id: &'a str, data: Option<String>, with_session: Option<&'a Self::Callback>) -> SessionChangeResult => fork(entry_id.to_owned(), data, with_session);
        navigate_tree<'a>(context: Self::CommandContext, target_id: &'a str, options: Option<String>) -> SessionChangeResult => navigate_tree(target_id.to_owned(), options);
        switch_session<'a>(context: Self::CommandContext, path: &'a str, options: Option<()>, with_session: Option<&'a Self::Callback>) -> SessionChangeResult => switch_session(path.to_owned(), options.map(|()| host::SwitchSessionOptions { with_session }));
        new_session<'a>(context: Self::CommandContext, session_data: Option<NewSessionCommandData>, setup: Option<&'a Self::Callback>, with_session: Option<&'a Self::Callback>) -> SessionChangeResult => new_session(session_data, setup, with_session);
        reload<'a>(context: Self::CommandContext) -> () => reload();
        wait_for_idle<'a>(context: Self::CommandContext) -> () => wait_for_idle();
        replaced_send_message<'a>(context: Self::ReplacedContext, message: String, options: Option<String>) -> () => send_message(message, options);
        replaced_send_user_message<'a>(target: Self::ReplacedContext, content: String, options: Option<String>) -> () => send_user_message(content, options);
    } }
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
                    base_url: auth.base_url,
                },
                host::ResolvedRequestAuth::Error(error) => {
                    crate::ResolvedRequestAuth::Error { error }
                }
            })
        })
    }

    fn credentials_get_api_key<'a>(
        &'a self,
        resource: &'a Self::Credentials,
        provider: String,
        include_fallback: Option<bool>,
    ) -> ExtensionFuture<'a, Option<String>> {
        Box::pin(resource.get_api_key(provider, include_fallback))
    }
    fn registry_get_api_key_for_provider<'a>(
        &'a self,
        resource: &'a Self::Registry,
        provider: String,
    ) -> ExtensionFuture<'a, Option<String>> {
        Box::pin(resource.get_api_key_for_provider(provider))
    }

    type BashOutput = host::BashOutput;
    type Update = host::ToolUpdate;
    type Callback = host::Callback;
    type Theme = host::Theme;
    type Context = host::Context;
    type CommandContext = host::CommandContext;
    type ReplacedContext = host::ReplacedSessionContext;

    type Reader = host::ReadonlySessionManager;
    type Node = host::SessionTreeNode;

    fn new_callback(&self) -> (u32, host::Callback) {
        let handle = host::Callback::new();
        (handle.id(), handle)
    }
}

/// Transfers a rendered component and a new row's state to the host as their identities.
fn rendered_row(
    (component, row): (
        crate::ExtensionResult<crate::Component>,
        Option<std::rc::Rc<crate::types::ToolRow>>,
    ),
) -> host::ToolRendered {
    let identity = || super::callbacks::Identity::<Generated>::new(&Generated);
    host::ToolRendered {
        component: component.map(|component| identity().transfer(component)),
        row: row.map(|row| identity().transfer(row)),
    }
}

/// Entry type of an extension component: serves the generated exports for `E`.
#[doc(hidden)]
pub struct Glue<E>(PhantomData<E>);
