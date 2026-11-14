//! A host operation that calls back into this instance hands each nested call to the guest,
//! which runs it with the same bodies the ordinary exports run.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use super::{Generated, host};
use crate::ExtensionResult;
use crate::component_adapter::tools::ToolCapabilities;
use crate::component_adapter::{
    Capabilities, Exports, bash::BashCapabilities, callbacks, renderers,
};

/// One nested call's result, ready at once or after its later continuation.
type Running = Pin<Box<dyn Future<Output = host::LocalResult>>>;

/// Completes one host operation, running each nested call it makes into this instance.
///
/// # Errors
/// Returns the host operation's own error.
pub(super) fn drive(route: &host::ImportRoute) -> ExtensionResult<()> {
    match value(route)? {
        host::RouteValue::Unit => Ok(()),
        _ => Err("extension route result kind mismatch".into()),
    }
}

/// Drives the same continuation while preserving a non-unit operation result.
pub(super) fn value(route: &host::ImportRoute) -> ExtensionResult<host::RouteValue> {
    loop {
        match route.next() {
            host::RouteStep::Done(result) => return result,
            host::RouteStep::Invoke(call) => start(route, dispatch(call)),
        }
    }
}

/// Completes a native operation that returns a list of strings, running each nested call it
/// makes into this instance.
///
/// # Errors
/// Returns the operation's own error.
pub(super) fn lines(route: &host::ImportRoute) -> ExtensionResult<Vec<String>> {
    match value(route)? {
        host::RouteValue::Lines(lines) => Ok(lines),
        _ => Err("extension route result kind mismatch".into()),
    }
}

/// Runs a nested call's prefix now; a call still pending continues in this guest's task and
/// delivers its result through its settlement, so the host caller is released at once.
fn start(route: &host::ImportRoute, mut running: Running) {
    match running
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => route.resume(result),
        Poll::Pending => {
            let settlement = route.defer();
            wit_bindgen::spawn_local(async move { settlement.finish(running.await) });
        }
    }
}

/// Selects the authored body of one nested call.
fn dispatch(call: host::LocalCall) -> Running {
    let exports = Exports::new(Generated);
    let result = match call {
        host::LocalCall::AuthFallback(call) => auth_fallback(&call),
        host::LocalCall::AuthBackend(call) => return backend(call),
        call @ (host::LocalCall::Renderer(_)
        | host::LocalCall::Render(_)
        | host::LocalCall::Unit(_)
        | host::LocalCall::Key(_)
        | host::LocalCall::Focused(_)
        | host::LocalCall::SetFocused(_)) => component(call),
        host::LocalCall::Prepare(call) => host::LocalResult::Prepared(exports.invoke_prepare(
            call.target,
            &call.args,
            &call.emitter,
        )),
        host::LocalCall::Schema(call) => host::LocalResult::Schema(
            exports
                .invoke_schema(call.target, call.operation, &call.node, &call.emitter)
                .map_err(crate::schema_error::to_wire),
        ),
        host::LocalCall::Stream(call) => host::LocalResult::Stream(super::streams::invoke(
            call.target,
            call.value,
            call.operation,
        )),
        host::LocalCall::Synchronous(call) => host::LocalResult::Synchronous(synchronous(&call)),
        call @ (host::LocalCall::Expanded(_)
        | host::LocalCall::Factory(_)
        | host::LocalCall::CustomFactory(_)
        | host::LocalCall::ToolRender(_)
        | host::LocalCall::TerminalInput(_)
        | host::LocalCall::Style(_)
        | host::LocalCall::Highlight(_)
        | host::LocalCall::Widget(_)
        | host::LocalCall::EditorFactory(_)
        | host::LocalCall::Editor(_)
        | host::LocalCall::AutocompleteFactory(_)) => ui(call),
        call => return asynchronous(call),
    };
    Box::pin(std::future::ready(result))
}

/// Runs one nested message renderer or retained component method.
fn component(call: host::LocalCall) -> host::LocalResult {
    let exports = Exports::new(Generated);
    match call {
        host::LocalCall::Renderer(call) => host::LocalResult::Renderer(
            exports
                .invoke_renderer(
                    call.target,
                    renderers::RenderRequest {
                        message: &call.message,
                        expanded: call.expanded,
                        theme: call.theme,
                        emitter: &call.emitter,
                    },
                )
                .map(|view| renderers::retain(&Generated, view)),
        ),
        host::LocalCall::Render(call) => host::LocalResult::Render(exports.invoke_component(
            call.target,
            &call.emitter,
            |view, emitter| view.render(call.width, emitter),
        )),
        host::LocalCall::Unit(call) => host::LocalResult::Unit(exports.invoke_component(
            call.target,
            &call.emitter,
            |view, emitter| match call.data.as_deref() {
                Some(data) => view.handle_input(data, emitter),
                None => view.invalidate(emitter),
            },
        )),
        host::LocalCall::Focused(call) => host::LocalResult::Focused(exports.invoke_component(
            call.target,
            &call.emitter,
            crate::Component::focused,
        )),
        host::LocalCall::SetFocused(call) => host::LocalResult::Unit(exports.invoke_component(
            call.target,
            &call.emitter,
            |view, emitter| view.set_focused(call.focused, emitter),
        )),
        host::LocalCall::Key(call) => host::LocalResult::Key(exports.invoke_component(
            call.target,
            &call.emitter,
            crate::Component::wants_key_release,
        )),
        _ => host::LocalResult::Unit(Err("expected a component call".to_owned())),
    }
}

