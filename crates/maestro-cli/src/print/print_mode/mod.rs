//! One session run presented as its final text or as every session event.
mod signals;
#[cfg(test)]
mod tests;

use maestro_app::{
    AgentMessage, AgentSession, AgentSessionEvent, AgentSessionEventListener, AgentSessionRuntime,
    AssistantContent, DiagnosticErrorInfo, ExtensionBindings, ExtensionCommandContextActions,
    FileEntry, ImageContent, PromptOptions, ReadonlySessionManager, RebindSession,
    SharedAssistantMessage, StopReason, TerminationSignal, kill_tracked_detached_children,
    serialize_json_line,
};
pub(crate) use signals::{NativeSignals, SignalPort};
use std::{
    cell::RefCell,
    future::Future,
    io,
    pin::Pin,
    rc::{Rc, Weak},
    sync::{Arc, PoisonError},
};

/// Which result a run presents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintMode {
    /// The final answer alone.
    Text,
    /// Every session event.
    Json,
}

/// The inputs of one run.
pub struct PrintModeOptions {
    /// The presented result.
    pub mode: PrintMode,
    /// Prompts sent after the first one, in order.
    pub messages: Vec<String>,
    /// The first prompt, sent with the initial images when it holds text.
    pub initial_message: Option<String>,
    /// Images attached to the first prompt.
    pub initial_images: Option<Vec<ImageContent>>,
}

/// Runs one session to completion and presents its result.
///
/// # Errors
/// Returns a cleanup failure: a guarded disposal or the final flush rejects instead of
/// becoming a status.
pub async fn run_print_mode(
    runtime_host: Rc<AgentSessionRuntime>,
    options: PrintModeOptions,
) -> Result<i32, DiagnosticErrorInfo> {
    run_print_mode_with(
        Rc::new(RuntimeAdapter {
            inner: runtime_host,
        }),
        options,
        PrintEffects::native(),
    )
    .await
}

/// An operation of a print run.
pub(crate) type PrintFuture<T> = Pin<Box<dyn Future<Output = Result<T, DiagnosticErrorInfo>>>>;

/// Replaces the run's session with a replacement one.
pub(crate) type PrintRebind = Rc<dyn Fn(Rc<dyn PrintSession>) -> PrintFuture<()>>;

/// The final history entry as text mode reads it.
#[derive(Clone)]
pub(crate) enum FinalMessage {
    /// The assistant response, shared with the session's history.
    Assistant(SharedAssistantMessage),
    /// Any other final entry.
    Other,
}

/// The session operations a print run drives.
pub(crate) trait PrintSession {
    /// Binds the session's extensions.
    fn bind_extensions(&self, bindings: ExtensionBindings) -> PrintFuture<()>;
    /// Adds an event listener; the returned operation removes it.
    fn subscribe(&self, listener: AgentSessionEventListener) -> Box<dyn Fn()>;
    /// Sends one prompt.
    fn prompt(&self, text: String, options: PromptOptions) -> PrintFuture<()>;
    /// The session header, when the session stores one.
    fn header(&self) -> Option<FileEntry>;
    /// The final history entry.
    fn final_message(&self) -> Option<FinalMessage>;
}

/// The runtime operations a print run drives.
pub(crate) trait PrintRuntime {
    /// The session now in effect.
    fn session(&self) -> Rc<dyn PrintSession>;
    /// Sets the operation the runtime calls for each replacement session.
    fn set_rebind(&self, rebind: Option<PrintRebind>);
    /// The command-context operations the run binds for the current session.
    fn command_actions(&self) -> ExtensionCommandContextActions;
    /// Disposes the current session.
    fn dispose(&self) -> PrintFuture<()>;
}

/// Writes protocol and final bytes to raw standard output.
pub(crate) type WriteStdout = Rc<dyn Fn(&str) -> io::Result<()>>;

/// Writes one diagnostic line to standard error.
pub(crate) type WriteStderrLine = Rc<dyn Fn(&str)>;

/// Flushes raw standard output.
pub(crate) type FlushStdout = Rc<dyn Fn() -> io::Result<()>>;

/// Kills the tracked detached children.
pub(crate) type KillChildren = Rc<dyn Fn()>;

