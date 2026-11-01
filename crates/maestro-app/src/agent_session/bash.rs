//! Direct shell commands and their recorded transcript.
use super::state::thrown;
use super::{AgentSession, BashChunkCallback, BashRunOptions};
use crate::utils::lock::write;
use maestro_agent::AgentMessage;
use maestro_extensions::{ExtensionExecution, NativeAgentMessage};
use maestro_models::{Cancellation, DiagnosticErrorInfo, timestamp_now};
use maestro_session::Presence;
use maestro_session::messages::{BashExecutionMessage, CustomAgentMessages};
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_tools::{
    BashError, BashResult,
    process::{
        bash::local::create_local_bash_operations,
        bash_executor::local::{BashExecutorOptions, execute_bash_with_operations},
    },
};
use std::rc::Rc;
use std::sync::{Arc, RwLock};

impl<E: ExtensionExecution> AgentSession<E> {
    /// Runs `command` in the session directory and records the result under `command`. The
    /// configured command prefix is put on its own line before the command it runs; the
    /// configured shell path runs it unless `options` supply the operations. Calling this starts
    /// the command. It belongs to the session: dropping even the never-polled future leaves it running,
    /// and `abort_bash` stops it.
    ///
    /// # Errors
    /// Returns the shell operation's failure, an output failure or the session history's
    /// failure.
    pub fn execute_bash(
        self: &Rc<Self>,
        command: &str,
        on_chunk: Option<BashChunkCallback>,
        options: BashRunOptions,
    ) -> impl std::future::Future<Output = Result<BashResult, DiagnosticErrorInfo>> + use<E> {
        let session = Rc::clone(self);
        let command = command.to_owned();
        let result = self
            .start_operation(async move { session.run_bash(&command, on_chunk, options).await });
        async move { result.await.map_err(thrown)? }
    }

    /// The shell command the session runs, from installing its signal to recording its
    /// result.
    async fn run_bash(
        &self,
        command: &str,
        mut on_chunk: Option<BashChunkCallback>,
        options: BashRunOptions,
    ) -> Result<BashResult, DiagnosticErrorInfo> {
        let signal = Cancellation::new();
        self.state.borrow_mut().bash_signal = Some(signal.clone());
        let (prefix, shell_path) = {
            let settings = self.settings_manager.borrow();
            (
                settings.get_shell_command_prefix(),
                settings.get_shell_path(),
            )
        };
        let resolved = match prefix.filter(|prefix| !prefix.is_empty()) {
            Some(prefix) => format!("{prefix}\n{command}"),
            None => command.to_owned(),
        };
        let operations = options
            .operations
            .unwrap_or_else(|| create_local_bash_operations(self.bin_dir.clone(), shell_path));
        let cwd = self.session_manager.borrow().get_cwd().to_owned();
        let executor = BashExecutorOptions {
            // Shortens the callback's lifetime to the signal's borrow.
            on_chunk: on_chunk
                .as_deref_mut()
                .map(|chunk| -> &mut dyn FnMut(&str) { chunk }),
            signal: Some(&signal),
        };
        let result = execute_bash_with_operations(&resolved, &cwd, &*operations, executor).await;
        self.state.borrow_mut().bash_signal = None;
        let result = result.map_err(|error| match error {
            // The operation's own failure keeps the diagnostic it carries.
            BashError::Other(error) => thrown(error),
            error => thrown(error),
        })?;
        self.record_bash_result(command, &result, options.exclude_from_context)?;
        Ok(result)
    }

    /// Records a shell result: appended to the history and the session at once, or kept
    /// until the next prompt while a response streams, so the response's tool calls and
    /// results stay together.
    ///
    /// # Errors
    /// Returns the session history's failure.
    pub fn record_bash_result(
        &self,
        command: &str,
        result: &BashResult,
        exclude_from_context: Option<bool>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let message = AgentMessage::Custom(Arc::new(RwLock::new(
            CustomAgentMessages::BashExecution(BashExecutionMessage {
                command: command.to_owned(),
                output: result.output.clone(),
                exit_code: result
                    .exit_code
                    .map_or(Presence::Missing, Presence::Present),
                cancelled: result.cancelled,
                truncated: result.truncated,
                full_output_path: (result.full_output_path.clone())
                    .map_or(Presence::Missing, Presence::Present),
                timestamp: timestamp_now(),
                exclude_from_context: exclude_from_context
                    .map_or(Presence::Missing, Presence::Present),
            }),
        )));
        if self.is_streaming() {
            self.state
                .borrow_mut()
                .pending_bash_messages
                .push_back(message);
            return Ok(());
        }
        self.append_bash_message(message)
    }

    /// Stops the running shell command.
    pub fn abort_bash(&self) {
        let signal = self.state.borrow().bash_signal.clone();
        if let Some(signal) = signal {
            signal.abort();
        }
    }

    /// Whether a shell command is running.
    #[must_use]
    pub fn is_bash_running(&self) -> bool {
        self.state.borrow().bash_signal.is_some()
    }

    /// Whether shell results wait for the next prompt.
    #[must_use]
    pub fn has_pending_bash_messages(&self) -> bool {
        !self.state.borrow().pending_bash_messages.is_empty()
    }

    /// Appends the shell results kept while a response streamed.
    ///
    /// # Errors
    /// Returns the session history's first failure.
    pub(super) fn flush_pending_bash_messages(&self) -> Result<(), DiagnosticErrorInfo> {
        // Each result leaves the queue before its append, so a failed write keeps only the
        // results not yet appended.
        loop {
            let next = self.state.borrow_mut().pending_bash_messages.pop_front();
            let Some(message) = next else {
                return Ok(());
            };
            self.append_bash_message(message)?;
        }
    }

    /// Appends one shell result to the history and the session.
    fn append_bash_message(&self, message: NativeAgentMessage) -> Result<(), DiagnosticErrorInfo> {
        let record = serde_json::to_value(&message).map_err(thrown)?;
        write(&self.messages()).push(message);
        self.session_manager
            .borrow_mut()
            .append_message(record)
            .map_err(thrown)?;
        Ok(())
    }
}
