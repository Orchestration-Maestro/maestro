//! Drive tool progress futures alongside execution without spawning tasks.
use super::Driver;
use crate::{
    AgentEvent, AgentToolResult, AgentToolUpdateCallback, CustomAgentMessages, ExecuteTool,
};
use futures_util::{StreamExt, future::join};
use maestro_models::{BoxFuture, DiagnosticErrorInfo, EventStream, ToolCall};
use std::sync::{Arc, Mutex};

/// Reservations spanning synchronous observation and future admission.
#[derive(Default)]
struct Admission {
    /// Whether execution has settled and no new observation may enter.
    closed: bool,
    /// Observers that reserved entry but have not yet queued their future.
    pending: usize,
}

impl<C: CustomAgentMessages + 'static> Driver<'_, C> {
    /// Admit execution immediately; close and settle progress when its future completes.
    pub(super) fn execute(
        &self,
        call: &ToolCall,
        execute: &ExecuteTool,
        arguments: serde_json::Value,
    ) -> BoxFuture<Result<Result<AgentToolResult, DiagnosticErrorInfo>, DiagnosticErrorInfo>> {
        let updates = EventStream::new(
            |_: &BoxFuture<Result<(), DiagnosticErrorInfo>>| false,
            |_| (),
        );
        let admission = Arc::new(Mutex::new(Admission::default()));
        let callback =
            self.progress_callback(call.clone(), updates.clone(), Arc::clone(&admission));
        let execution = execute(
            call.id.clone(),
            arguments,
            self.options.signal.clone(),
            Some(callback),
        );
        let pending = futures_util::stream::unfold(updates.clone(), |updates| async move {
            updates.next().await.map(|future| (future, updates))
        });
        #[cfg(not(target_arch = "wasm32"))]
        let pending = pending.boxed();
        #[cfg(target_arch = "wasm32")]
        let pending = pending.boxed_local();
        let progress = pending
            .map(futures_util::stream::once)
            .flatten_unordered(None);
        #[cfg(not(target_arch = "wasm32"))]
        let progress = progress.boxed();
        #[cfg(target_arch = "wasm32")]
        let progress = progress.boxed_local();
        let progress = progress.collect::<Vec<_>>();
        Box::pin(async move {
            let (result, progress) = join(
                async {
                    let result = execution.await;
                    close_progress(&admission, &updates);
                    result
                },
                progress,
            )
            .await;
            for settled in progress {
                settled?;
            }
            Ok(result)
        })
    }
    /// Reserve observation entry before invoking foreign code without a lock.
    fn progress_callback(
        &self,
        update_call: ToolCall,
        admitted: EventStream<BoxFuture<Result<(), DiagnosticErrorInfo>>, ()>,
        reservation: Arc<Mutex<Admission>>,
    ) -> AgentToolUpdateCallback {
        let emit = Arc::clone(&self.emit);
        Arc::new(move |partial_result| {
            let mut state = reservation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.closed {
                return;
            }
            state.pending += 1;
            drop(state);
            admitted.push(emit(AgentEvent::ToolExecutionUpdate {
                tool_call: update_call.clone(),
                partial_result,
            }));
            let mut state = reservation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.pending -= 1;
            let settled = state.closed && state.pending == 0;
            drop(state);
            if settled {
                admitted.end(Some(()));
            }
        })
    }
}

/// Revoke new delivery while allowing admitted observers to finish enqueueing.
fn close_progress(
    admission: &Mutex<Admission>,
    updates: &EventStream<BoxFuture<Result<(), DiagnosticErrorInfo>>, ()>,
) {
    let mut state = admission
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.closed = true;
    let settled = state.pending == 0;
    drop(state);
    if settled {
        updates.end(Some(()));
    }
}
