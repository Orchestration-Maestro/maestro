//! A host operation that calls back into this instance hands each nested call to the guest,
//! which runs it with the same bodies the ordinary exports run.

use super::{Generated, host};
use crate::ExtensionResult;
use crate::component_adapter::{Exports, renderers};

/// Completes one host operation, running each nested call it makes into this instance.
///
/// # Errors
/// Returns the host operation's own error.
pub(super) fn drive(route: &host::ImportRoute) -> ExtensionResult<()> {
    loop {
        match route.next() {
            host::RouteStep::Done(result) => return result,
            host::RouteStep::Invoke(call) => route.resume(dispatch(call)),
        }
    }
}

/// Runs one nested call against the authored objects the ordinary exports use.
fn dispatch(call: host::LocalCall) -> host::LocalResult {
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
        host::LocalCall::Synchronous(call) => host::LocalResult::Synchronous(match call.kind {
            host::LocalKind::Listener => {
                exports.invoke_listener(call.target, &call.data, &call.emitter)
            }
            host::LocalKind::CompactionComplete => {
                exports.invoke_compaction_complete(call.target, &call.data, &call.emitter)
            }
            host::LocalKind::CompactionError => {
                exports.invoke_compaction_error(call.target, call.data, &call.emitter)
            }
        }),
    }
}
