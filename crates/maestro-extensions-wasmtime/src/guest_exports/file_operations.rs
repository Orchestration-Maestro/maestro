//! Canonical file callback borrowing shared by native read, write and edit effects.
use crate::{guest_calls::Callback, lifecycle::replies::Reply};
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
