//! Original file-effect operands on the existing nested invocation route.
use wasmtime::component::{ComponentType, Lower};

/// Original operation operands of a nested file observation.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in crate::import_route) struct LocalRead {
    /// Original guest operations identity.
    pub(super) target: u32,
    /// The requested observation.
    pub(super) operation: crate::guest_calls::read::Operation,
    /// Exact absolute authored path.
    pub(super) path: String,
}

/// Original operation operands of a nested file effect.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in crate::import_route) struct LocalWrite {
    /// Original guest effect identity.
    pub(super) target: u32,
    /// Requested effect and its original content.
    pub(super) operation: crate::guest_calls::write::Operation,
    /// Exact absolute target.
    pub(super) path: String,
}

/// Original operation operands of a nested edit effect.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in crate::import_route) struct LocalEdit {
    /// Original guest effect identity.
    pub(super) target: u32,
    /// Requested edit effect and its original content.
    pub(super) operation: crate::guest_calls::edit::Operation,
    /// Exact absolute target.
    pub(super) path: String,
}

/// Original operation operands of a nested file search.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in crate::import_route) struct LocalFind {
    /// Original guest effect identity.
    pub(super) target: u32,
    /// Requested find observation and glob constraints.
    pub(super) operation: crate::guest_calls::find::Operation,
    /// Exact absolute target.
    pub(super) path: String,
}

/// Original operation operands of a nested directory listing.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in crate::import_route) struct LocalLs {
    /// Original guest effect identity.
    pub(super) target: u32,
    /// Requested directory listing observation.
    pub(super) operation: crate::guest_calls::ls::Operation,
    /// Exact absolute target.
    pub(super) path: String,
}

/// Original operation operands of a nested content search.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in crate::import_route) struct LocalGrep {
    /// Original guest effect identity.
    pub(super) target: u32,
    /// Requested content search observation.
    pub(super) operation: crate::guest_calls::grep::Operation,
    /// Exact absolute target.
    pub(super) path: String,
}

/// Marshals original typed file observations without entering authored code.
pub(super) fn lower(
    call: &crate::guest_calls::Call<crate::guest_calls::Callback>,
) -> wasmtime::Result<super::LocalCall> {
    use super::LocalCall;
    use crate::guest_calls::Call;
    Ok(match call {
        Call::Ls(call) => LocalCall::Ls(LocalLs {
            target: call.handler.id,
            operation: call.operation,
            path: call.path.clone(),
        }),
        Call::Grep(call) => LocalCall::Grep(LocalGrep {
            target: call.handler.id,
            operation: call.operation,
            path: call.path.clone(),
        }),
        Call::Find(call) => LocalCall::Find(LocalFind {
            target: call.handler.id,
            operation: call.operation.clone(),
            path: call.path.clone(),
        }),
        Call::Edit(call) => LocalCall::Edit(LocalEdit {
            target: call.handler.id,
            operation: call.operation.clone(),
            path: call.path.clone(),
        }),
        Call::Write(call) => LocalCall::Write(LocalWrite {
            target: call.handler.id,
            operation: call.operation.clone(),
            path: call.path.clone(),
        }),
        Call::Read(call) => LocalCall::Read(LocalRead {
            target: call.handler.id,
            operation: call.operation,
            path: call.path.clone(),
        }),
        _ => return Err(wasmtime::format_err!("extension call cannot run nested")),
    })
}

/// Returns each file observation to its original typed reply without changing its diagnostic.
pub(super) fn settle(
    call: crate::guest_calls::Call<crate::guest_calls::Callback>,
    value: super::LocalResult,
) -> wasmtime::Result<()> {
    use super::LocalResult;
    use crate::guest_calls::Call;
    match (call, value) {
        (Call::Grep(call), LocalResult::Grep(value)) => {
            let _ = call.reply.send(Ok(value.map_err(Into::into)));
        }
        (Call::Ls(call), LocalResult::Ls(value)) => {
            let _ = call.reply.send(Ok(value.map_err(Into::into)));
        }
        (Call::Find(call), LocalResult::Find(value)) => {
            let _ = call.reply.send(Ok(value.map_err(Into::into)));
        }
        (Call::Edit(call), LocalResult::Edit(value)) => {
            let _ = call.reply.send(Ok(value.map_err(Into::into)));
        }
        (Call::Write(call), LocalResult::Write(value)) => {
            let _ = call.reply.send(Ok(value.map_err(Into::into)));
        }
        (Call::Read(call), LocalResult::Read(value)) => {
            let _ = call.reply.send(Ok(value.map_err(Into::into)));
        }
        _ => {
            return Err(wasmtime::format_err!(
                "extension route result kind mismatch"
            ));
        }
    }
    Ok(())
}
