//! Ordered files and single terminal ownership shared by native and browser execution.
use maestro_app::browser::components::sandboxed_iframe::{SandboxError, SandboxValue};
use maestro_app::{SandboxFile, SandboxResult};

/// Selected lifecycle fields, without converting native payloads.
pub(super) enum ExecutionMessage<V: SandboxValue, C> {
    /// Original download fields.
    File(SandboxFile<C>),
    /// Optional native return.
    Complete(Option<V>),
    /// Supplied error fields.
    Error(SandboxError),
    /// Unrelated traffic.
    Other,
}
/// Captured file fields and terminal admission, independent of native effects.
pub(super) struct ExecutionState<V: SandboxValue, C> {
    /// Original fields in message order.
    files: Vec<SandboxFile<C>>,
    /// Retained value type selected by the caller.
    value: std::marker::PhantomData<V>,
    /// Whether a terminal message has already been admitted.
    settled: bool,
}
impl<V: SandboxValue, C> Default for ExecutionState<V, C> {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            settled: false,
            value: std::marker::PhantomData,
        }
    }
}
impl<V: SandboxValue, C> ExecutionState<V, C> {
    /// Collect a selected message; only the first terminal message produces a result.
    pub(super) fn receive(
        &mut self,
        message: ExecutionMessage<V, C>,
    ) -> Option<SandboxResult<V, C>> {
        match message {
            ExecutionMessage::File(file) => {
                self.file(file);
                None
            }
            ExecutionMessage::Complete(value) => self.complete(value),
            ExecutionMessage::Error(error) => self.error(error),
            ExecutionMessage::Other => None,
        }
    }
    /// Retain fields without copying their payload.
    pub(super) fn file(&mut self, file: SandboxFile<C>) {
        if !self.settled {
            self.files.push(file);
        }
    }
    /// Admit successful completion once, keeping absence distinct from null.
    pub(super) fn complete(&mut self, value: Option<V>) -> Option<SandboxResult<V, C>> {
        self.finish(true, value, None)
    }
    /// Admit a runtime error once, retaining previously captured files.
    pub(super) fn error(&mut self, error: SandboxError) -> Option<SandboxResult<V, C>> {
        self.finish(false, None, Some(error))
    }
    /// Move the collected fields into the first result.
    fn finish(
        &mut self,
        success: bool,
        return_value: Option<V>,
        error: Option<SandboxError>,
    ) -> Option<SandboxResult<V, C>> {
        if self.settled {
            return None;
        }
        self.settled = true;
        Some(SandboxResult {
            success,
            console: Vec::new(),
            files: Some(std::mem::take(&mut self.files)),
            error,
            return_value,
        })
    }
}
