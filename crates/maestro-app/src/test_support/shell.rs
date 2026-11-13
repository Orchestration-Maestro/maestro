//! Shell operations that replay supplied output instead of running a process.
use maestro_models::{Execution, Local, Threaded};
use maestro_tools::{BashError, BashExecOptions, BashExecResult, BashOperations};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// A supplied shell's exceptional settlement after streaming its bytes.
#[derive(Clone, Copy, Debug)]
pub enum ShellFailure {
    /// Settle as cancelled.
    Aborted,
    /// Report a timeout with the supplied seconds.
    Timeout(f64),
}

/// Shell operations that record execution, stream supplied bytes, then optionally wait for release.
pub struct ScriptedShell {
    /// Each execution's command and directory, in order.
    pub executed: Mutex<Vec<Value>>,
    /// Notified once the chunks are written.
    pub started: Arc<Notify>,
    /// Lets a holding execution settle.
    pub release: Arc<Notify>,
    /// The output chunks, including incomplete UTF-8 sequences.
    chunks: Vec<Vec<u8>>,
    /// The exit status.
    exit_code: Option<f64>,
    /// Whether the execution waits for `release`.
    hold: bool,
    /// Exceptional settlement instead of the exit status.
    failure: Option<ShellFailure>,
}

impl ScriptedShell {
    /// Operations streaming `chunks` and settling with the supplied status or failure.
    #[must_use]
    pub fn raw(
        chunks: Vec<Vec<u8>>,
        exit_code: Option<f64>,
        failure: Option<ShellFailure>,
    ) -> Self {
        Self {
            executed: Mutex::default(),
            started: Arc::default(),
            release: Arc::default(),
            chunks,
            exit_code,
            hold: false,
            failure,
        }
    }

    /// Operations writing `chunks` and settling with `exit_code` at once.
    #[must_use]
    pub fn chunks(chunks: Vec<String>, exit_code: Option<f64>) -> Self {
        Self::raw(
            chunks.into_iter().map(String::into_bytes).collect(),
            exit_code,
            None,
        )
    }

    /// Operations writing `text`, or nothing when it is empty, and settling at once.
    #[must_use]
    pub fn output(text: &str, exit_code: Option<f64>) -> Self {
        let chunks = if text.is_empty() {
            Vec::new()
        } else {
            vec![text.to_owned()]
        };
        Self::chunks(chunks, exit_code)
    }

    /// Operations writing `chunks` that settle with `exit_code` only once released.
    #[must_use]
    pub fn held(chunks: Vec<String>, exit_code: Option<f64>) -> Self {
        Self {
            hold: true,
            ..Self::chunks(chunks, exit_code)
        }
    }

    /// Streams and acknowledges one execution, preserving local or thread-safe callback ownership.
    async fn run<S: FnMut(&[u8]) + ?Sized>(
        &self,
        command: &str,
        cwd: &str,
        on_data: &mut S,
    ) -> Result<BashExecResult, BashError> {
        maestro_lock::lock(&self.executed).push(json!({"command":command,"cwd":cwd}));
        for chunk in &self.chunks {
            on_data(chunk);
        }
        self.started.notify_one();
        if self.hold {
            self.release.notified().await;
        }
        match self.failure {
            Some(ShellFailure::Aborted) => Err(BashError::Aborted),
            Some(ShellFailure::Timeout(seconds)) => Err(BashError::Timeout(seconds)),
            None => Ok(BashExecResult {
                exit_code: self.exit_code,
            }),
        }
    }
}

impl BashOperations<Local> for ScriptedShell {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: BashExecOptions<'a, dyn FnMut(&[u8]) + 'a>,
    ) -> <Local as Execution>::Future<'a, Result<BashExecResult, BashError>> {
        Box::pin(self.run(command, cwd, options.on_data))
    }
}

impl BashOperations<Threaded> for ScriptedShell {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: BashExecOptions<'a>,
    ) -> <Threaded as Execution>::Future<'a, Result<BashExecResult, BashError>> {
        Box::pin(self.run(command, cwd, options.on_data))
    }
}
