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
use crate::component_adapter::{Exports, bash::BashCapabilities, callbacks, renderers};

/// One nested call's result, ready at once or after its later continuation.
type Running = Pin<Box<dyn Future<Output = host::LocalResult>>>;

/// Completes one host operation, running each nested call it makes into this instance.
///
/// # Errors
/// Returns the host operation's own error.
pub(super) fn drive(route: &host::ImportRoute) -> ExtensionResult<()> {
    loop {
        match route.next() {
            host::RouteStep::Done(result) => return result,
            host::RouteStep::Invoke(call) => start(route, dispatch(call)),
        }
    }
}

/// Completes a host rendering, running each nested call it makes into this instance.
///
/// # Errors
/// Returns the rendering's own error.
pub(super) fn drive_render(route: &host::RenderRoute) -> ExtensionResult<Vec<String>> {
    loop {
        match route.next() {
            host::RenderStep::Done(result) => return result,
            host::RenderStep::Invoke(call) => start(route, dispatch(call)),
        }
    }
}

/// A host operation in progress that hands this instance its nested calls.
trait Progress {
    /// Returns a nested call's result.
    fn resume(&self, value: host::LocalResult);
    /// Releases the host caller now; the nested call's result follows through the settlement.
    fn defer(&self) -> host::LocalSettlement;
}
impl Progress for host::ImportRoute {
    fn resume(&self, value: host::LocalResult) {
        host::ImportRoute::resume(self, value);
    }
    fn defer(&self) -> host::LocalSettlement {
        host::ImportRoute::defer(self)
    }
}
impl Progress for host::RenderRoute {
    fn resume(&self, value: host::LocalResult) {
        host::RenderRoute::resume(self, value);
    }
    fn defer(&self) -> host::LocalSettlement {
        host::RenderRoute::defer(self)
    }
}

/// Runs a nested call's prefix now; a call still pending continues in this guest's task and
/// delivers its result through its settlement, so the host caller is released at once.
fn start(route: &impl Progress, mut running: Running) {
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
        host::LocalCall::Key(call) => host::LocalResult::Key(exports.invoke_component(
            call.target,
            &call.emitter,
            crate::Component::wants_key_release,
        )),
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
        host::LocalCall::Factory(call) => host::LocalResult::Factory(factory(call)),
        host::LocalCall::ToolRender(call) => host::LocalResult::ToolRender(tool_render(call)),
        host::LocalCall::TerminalInput(call) => host::LocalResult::TerminalInput(
            exports.invoke_terminal_input(call.target, &call.data, &call.emitter),
        ),
        host::LocalCall::Synchronous(call) => host::LocalResult::Synchronous(synchronous(&call)),
        host::LocalCall::Style(call) => {
            host::LocalResult::Style(super::super::invoke_style(call.target, &call.text))
        }
        host::LocalCall::Highlight(call) => host::LocalResult::Highlight(
            super::super::invoke_highlight(call.target, &call.code, call.lang.as_deref()),
        ),
        call => return asynchronous(call),
    };
    Box::pin(std::future::ready(result))
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
        host::LocalKind::Abort => exports.invoke_abort(call.target, &call.emitter),
        host::LocalKind::Dispose => exports.invoke_dispose(call.target, &call.emitter),
        host::LocalKind::BranchChange => {
            crate::component_adapter::factories::invoke_branch_change(call.target)
        }
    }
}

/// Runs one nested tool rendering and transfers what it returned.
fn tool_render(call: host::LocalToolRender) -> ExtensionResult<host::ToolRendered> {
    let lent = (call.theme, call.invalidator);
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
            host::LocalCall::Tail(target) => {
                host::LocalResult::Unit(Exports::<Generated>::invoke_tail(target).await)
            }
            host::LocalCall::Bash(call) => {
                host::LocalResult::Bash(bash(call.target, call.request, call.resources).await)
            }
            _ => host::LocalResult::Unit(Err("extension nested call has no export".into())),
        }
    })
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
        .invoke_event(target, event, super::Capabilities { ctx, signal })
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