/// Runs one nested UI callback: an expansion, a factory, a tool rendering, terminal input, an
/// authored style or highlighting, a widget callback or an authored editor request.
fn ui(call: host::LocalCall) -> host::LocalResult {
    let exports = Exports::new(Generated);
    match call {
        host::LocalCall::Expanded(call) => host::LocalResult::Unit(exports.invoke_set_expanded(
            call.target,
            call.expanded,
            &call.emitter,
        )),
        host::LocalCall::Factory(call) => host::LocalResult::Factory(factory(call)),
        host::LocalCall::CustomFactory(call) => {
            let lent = super::super::custom::Lent {
                tui: call.input.tui,
                theme: call.input.theme,
                keybindings: call.input.keybindings,
                close: call.input.close,
            };
            host::LocalResult::CustomFactory(
                exports
                    .invoke_custom_factory(call.target, lent, &call.emitter)
                    .map(|outcome| super::super::custom::transfer(&Generated, outcome)),
            )
        }
        host::LocalCall::ToolRender(call) => host::LocalResult::ToolRender(tool_render(call)),
        host::LocalCall::TerminalInput(call) => host::LocalResult::TerminalInput(
            exports.invoke_terminal_input(call.target, &call.data, &call.emitter),
        ),
        host::LocalCall::EditorFactory(call) => {
            host::LocalResult::EditorFactory(super::editors::editor_factory(
                call.target,
                call.tui,
                call.theme,
                call.keybindings,
                &call.emitter,
            ))
        }
        host::LocalCall::Editor(call) => host::LocalResult::Editor(super::editors::authored(
            call.target,
            call.request,
            &call.emitter,
        )),
        host::LocalCall::AutocompleteFactory(call) => host::LocalResult::AutocompleteFactory(
            super::editors::provider_factory(call.target, call.current, &call.emitter),
        ),
        host::LocalCall::Style(call) => {
            host::LocalResult::Style(super::super::invoke_style(call.target, &call.text))
        }
        host::LocalCall::Highlight(call) => host::LocalResult::Highlight(
            super::super::invoke_highlight(call.target, &call.code, call.lang.as_deref()),
        ),
        host::LocalCall::Widget(call) => host::LocalResult::Widget(
            super::widget_events::invoke_widget(call.target, call.event, &call.emitter),
        ),
        _ => host::LocalResult::Unit(Err("expected a UI callback".to_owned())),
    }
}

/// Invoke the retained credential fallback through its original author adapter.
fn auth_fallback(call: &host::LocalAuthFallback) -> host::LocalResult {
    host::LocalResult::AuthFallback(super::super::auth_storage::invoke_fallback(
        call.target,
        &call.provider,
    ))
}

/// Runs one nested synchronous callback prefix.
fn synchronous(call: &host::LocalSynchronous) -> ExtensionResult<()> {
    let exports = Exports::new(Generated);
    match call.kind {
        host::LocalKind::Listener => {
            exports.invoke_listener(call.target, &call.data, &call.emitter)
        }
        host::LocalKind::CompactionComplete => {
            exports.invoke_compaction_complete(call.target, &call.data, &call.emitter)
        }
        host::LocalKind::CompactionError => {
            exports.invoke_compaction_error(call.target, call.data.clone(), &call.emitter)
        }
        host::LocalKind::Watch => exports.invoke_watch(call.target, &call.emitter),
        host::LocalKind::Abort => exports.invoke_abort(call.target, &call.emitter),
        host::LocalKind::Dispose => exports.invoke_dispose(call.target, &call.emitter),
        host::LocalKind::BranchChange => {
            crate::component_adapter::factories::invoke_branch_change(call.target)
        }
    }
}

/// Runs one nested tool rendering and transfers what it returned.
fn tool_render(call: host::LocalToolRender) -> ExtensionResult<host::ToolRendered> {
    let lent = (
        call.theme,
        call.invalidator,
        call.native_row.map(|row| crate::NativeRow {
            handle: std::rc::Rc::new(row),
        }),
    );
    Exports::new(Generated)
        .invoke_tool_render(call.target, &call.render, lent, &call.emitter)
        .map(super::rendered_row)
}

