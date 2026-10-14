//! Per-call raw output and progress execution.
use super::{BashExecOptions, BashOperations, BashSpawnContext, BashToolDetails};
use crate::{
    output::{
        output_accumulator::{OutputAccumulator, OutputAccumulatorOptions, OutputSnapshot},
        text_block,
    },
    utils::diagnostics::diagnostic,
};
use maestro_agent::{AgentToolResult, AgentToolUpdateCallback};
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::Notify, time::Instant};

/// Output state shared only with the borrowed data sink.
struct Progress {
    /// Decoder and raw artifact writer.
    output: OutputAccumulator,
    /// First accumulator failure, retained until the operation settles.
    error: Option<io::Error>,
    /// Pending data since the last delivered update.
    dirty: bool,
    /// Last emission, absent until the first data callback.
    last: Option<Instant>,
}
impl Progress {
    /// Construct without observing time or opening an artifact.
    fn new() -> Self {
        Self {
            output: OutputAccumulator::new(OutputAccumulatorOptions {
                temp_file_prefix: Some("maestro-bash".into()),
                ..OutputAccumulatorOptions::default()
            }),
            error: None,
            dirty: false,
            last: None,
        }
    }
    /// Append raw bytes and select any immediately due progress.
    fn append(&mut self, bytes: &[u8], observed: bool) -> Option<AgentToolResult<BashToolDetails>> {
        if self.error.is_some() {
            return None;
        }
        self.error = self.output.append(bytes).err();
        if !observed || self.error.is_some() {
            return None;
        }
        self.dirty = true;
        if self
            .last
            .is_none_or(|last| Instant::now() >= last + Duration::from_millis(100))
        {
            self.emit()
        } else {
            None
        }
    }
    /// The single pending emission deadline, never a periodic tick.
    fn deadline(&self) -> Option<Instant> {
        self.last
            .filter(|_| self.dirty && self.error.is_none())
            .map(|last| last + Duration::from_millis(100))
    }
    /// Select an update, without calling the external observer.
    fn emit(&mut self) -> Option<AgentToolResult<BashToolDetails>> {
        if !self.dirty || self.error.is_some() {
            return None;
        }
        self.dirty = false;
        self.last = Some(Instant::now());
        match self.output.snapshot(true) {
            Ok(snapshot) => Some(AgentToolResult {
                content: vec![text_block(snapshot.content)],
                details: Some(BashToolDetails {
                    truncation: snapshot.truncation.truncated.then_some(snapshot.truncation),
                    full_output_path: snapshot.full_output_path,
                }),
                terminate: None,
            }),
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }
    /// Finalize decoding and emit pending progress while error-free; always attempt artifact closure.
    fn finish(
        mut self,
        update: Option<&AgentToolUpdateCallback<BashToolDetails>>,
    ) -> io::Result<(OutputSnapshot, usize)> {
        if self.error.is_none() {
            self.error = self.output.finish().err();
        }
        if let Some(value) = self.emit()
            && let Some(update) = update
        {
            update(value);
        }
        if let Some(error) = self.error {
            let _ = self.output.close_temp_file();
            return Err(error);
        }
        let snapshot = self.output.snapshot(true);
        let closed = self.output.close_temp_file();
        let snapshot = snapshot?;
        closed?;
        Ok((snapshot, self.output.get_last_line_bytes()))
    }
}

/// Wait only when output has selected a pending progress deadline.
async fn wait(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// Run the supplied operation and close its output before classifying settlement.
pub(super) async fn run(
    operations: Arc<dyn BashOperations>,
    context: BashSpawnContext,
    timeout: Option<f64>,
    signal: Option<Cancellation>,
    update: Option<AgentToolUpdateCallback<BashToolDetails>>,
) -> Result<AgentToolResult<BashToolDetails>, DiagnosticErrorInfo> {
    let state = Mutex::new(Progress::new());
    let changed = Notify::new();
    let result = {
        let mut data = |bytes: &[u8]| {
            let value = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .append(bytes, update.is_some());
            changed.notify_one();
            if let (Some(update), Some(value)) = (&update, value) {
                update(value);
            }
        };
        let mut operation = operations.exec(
            &context.command,
            &context.cwd,
            BashExecOptions {
                on_data: &mut data,
                signal: signal.as_ref(),
                timeout,
                env: Some(&context.env),
            },
        );
        loop {
            let deadline = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .deadline();
            tokio::select! {
                biased;
                () = wait(deadline) => {
                    let value = state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).emit();
                    if let (Some(update), Some(value)) = (&update, value) { update(value); }
                }
                () = changed.notified() => {}
                result = &mut operation => break result,
            }
        }
    };
    let state = state
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (snapshot, last_line_bytes) = state
        .finish(update.as_ref())
        .map_err(|error| diagnostic(&error))?;
    super::result::settle(snapshot, last_line_bytes, result)
}
