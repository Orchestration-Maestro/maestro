//! The generated guest exports, each a single call into the shared adapter logic.

use super::super::tools::ToolCapabilities;
use super::super::{Exports, release};
use super::{Generated, Glue};
use crate::AutocompleteItem;
use crate::bindings::exports::maestro::extension::guest::ToolInvocation;
use crate::bindings::exports::maestro::extension::guest::{self, Guest};
use crate::bindings::maestro::extension::host;
use crate::loader::Extension;
use crate::types::ExtensionResult;
use std::future::Future;

impl<E: Extension> Guest for Glue<E> {
    async fn invoke_oauth_callback(
        handler: &host::Callback,
        phase: host::OauthCallback,
        data: String,
        interaction: Option<host::OauthInteraction>,
    ) -> ExtensionResult<Option<String>> {
        let interaction = interaction.map(super::oauth::interaction);
        super::super::oauth::invoke(handler.id(), phase, &data, interaction).await
    }
    fn invoke_provider(
        handler: &host::Callback,
        data: String,
        input: host::ProviderInput,
    ) -> impl Future<Output = ExtensionResult<host::Callback>> {
        std::future::ready(super::provider_streams::invoke(handler.id(), &data, input))
    }
    async fn invoke_assistant(
        handler: &host::Callback,
        operation: host::AssistantOperation,
        data: Option<String>,
        signal: Option<host::AbortSignal>,
    ) -> ExtensionResult<Option<String>> {
        super::provider_streams::assistant(handler.id(), operation, data, signal).await
    }

    fn invoke_auth_backend(
        handler: &host::Callback,
        update: &host::AuthLockUpdate,
        asynchronous: bool,
    ) -> impl Future<Output = ExtensionResult<()>> {
        let target = handler.id();
        async move {
            if asynchronous {
                super::super::auth_storage::invoke_backend_async(&Generated, target, update).await
            } else {
                super::super::auth_storage::invoke_backend(&Generated, target, update)
            }
        }
    }
    fn invoke_auth_fallback(
        handler: &host::Callback,
        provider: String,
    ) -> impl Future<Output = ExtensionResult<Option<String>>> {
        std::future::ready(super::super::auth_storage::invoke_fallback(
            handler.id(),
            &provider,
        ))
    }
    async fn invoke_model_hook(
        handler: &host::Callback,
        phase: host::ModelHook,
        data: String,
        model: String,
        signal: Option<host::AbortSignal>,
    ) -> ExtensionResult<host::PayloadSelection> {
        super::model_streams::hook(handler.id(), phase, data, model, signal).await
    }

