//! Native waits use the original cancellation and the sole local request pump.
use crate::{
    api_imports::ApiView,
    local_resources::{Admission, Message, SignalEndpoint, Work, pump::State},
};
use futures_util::future::{Either, select};
use std::time::Duration;
use tokio::sync::oneshot;
use wasmtime::component::{LinkerInstance, Resource};

/// Selected native duration beside its independently retained cancellation resource.
pub(crate) struct Wait {
    /// Seconds and fractional nanoseconds; no floating-point duration conversion.
    duration: (u64, u32),
    /// The cancellation admitted with this particular wait.
    signal: Option<SignalEndpoint>,
}

/// Installs the typed wait without adding a guest-side timer.
pub(crate) fn add_to_linker<T: ApiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "sleep",
        |accessor, (duration, signal): ((u64, u32), Option<Resource<SignalEndpoint>>)| {
            Box::pin(async move {
                let (api, signal) = accessor.with(|mut access| {
                    let state = access.get();
                    let signal = signal
                        .map(|s| state.ctx().table.get(&s).cloned())
                        .transpose()?;
                    Ok::<_, wasmtime::Error>((state.api().clone(), signal))
                })?;
                let result = api
                    .0
                    .request_async(|_, reply, admission| {
                        Message::Sleep(Wait { duration, signal }, reply, admission)
                    })
                    .await;
                Ok((result,))
            })
        },
    )
}

impl State {
    /// Admits the selected original signal without blocking later resource requests.
    pub(super) fn sleep(
        &self,
        request: Wait,
        reply: oneshot::Sender<Result<(), String>>,
        admission: Admission,
    ) {
        let signal = request
            .signal
            .map(|s| self.registry.cancellation(&s))
            .transpose();
        match signal {
            Ok(signal) if request.duration.1 < 1_000_000_000 => self.admit(
                sleeping(
                    Duration::new(request.duration.0, request.duration.1),
                    signal,
                    reply,
                ),
                admission,
            ),
            Ok(_) => {
                let _ = reply.send(Err("Invalid fractional nanoseconds".into()));
            }
            Err(error) => {
                let _ = reply.send(Err(error));
            }
        }
    }
}

/// Dropping the waiting guest releases only this operation, never its caller signal.
fn sleeping(
    duration: Duration,
    signal: Option<maestro_extensions::Cancellation>,
    mut reply: oneshot::Sender<Result<(), String>>,
) -> Work {
    Box::pin(async move {
        let result = {
            let wait = std::pin::pin!(maestro_extensions::native_io::sleep(
                duration,
                signal.as_ref()
            ));
            let closed = std::pin::pin!(reply.closed());
            match select(wait, closed).await {
                Either::Left((result, _)) => Some(result.map_err(|e| e.to_string())),
                Either::Right(_) => None,
            }
        };
        if let Some(result) = result {
            let _ = reply.send(result);
        }
    })
}