/// The output, signal and child-process effects of a print run.
pub(crate) struct PrintEffects {
    /// Writes protocol and final bytes to raw standard output.
    pub(crate) write_stdout: WriteStdout,
    /// Writes one diagnostic line to standard error.
    pub(crate) write_stderr_line: WriteStderrLine,
    /// Flushes raw standard output.
    pub(crate) flush_stdout: FlushStdout,
    /// Kills the tracked detached children.
    pub(crate) kill_children: KillChildren,
    /// Delivers termination signals and ends the process.
    pub(crate) signals: Rc<dyn SignalPort>,
}

impl PrintEffects {
    /// The host's own output, child cleanup and signal effects.
    pub(crate) fn native() -> Self {
        Self {
            write_stdout: Rc::new(maestro_app::output_guard::write_raw_stdout),
            write_stderr_line: Rc::new(|line| {
                let mut text = String::with_capacity(line.len() + 1);
                text.push_str(line);
                text.push('\n');
                drop(io::Write::write_all(&mut io::stderr(), text.as_bytes()));
            }),
            flush_stdout: Rc::new(maestro_app::output_guard::flush_raw_stdout),
            kill_children: Rc::new(kill_tracked_detached_children),
            signals: Rc::new(NativeSignals),
        }
    }
}

/// A run's live session, subscription and completion state.
struct RunState {
    /// The session now in effect.
    session: Rc<dyn PrintSession>,
    /// Removes the current event subscription.
    unsubscribe: Option<Box<dyn Fn()>>,
    /// Whether a disposal started.
    disposed: bool,
    /// Stops the run's own signal delivery.
    signal_cleanup: Vec<Box<dyn Fn()>>,
    /// The first event record the run could not write.
    event_failure: Option<DiagnosticErrorInfo>,
}

/// One run over its runtime and effects.
struct PrintRun {
    /// The runtime the run drives.
    runtime: Rc<dyn PrintRuntime>,
    /// Its output, signal and child effects.
    effects: PrintEffects,
    /// The presented result.
    mode: PrintMode,
    /// Its live state.
    state: RefCell<RunState>,
}

impl PrintRun {
    /// The session now in effect.
    fn current_session(&self) -> Rc<dyn PrintSession> {
        Rc::clone(&self.state.borrow().session)
    }

    /// Writes `text` to raw standard output.
    ///
    /// # Errors
    /// Returns the write failure.
    fn write_stdout(&self, text: &str) -> Result<(), DiagnosticErrorInfo> {
        (self.effects.write_stdout)(text).map_err(message_of)
    }

    /// Writes one diagnostic line to standard error.
    fn write_stderr(&self, line: &str) {
        (self.effects.write_stderr_line)(line);
    }

    /// Writes one session event while `mode` is JSON.
    ///
    /// A session's emit ignores listener results, so the first failure waits for the run's own
    /// error path instead of ending the stream silently.
    fn write_event(&self, event: &AgentSessionEvent) {
        if self.mode != PrintMode::Json {
            return;
        }
        if let Err(error) = self.write_record(event) {
            let mut state = self.state.borrow_mut();
            state.event_failure = state.event_failure.take().or(Some(error));
        }
    }

    /// Writes `event` as one compact JSONL record.
    ///
    /// # Errors
    /// Returns the encoding or the write failure.
    fn write_record(&self, event: &AgentSessionEvent) -> Result<(), DiagnosticErrorInfo> {
        let value = serde_json::to_value(event).map_err(message_of)?;
        self.write_stdout(&serialize_json_line(&value).map_err(message_of)?)
    }

    /// The first event record the run could not write, if any.
    fn event_failure(&self) -> Option<DiagnosticErrorInfo> {
        self.state.borrow_mut().event_failure.take()
    }

    /// Binds the current session's extensions, then replaces the event subscription.
    async fn bind_current(self: Rc<Self>) -> Result<(), DiagnosticErrorInfo> {
        let session = self.current_session();
        let on_error = error_listener(Rc::downgrade(&self));
        session
            .bind_extensions(ExtensionBindings {
                ui_context: None,
                command_context_actions: Some(self.runtime.command_actions()),
                abort_handler: None,
                shutdown_handler: None,
                on_error: Some(on_error),
            })
            .await?;
        let previous = self.state.borrow_mut().unsubscribe.take();
        if let Some(previous) = previous {
            previous();
        }
        let run = Rc::downgrade(&self);
        let unsubscribe = session.subscribe(Rc::new(move |event| {
            if let Some(run) = run.upgrade() {
                run.write_event(event);
            }
        }));
        self.state.borrow_mut().unsubscribe = Some(unsubscribe);
        Ok(())
    }