    fn invoke_stream(
        handler: &host::Callback,
        value: &host::Callback,
        operation: host::StreamOperation,
    ) -> impl Future<Output = ExtensionResult<host::StreamSelection>> {
        std::future::ready(super::streams::invoke(handler.id(), value.id(), operation))
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
        let request = super::super::renderers::RenderRequest {
            message: &request.message,
            expanded: request.expanded,
            theme: request.theme,
            emitter,
        };
        std::future::ready(
            Exports::new(Generated)
                .invoke_renderer(handler.id(), request)
                .map(|view| super::super::renderers::retain(&Generated, view)),
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
    fn component_focused(
        handler: &host::Callback,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<Option<bool>>> {
        std::future::ready(Exports::new(Generated).invoke_component(
            handler.id(),
            emitter,
            crate::Component::focused,
        ))
    }
    fn component_set_focused(
        handler: &host::Callback,
        focused: bool,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_component(
            handler.id(),
            emitter,
            |view, emitter| view.set_focused(focused, emitter),
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

    fn invoke_watch(
        handler: &host::Callback,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_watch(handler.id(), emitter))
    }

    fn invoke_abort(
        handler: &host::Callback,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_abort(handler.id(), emitter))
    }

    fn invoke_custom_factory(
        handler: &host::Callback,
        input: host::CustomInput,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<host::CustomBuilt>> {
        let lent = super::super::custom::Lent {
            tui: input.tui,
            theme: input.theme,
            keybindings: input.keybindings,
            close: input.close,
        };
        std::future::ready(
            Exports::new(Generated)
                .invoke_custom_factory(handler.id(), lent, emitter)
                .map(|outcome| super::super::custom::transfer(&Generated, outcome)),
        )
    }
    async fn invoke_custom_future(
        handler: &host::Callback,
    ) -> ExtensionResult<host::FactoryComponent> {
        super::super::custom::invoke_custom_future(handler.id())
            .await
            .map(|built| super::super::factories::transfer(&Generated, built))
    }
    fn invoke_component_factory(
        handler: &host::Callback,
        tui: host::Tui,
        theme: host::Theme,
        footer: Option<host::FooterData>,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<host::FactoryComponent>> {
        let lent = super::super::factories::Lent { tui, theme, footer };
        let built = Exports::new(Generated).invoke_component_factory(handler.id(), lent, emitter);
        std::future::ready(built.map(|built| super::super::factories::transfer(&Generated, built)))
    }

    fn invoke_tool_render(
        handler: &host::Callback,
        render: host::ToolRender,
        theme: host::Theme,
        invalidator: host::ToolInvalidator,
        native_row: Option<host::NativeRow>,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<host::ToolRendered>> {
        let exports = Exports::new(Generated);
        let rendered = exports.invoke_tool_render(
            handler.id(),
            &render,
            (
                theme,
                invalidator,
                native_row.map(|row| crate::NativeRow {
                    handle: std::rc::Rc::new(row),
                }),
            ),
            emitter,
        );
        std::future::ready(rendered.map(super::rendered_row))
    }

    forward_resources! { static functions {
        invoke_grep(handler: &host::Callback, operation: host::GrepOperation, path: String) -> impl Future<Output = Result<host::GrepOutput, host::DiagnosticErrorInfo>> => super::super::tools::grep::invoke[handler.id(), operation, path];
        invoke_ls(handler: &host::Callback, operation: host::LsOperation, path: String) -> impl Future<Output = Result<host::LsOutput, host::DiagnosticErrorInfo>> => super::super::tools::ls::invoke[handler.id(), operation, path];
        invoke_find(handler: &host::Callback, operation: host::FindOperation, path: String) -> impl Future<Output = Result<host::FindOutput, host::DiagnosticErrorInfo>> => super::super::tools::find::invoke[handler.id(), operation, path];
        invoke_edit(handler: &host::Callback, operation: host::EditOperation, path: String) -> impl Future<Output = Result<host::ReadOutput, host::DiagnosticErrorInfo>> => super::super::tools::edit::invoke[handler.id(), operation, path];
        invoke_write(handler: &host::Callback, operation: host::WriteOperation, path: String) -> impl Future<Output = Result<(), host::DiagnosticErrorInfo>> => super::super::tools::write::invoke[handler.id(), operation, path];
        invoke_read(handler: &host::Callback, operation: host::ReadOperation, path: String) -> impl Future<Output = Result<host::ReadOutput, host::DiagnosticErrorInfo>> => super::super::tools::read::invoke[handler.id(), operation, path];
    } }

    fn invoke_branch_change(
        handler: &host::Callback,
        _: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(super::super::factories::invoke_branch_change(handler.id()))
    }

    fn invoke_set_expanded(
        handler: &host::Callback,
        expanded: bool,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_set_expanded(
            handler.id(),
            expanded,
            emitter,
        ))
    }

    fn invoke_editor_factory(
        handler: &host::Callback,
        tui: host::Tui,
        theme: host::EditorTheme,
        keybindings: host::KeybindingsManager,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<host::EditorBuilt>> {
        std::future::ready(super::editors::editor_factory(
            handler.id(),
            tui,
            theme,
            keybindings,
            emitter,
        ))
    }

    async fn invoke_autocomplete(
        handler: &host::Callback,
        request: host::ProviderRequest,
        signal: Option<host::AbortSignal>,
    ) -> ExtensionResult<host::ProviderAnswer> {
        super::editors::provider(handler.id(), request, signal).await
    }

    fn invoke_autocomplete_factory(
        handler: &host::Callback,
        current: host::CompletionProvider,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<host::Callback>> {
        std::future::ready(super::editors::provider_factory(
            handler.id(),
            current,
            emitter,
        ))
    }

    fn invoke_editor(
        handler: &host::Callback,
        request: host::AuthoredEditorRequest,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<host::AuthoredEditorAnswer>> {
        std::future::ready(super::editors::authored(handler.id(), request, emitter))
    }

    fn invoke_dispose(
        handler: &host::Callback,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<()>> {
        std::future::ready(Exports::new(Generated).invoke_dispose(handler.id(), emitter))
    }

    fn invoke_style(
        handler: &host::Callback,
        text: String,
    ) -> impl Future<Output = ExtensionResult<String>> {
        std::future::ready(super::super::invoke_style(handler.id(), &text))
    }

    fn invoke_highlight(
        handler: &host::Callback,
        code: String,
        lang: Option<String>,
    ) -> impl Future<Output = ExtensionResult<Vec<String>>> {
        std::future::ready(super::super::invoke_highlight(
            handler.id(),
            &code,
            lang.as_deref(),
        ))
    }

    fn invoke_widget(
        handler: &host::Callback,
        event: host::WidgetEvent,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<host::WidgetAnswer>> {
        std::future::ready(super::widget_events::invoke_widget(
            handler.id(),
            event,
            emitter,
        ))
    }

    fn invoke_terminal_input(
        handler: &host::Callback,
        data: String,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<Option<host::TerminalInputResult>>> {
        std::future::ready(Exports::new(Generated).invoke_terminal_input(
            handler.id(),
            &data,
            emitter,
        ))
    }

    async fn start() -> ExtensionResult<()> {
        super::super::install(super::widgets::widget_host());
        // The standard-library libc otherwise starts its process cwd at `/`.
        if let Some(cwd) = crate::wasi::cli::environment::get_initial_cwd() {
            std::env::set_current_dir(cwd).map_err(|error| error.to_string())?;
        }
        Exports::new(Generated).start::<E>().await
    }

    fn release(handler: &host::Callback) {
        release(handler.id());
    }

    fn invoke_prepare(
        handler: &host::Callback,
        args: String,
        emitter: &host::CallbackEmitter,
    ) -> impl Future<Output = ExtensionResult<host::PreparedArguments>> {
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
        super::route::event(handler.id(), event, resources).await
    }
    async fn bash_exec(
        handler: &host::Callback,
        request: host::BashRequest,
        resources: host::BashCapabilities,
    ) -> ExtensionResult<Option<f64>> {
        super::route::bash(handler.id(), request, resources).await
    }
}
