//! Drive tool progress futures alongside execution without spawning tasks.
use super::Driver;
use crate::{
    AgentEvent, AgentToolResult, AgentToolUpdateCallback, CustomAgentMessages, ExecuteTool,
};
use futures_util::{StreamExt, future::join};
use maestro_models::{DiagnosticErrorInfo, EventStream, ToolCall};
use std::sync::{Arc, Mutex};

/// Reservations spanning synchronous observation and future admission.
#[derive(Default)]
struct Admission {
    /// Whether execution has settled and no new observation may enter.
    closed: bool,
    /// Observers that reserved entry but have not yet queued their future.
    pending: usize,
}

impl<C: CustomAgentMessages + 'static, E: crate::ExecutionFor<C>> Driver<'_, C, E> {
    /// Admit execution immediately; close and settle progress when its future completes.
    pub(super) fn execute(
        &self,
        call: &ToolCall,
        execute: &ExecuteTool<serde_json::Value, serde_json::Value, E>,
        arguments: serde_json::Value,
    ) -> impl std::future::Future<
        Output = Result<Result<AgentToolResult, DiagnosticErrorInfo>, DiagnosticErrorInfo>,
    > + use<C, E> {
        let updates = EventStream::new_local_items(|_: &E::ObservationFuture| false, |_| ());
        let admission = Arc::new(Mutex::new(Admission::default()));
        let callback =
            self.progress_callback(call.clone(), updates.clone(), Arc::clone(&admission));
        let execution = execute(
            call.id.clone(),
            arguments,
            self.options.signal.clone(),
            Some(callback),
        );
        let progress = E::pending_progress(updates.clone()).collect::<Vec<_>>();
        async move {
            let (result, progress) = join(
                async {
                    let result = execution.await;
                    close_progress::<E>(&admission, &updates);
                    result
                },
                progress,
            )
            .await;
            for settled in progress {
                settled?;
            }
            Ok(result)
        }
    }
    /// Create the mode's erased progress callback.
    fn progress_callback(
        &self,
        update_call: ToolCall,
        admitted: EventStream<E::ObservationFuture, ()>,
        reservation: Arc<Mutex<Admission>>,
    ) -> AgentToolUpdateCallback<serde_json::Value, E> {
        E::progress(Progress {
            emit: self.emit.clone(),
            update_call,
            admitted,
            reservation,
        })
    }
}
/// Shared progress admission state.
pub struct Progress<C: CustomAgentMessages, E: crate::AgentExecution> {
    /// Callback admitting observations.
    emit: crate::AgentEventSink<C, E>,
    /// Identity of the tool producing this update.
    update_call: ToolCall,
    /// Existing FIFO for pending observations.
    admitted: EventStream<E::ObservationFuture, ()>,
    /// Admission state shared with settlement.
    reservation: Arc<Mutex<Admission>>,
}
impl<C: CustomAgentMessages, E: crate::AgentExecution> Progress<C, E> {
    /// Admit the synchronous callback prefix before queueing its future.
    pub fn update(&self, partial_result: AgentToolResult) {
        let mut state = self
            .reservation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            return;
        }
        state.pending += 1;
        drop(state);
        self.admitted
            .push((self.emit)(AgentEvent::ToolExecutionUpdate {
                tool_call: self.update_call.clone(),
                partial_result,
            }));
        let mut state = self
            .reservation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.pending -= 1;
        let settled = state.closed && state.pending == 0;
        drop(state);
        if settled {
            self.admitted.end(Some(()));
        }
    }
}

/// Revoke new delivery while allowing admitted observers to finish enqueueing.
fn close_progress<E: crate::AgentExecution>(
    admission: &Mutex<Admission>,
    updates: &EventStream<E::ObservationFuture, ()>,
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