    /// Makes `session` the run's current session, then binds it.
    async fn rebind(
        self: Rc<Self>,
        session: Rc<dyn PrintSession>,
    ) -> Result<(), DiagnosticErrorInfo> {
        self.state.borrow_mut().session = session;
        self.bind_current().await
    }

    /// Writes the header and runs the prompts, presenting the result text mode reads.
    async fn body(self: &Rc<Self>, options: PrintModeOptions) -> Result<i32, DiagnosticErrorInfo> {
        if self.mode == PrintMode::Json
            && let Some(header) = self.current_session().header()
        {
            self.write_stdout(&serialize_json_line(&header).map_err(message_of)?)?;
        }
        Rc::clone(self).bind_current().await?;
        if let Some(initial) = options.initial_message.filter(|text| !text.is_empty()) {
            let prompt = PromptOptions {
                images: options.initial_images,
                ..PromptOptions::default()
            };
            self.current_session().prompt(initial, prompt).await?;
        }
        for message in options.messages {
            self.current_session()
                .prompt(message, PromptOptions::default())
                .await?;
        }
        if self.mode == PrintMode::Json {
            return Ok(0);
        }
        self.present_final_message()
    }

    /// Presents text mode's final answer or failure.
    ///
    /// # Errors
    /// Returns a write failure.
    fn present_final_message(&self) -> Result<i32, DiagnosticErrorInfo> {
        let Some(FinalMessage::Assistant(assistant)) = self.current_session().final_message()
        else {
            return Ok(0);
        };
        let assistant = assistant.read().unwrap_or_else(PoisonError::into_inner);
        if matches!(
            assistant.stop_reason,
            StopReason::Error | StopReason::Aborted
        ) {
            let reason = if assistant.stop_reason == StopReason::Error {
                "Request error"
            } else {
                "Request aborted"
            };
            let message = assistant
                .error_message
                .as_deref()
                .filter(|text| !text.is_empty())
                .unwrap_or(reason);
            self.write_stderr(message);
            return Ok(1);
        }
        for content in &assistant.content {
            if let AssistantContent::Text(text) = content {
                self.write_stdout(&format!("{}\n", text.text))?;
            }
        }
        Ok(0)
    }

    /// Registers this run's signal delivery.
    ///
    /// # Errors
    /// Returns the native registration failure.
    fn register_signals(self: &Rc<Self>) -> io::Result<()> {
        let run = Rc::downgrade(self);
        let cleanup =
            (self.effects.signals).register(Rc::new(move |signal: TerminationSignal| {
                if let Some(run) = run.upgrade() {
                    run.on_signal(signal);
                }
            }))?;
        self.state.borrow_mut().signal_cleanup.push(cleanup);
        Ok(())
    }

    /// Kills the tracked children, then starts the guarded disposal and exits once it settles.
    fn on_signal(self: &Rc<Self>, signal: TerminationSignal) {
        (self.effects.kill_children)();
        let run = Rc::clone(self);
        drop(tokio::task::spawn_local(async move {
            drop(run.dispose_runtime().await);
            (run.effects.signals).exit(128 + signal.as_raw());
        }));
    }

    /// Removes the current subscription and disposes the runtime once.
    async fn dispose_runtime(&self) -> Result<(), DiagnosticErrorInfo> {
        if self.state.borrow().disposed {
            return Ok(());
        }
        let unsubscribe = {
            let mut state = self.state.borrow_mut();
            state.disposed = true;
            state.unsubscribe.take()
        };
        if let Some(unsubscribe) = unsubscribe {
            unsubscribe();
        }
        self.runtime.dispose().await
    }

    /// Reports every failure the run retained and returns the exit status.
    fn settle(&self, outcome: Result<i32, DiagnosticErrorInfo>) -> i32 {
        let event = self.event_failure();
        if let Some(error) = &event {
            self.write_stderr(&error.message);
        }
        match outcome {
            Err(error) => {
                self.write_stderr(&error.message);
                1
            }
            Ok(status) if event.is_none() => status,
            Ok(_) => 1,
        }
    }

    /// Removes this run's signal handlers, disposes the runtime and flushes raw output.
    async fn finish(&self) -> Result<(), DiagnosticErrorInfo> {
        let cleanups = std::mem::take(&mut self.state.borrow_mut().signal_cleanup);
        for cleanup in cleanups {
            cleanup();
        }
        self.dispose_runtime().await?;
        (self.effects.flush_stdout)().map_err(message_of)
    }
}

