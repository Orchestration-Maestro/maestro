//! Nested tool execution keeps its original per-call capabilities.

use super::{Callback, Context, LocalCall, LocalTool, ToolCall, ToolInvocation, ToolResources};
use wasmtime::component::ResourceTable;

/// Marshals one tool execution with its per-call capabilities.
pub(super) fn lower(
    table: &mut ResourceTable,
    call: &ToolCall<Callback>,
) -> wasmtime::Result<LocalCall> {
    let ToolCall::Execute {
        handler,
        id,
        params,
        context,
        signal,
        update,
        ..
    } = call
    else {
        return Err(wasmtime::format_err!(
            "extension preparation is synchronous"
        ));
    };
    Ok(LocalCall::Tool(LocalTool {
        target: handler.id,
        invocation: ToolInvocation {
            tool_call_id: id.clone(),
            params: params.clone(),
        },
        resources: ToolResources {
            ctx: table.push(Context(context.clone()))?,
            signal: signal
                .clone()
                .map(|signal| table.push(signal))
                .transpose()?,
            update: update
                .clone()
                .map(|update| table.push(update))
                .transpose()?,
        },
    }))
}