/// Runs one nested component factory and transfers what it built.
fn factory(call: host::LocalFactory) -> ExtensionResult<host::FactoryComponent> {
    let lent = crate::component_adapter::factories::Lent {
        tui: call.tui,
        theme: call.theme,
        footer: call.footer,
    };
    Exports::new(Generated)
        .invoke_component_factory(call.target, lent, &call.emitter)
        .map(|built| crate::component_adapter::factories::transfer(&Generated, built))
}

/// Starts one nested asynchronous callback with its original capabilities.
fn asynchronous(call: host::LocalCall) -> Running {
    let exports = Exports::new(Generated);
    Box::pin(async move {
        match call {
            host::LocalCall::Provider(call) => host::LocalResult::Provider(
                super::provider_streams::invoke(call.target, &call.data, call.input),
            ),
            host::LocalCall::Assistant(call) => host::LocalResult::Assistant(
                super::provider_streams::assistant(
                    call.target,
                    call.operation,
                    call.data,
                    call.signal,
                )
                .await,
            ),
            host::LocalCall::Oauth(call) => {
                host::LocalResult::Oauth(super::oauth::invoke(call).await)
            }
            host::LocalCall::Event(call) => {
                host::LocalResult::Event(event(call.target, call.event, call.resources).await)
            }
            host::LocalCall::Command(call) => host::LocalResult::Unit(
                exports
                    .invoke_command(call.target, call.args, call.ctx)
                    .await,
            ),
            host::LocalCall::Completion(call) => host::LocalResult::Completion(
                exports.invoke_completion(call.target, call.prefix).await,
            ),
            host::LocalCall::Shortcut(call) => {
                host::LocalResult::Unit(exports.invoke_shortcut(call.target, call.ctx).await)
            }
            host::LocalCall::Tool(call) => host::LocalResult::Tool(tool(call).await),
            host::LocalCall::Setup(call) => {
                host::LocalResult::Unit(exports.invoke_setup(call.target, call.writer).await)
            }
            host::LocalCall::WithSession(call) => {
                host::LocalResult::Unit(exports.invoke_with_session(call.target, call.ctx).await)
            }
            call @ (host::LocalCall::CustomFuture(_) | host::LocalCall::Autocomplete(_)) => {
                ui_asynchronous(call).await
            }
            host::LocalCall::Tail(target) => {
                host::LocalResult::Unit(Exports::<Generated>::invoke_tail(target).await)
            }
            call @ (host::LocalCall::Read(_)
            | host::LocalCall::Write(_)
            | host::LocalCall::Edit(_)
            | host::LocalCall::Grep(_)
            | host::LocalCall::Find(_)
            | host::LocalCall::Ls(_)) => file_operation(call).await,
            host::LocalCall::Bash(call) => {
                host::LocalResult::Bash(bash(call.target, call.request, call.resources).await)
            }
            _ => host::LocalResult::Unit(Err("extension nested call has no export".into())),
        }
    })
}

/// Runs one nested asynchronous UI callback: a custom factory's owned continuation or a
/// completion provider request.
async fn ui_asynchronous(call: host::LocalCall) -> host::LocalResult {
    match call {
        host::LocalCall::CustomFuture(target) => host::LocalResult::Factory(
            super::super::custom::invoke_custom_future(target)
                .await
                .map(|built| super::super::factories::transfer(&Generated, built)),
        ),
        host::LocalCall::Autocomplete(call) => host::LocalResult::Autocomplete(
            super::editors::provider(call.target, call.request, call.signal).await,
        ),
        _ => host::LocalResult::Unit(Err("expected an asynchronous UI callback".into())),
    }
}

/// Executes the authored tool with its supplied capabilities.
async fn tool(call: host::LocalTool) -> ExtensionResult<String> {
    let host::ToolCapabilities {
        ctx,
        signal,
        update,
    } = call.resources;
    Exports::new(Generated)
        .invoke_tool(
            call.target,
            call.invocation,
            ToolCapabilities {
                ctx,
                signal,
                update,
            },
        )
        .await
}

/// Delivers an event and transfers any returned operations under a fresh identity.
pub(super) async fn event(
    target: u32,
    event: String,
    resources: host::Capabilities,
) -> Result<host::EventOutcome, String> {
    let host::Capabilities { ctx, signal } = resources;
    let outcome = Exports::new(Generated)
        .invoke_event(target, event, Capabilities { ctx, signal })
        .await?;
    Ok(host::EventOutcome {
        event: outcome.event,
        decision: outcome.decision,
        operations: renderers::retain(&Generated, outcome.operations),
    })
}