/// Runs `options` over `runtime`, presenting through `effects`.
///
/// # Errors
/// Returns a cleanup failure: a guarded disposal or the final flush rejects.
pub(crate) async fn run_print_mode_with(
    runtime: Rc<dyn PrintRuntime>,
    options: PrintModeOptions,
    effects: PrintEffects,
) -> Result<i32, DiagnosticErrorInfo> {
    let run = Rc::new(PrintRun {
        mode: options.mode,
        state: RefCell::new(RunState {
            session: runtime.session(),
            unsubscribe: None,
            disposed: false,
            signal_cleanup: Vec::new(),
            event_failure: None,
        }),
        runtime,
        effects,
    });
    run.register_signals().map_err(message_of)?;
    let weak: Weak<PrintRun> = Rc::downgrade(&run);
    run.runtime.set_rebind(Some(Rc::new(move |session| {
        let run = weak.upgrade();
        Box::pin(async move {
            match run {
                Some(run) => run.rebind(session).await,
                None => Ok(()),
            }
        })
    })));
    let status = run.settle(run.body(options).await);
    run.finish().await?;
    Ok(status)
}

/// The listener that reports extension failures.
type ErrorListener = Rc<
    dyn Fn(&maestro_app::ExtensionError) -> Result<(), Box<dyn std::error::Error + Send + Sync>>,
>;

/// The listener that reports extension failures on standard error.
fn error_listener(run: Weak<PrintRun>) -> ErrorListener {
    Rc::new(move |error: &maestro_app::ExtensionError| {
        let Some(run) = run.upgrade() else {
            return Ok(());
        };
        run.write_stderr(&format!(
            "Extension error ({}): {}",
            error.extension_path, error.error
        ));
        Ok(())
    })
}

/// The diagnostic a lower failure becomes, keeping its message.
fn message_of(error: impl std::fmt::Display) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error(error.to_string())
}

/// The application session behind a print run.
struct SessionAdapter(Rc<AgentSession>);

impl PrintSession for SessionAdapter {
    fn bind_extensions(&self, bindings: ExtensionBindings) -> PrintFuture<()> {
        let session = Rc::clone(&self.0);
        Box::pin(async move { session.bind_extensions(bindings).await })
    }

    fn subscribe(&self, listener: AgentSessionEventListener) -> Box<dyn Fn()> {
        Box::new(self.0.subscribe(listener))
    }

    fn prompt(&self, text: String, options: PromptOptions) -> PrintFuture<()> {
        let session = Rc::clone(&self.0);
        Box::pin(async move { session.prompt(&text, options).await })
    }

    fn header(&self) -> Option<FileEntry> {
        self.0.session_manager().borrow().get_header().cloned()
    }

    fn final_message(&self) -> Option<FinalMessage> {
        let messages = self.0.messages();
        let messages = messages.read().unwrap_or_else(PoisonError::into_inner);
        match messages.last()? {
            AgentMessage::Assistant(assistant) => {
                Some(FinalMessage::Assistant(Arc::clone(assistant)))
            }
            AgentMessage::User(_) | AgentMessage::ToolResult(_) | AgentMessage::Custom(_) => {
                Some(FinalMessage::Other)
            }
        }
    }
}

/// The application runtime behind a print run.
struct RuntimeAdapter {
    /// The runtime the run drives.
    inner: Rc<AgentSessionRuntime>,
}

impl PrintRuntime for RuntimeAdapter {
    fn session(&self) -> Rc<dyn PrintSession> {
        Rc::new(SessionAdapter(self.inner.session()))
    }

    fn set_rebind(&self, rebind: Option<PrintRebind>) {
        let mapped: Option<RebindSession> = rebind.map(|rebind| {
            let mapped: RebindSession = Rc::new(move |session: Rc<AgentSession>| {
                rebind(Rc::new(SessionAdapter(session)) as Rc<dyn PrintSession>)
            });
            mapped
        });
        self.inner.set_rebind_session(mapped);
    }

    fn command_actions(&self) -> ExtensionCommandContextActions {
        self.inner.command_context_actions()
    }

    fn dispose(&self) -> PrintFuture<()> {
        let runtime = Rc::clone(&self.inner);
        Box::pin(async move { runtime.dispose().await })
    }
}
