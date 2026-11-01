//! Canonical file callback borrowing shared by native read, write and edit effects.
use crate::{
    guest_calls::{Call, Callback},
    lifecycle::replies::Reply,
};
use maestro_extensions::DiagnosticErrorInfo;
use wasmtime::component::{Accessor, ComponentType, Lift, Lower, Resource, TypedFunc};
use wasmtime_wasi::WasiView;

/// Canonical operation signature retaining its typed output and structured failure.
pub(super) type Export<O, R> =
    TypedFunc<(Resource<Callback>, O, String), (Result<R, crate::schema_wire::Error>,)>;

/// Keeps the original callback resource borrowed until its actual asynchronous settlement.
pub(super) async fn invoke<T, O, R>(
    export: &Export<O, R>,
    accessor: &Accessor<T>,
    handler: Callback,
    operands: (O, String),
    reply: Reply<Result<R, DiagnosticErrorInfo>>,
) -> wasmtime::Result<()>
where
    T: WasiView + 'static,
    O: ComponentType + Lower + Send + Sync + 'static,
    R: ComponentType + Lift + Send + Sync + 'static,
{
    let handler = accessor.with(|mut access| access.get().ctx().table.push(handler))?;
    let result = export
        .call_concurrent(
            accessor,
            (Resource::new_borrow(handler.rep()), operands.0, operands.1),
        )
        .await;
    accessor.with(|mut access| access.get().ctx().table.delete(handler))?;
    let _ = reply.send(Ok(result?.0.map_err(Into::into)));
    Ok(())
}

impl super::Exports {
    /// Enters retained asynchronous operations through their original guest exports.
    pub(super) async fn io<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Call<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            Call::Bash(call) => self.bash.invoke(accessor, call).await,
            call @ (Call::Grep(_) | Call::Find(_) | Call::Ls(_)) => {
                self.search(accessor, call).await
            }
            Call::Edit(call) => {
                invoke(
                    &self.edit,
                    accessor,
                    call.handler,
                    (call.operation, call.path),
                    call.reply,
                )
                .await
            }
            Call::Read(call) => {
                invoke(
                    &self.read,
                    accessor,
                    call.handler,
                    (call.operation, call.path),
                    call.reply,
                )
                .await
            }
            Call::Write(call) => {
                invoke(
                    &self.write,
                    accessor,
                    call.handler,
                    (call.operation, call.path),
                    call.reply,
                )
                .await
            }
            _ => Err(wasmtime::format_err!("extension operation has no export")),
        }
    }
}
