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
        host::LocalCall::Schema(call) => host::LocalResult::Schema(
            exports
                .invoke_schema(call.target, call.operation, &call.node, &call.emitter)
                .map_err(crate::schema_error::to_wire),
        ),
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