/// Executes the returned operations kept under one identity.
pub(super) async fn bash(
    target: u32,
    request: host::BashRequest,
    resources: host::BashCapabilities,
) -> ExtensionResult<Option<f64>> {
    let operations = callbacks::find::<crate::BashOperations>(target)?;
    let resources = BashCapabilities {
        output: resources.output,
        signal: resources.signal,
    };
    Ok(Exports::new(Generated)
        .invoke_bash(&operations, request, resources)
        .await?
        .exit_code)
}

/// Walks the retained native snapshot without changing mixed callback order.
pub(super) fn walk_snapshot(
    emitter: &host::CallbackEmitter,
    snapshot: u32,
    tags: &[Option<u32>],
    data: &str,
) -> ExtensionResult<()> {
    let mut index = 0;
    while index < tags.len() {
        if let Some(id) = tags[index] {
            let local = host::callback_begin_local(
                emitter,
                snapshot,
                u32::try_from(index).map_err(|error| error.to_string())?,
            )?;
            let result = Exports::new(Generated).invoke_listener(id, data, &local);
            drive(&host::callback_end_local(&local, result.err().as_deref()))?;
            index += 1;
        } else {
            let start = index;
            while index < tags.len() && tags[index].is_none() {
                index += 1;
            }
            drive(&host::callback_segment(
                emitter,
                snapshot,
                u32::try_from(start).map_err(|error| error.to_string())?,
                u32::try_from(index).map_err(|error| error.to_string())?,
            ))?;
        }
    }
    Ok(())
}

/// Runs either authored backend method with its still-borrowed native update.
fn backend(call: host::LocalAuthBackend) -> Running {
    Box::pin(async move {
        let result = if call.asynchronous {
            super::super::auth_storage::invoke_backend_async(&Generated, call.target, &call.update)
                .await
        } else {
            super::super::auth_storage::invoke_backend(&Generated, call.target, &call.update)
        };
        host::LocalResult::Unit(result)
    })
}

/// Preserves the original non-unit predicate result.
pub(super) fn boolean(route: &host::ImportRoute) -> ExtensionResult<bool> {
    match value(route)? {
        host::RouteValue::Boolean(value) => Ok(value),
        _ => Err("extension route result kind mismatch".into()),
    }
}
/// Preserves an encoded record returned by the original native owner.
pub(super) fn text(route: &host::ImportRoute) -> ExtensionResult<String> {
    match value(route)? {
        host::RouteValue::Text(value) => Ok(value),
        _ => Err("extension route result kind mismatch".into()),
    }
}

/// Preserves an encoded record that the original native owner may not have.
pub(super) fn optional_text(route: &host::ImportRoute) -> ExtensionResult<Option<String>> {
    match value(route)? {
        host::RouteValue::OptionalText(value) => Ok(value),
        _ => Err("extension route result kind mismatch".into()),
    }
}

/// Preserves the credential owner a native constructor retained.
pub(super) fn credentials(route: &host::ImportRoute) -> ExtensionResult<host::AuthStorage> {
    match value(route)? {
        host::RouteValue::Credentials(owner) => Ok(owner),
        _ => Err("extension route result kind mismatch".into()),
    }
}

/// Preserves the catalog a native constructor retained.
pub(super) fn registry(route: &host::ImportRoute) -> ExtensionResult<host::ModelRegistry> {
    match value(route)? {
        host::RouteValue::Catalog(registry) => Ok(registry),
        _ => Err("extension route result kind mismatch".into()),
    }
}

/// Forwards nested file observations to their existing typed author adapters.
async fn file_operation(call: host::LocalCall) -> host::LocalResult {
    match call {
        host::LocalCall::Grep(call) => host::LocalResult::Grep(
            super::super::tools::grep::invoke(call.target, call.operation, call.path).await,
        ),
        host::LocalCall::Ls(call) => host::LocalResult::Ls(
            super::super::tools::ls::invoke(call.target, call.operation, call.path).await,
        ),
        host::LocalCall::Find(call) => host::LocalResult::Find(
            super::super::tools::find::invoke(call.target, call.operation, call.path).await,
        ),
        host::LocalCall::Edit(call) => host::LocalResult::Edit(
            super::super::tools::edit::invoke(call.target, call.operation, call.path).await,
        ),
        host::LocalCall::Write(call) => host::LocalResult::Write(
            super::super::tools::write::invoke(call.target, call.operation, call.path).await,
        ),
        host::LocalCall::Read(call) => host::LocalResult::Read(
            super::super::tools::read::invoke(call.target, call.operation, call.path).await,
        ),
        _ => host::LocalResult::Unit(Err("extension nested call has no export".into())),
    }
}
