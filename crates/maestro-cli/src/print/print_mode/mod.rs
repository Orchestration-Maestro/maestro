//! One session run presented as its final text or as every session event.
mod callbacks;
mod signals;

use crate::print::print_mode::callbacks::command_actions;
use maestro_app::{
    AgentMessage, AgentSession, AgentSessionEvent, AgentSessionEventListener, AgentSessionRuntime,
    AssistantContent, DiagnosticErrorInfo, ExtensionBindings, FileEntry, ForkOptions, ImageContent,
    NavigateTreeOptions, NavigateTreeResult, NewSessionCommandOptions, PromptOptions,
    ReadonlySessionManager, RebindSession, SessionChangeResult, StopReason, SwitchSessionOptions,
    TerminationSignal, kill_tracked_detached_children, serialize_json_line,
};
pub use signals::{NativeSignals, SignalPort, signals_for_host};
use std::{
    cell::RefCell,
    future::Future,
    io,
    pin::Pin,
    rc::{Rc, Weak},
    sync::PoisonError,
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
pub type PrintFuture<T> = Pin<Box<dyn Future<Output = Result<T, DiagnosticErrorInfo>>>>;

/// Replaces the run's session with a replacement one.
pub type PrintRebind = Rc<dyn Fn(Rc<dyn PrintSession>) -> PrintFuture<()>>;

/// The final history entry as text mode reads it.
#[derive(Clone)]
pub enum FinalMessage {
    /// An assistant response with its outcome and text blocks, in order.
    Assistant {
        /// How the response ended.
        stop_reason: StopReason,
        /// The supplied failure message.
        error_message: Option<String>,
        /// The text blocks.
        text: Vec<String>,
    },
    /// Any other final entry.
    Other,
}

/// The session operations a print run drives.
pub trait PrintSession {
    fn bind_extensions(&self, bindings: ExtensionBindings) -> PrintFuture<()>;
    fn subscribe(&self, listener: AgentSessionEventListener) -> Box<dyn Fn()>;
    fn prompt(&self, text: String, options: PromptOptions) -> PrintFuture<()>;
    fn wait_for_idle(&self) -> PrintFuture<()>;
    fn navigate_tree(
        &self,
        target_id: String,
        options: NavigateTreeOptions,
    ) -> PrintFuture<NavigateTreeResult>;
    fn reload(&self) -> PrintFuture<()>;
    fn header(&self) -> Option<FileEntry>;
    fn final_message(&self) -> Option<FinalMessage>;
}

/// The runtime operations a print run drives.
pub trait PrintRuntime {
    fn session(&self) -> Rc<dyn PrintSession>;
    fn set_rebind(&self, rebind: Option<PrintRebind>);
    fn new_session(&self, options: NewSessionCommandOptions) -> PrintFuture<SessionChangeResult>;
    fn fork(&self, entry_id: String, options: ForkOptions) -> PrintFuture<SessionChangeResult>;
    fn switch_session(
        &self,
        session_path: String,
        options: SwitchSessionOptions,
    ) -> PrintFuture<SessionChangeResult>;
    fn dispose(&self) -> PrintFuture<()>;
}

/// The output, signal and child-process effects of a print run.
pub struct PrintEffects {
    /// Writes protocol and final bytes to raw standard output.
    pub write_stdout: Rc<dyn Fn(&str) -> io::Result<()>>,
    /// Writes one diagnostic line to standard error.
    pub write_stderr_line: Rc<dyn Fn(&str)>,
    /// Flushes raw standard output.
    pub flush_stdout: Rc<dyn Fn() -> io::Result<()>>,
    /// Kills the tracked detached children.
    pub kill_children: Rc<dyn Fn()>,
    /// Delivers termination signals and ends the process.
    pub signals: Rc<dyn SignalPort>,
}

impl PrintEffects {
    /// The host's own output, child cleanup and signal effects.
    pub fn native() -> Self {
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
    fn write_event(&self, event: &AgentSessionEvent) {
        if self.mode != PrintMode::Json {
            return;
        }
        let Ok(value) = serde_json::to_value(event) else {
            return;
        };
        let Ok(line) = serialize_json_line(&value) else {
            return;
        };
        // A write failure has no caller to reject here: a session's emit ignores listener results.
        drop((self.effects.write_stdout)(&line));
    }

    /// Binds the current session's extensions, then replaces the event subscription.
    async fn bind_current(self: Rc<Self>) -> Result<(), DiagnosticErrorInfo> {
        let session = self.current_session();
        let on_error = {
            let run = Rc::downgrade(&self);
            Rc::new(move |error: &maestro_app::ExtensionError| {
                if let Some(run) = run.upgrade() {
                    run.write_stderr(&format!(
                        "Extension error ({}): {}",
                        error.extension_path, error.error
                    ));
                }
                Ok(())
            })
        };
        session
            .bind_extensions(ExtensionBindings {
                ui_context: None,
                command_context_actions: Some(command_actions(Rc::downgrade(&self))),
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
    async fn body(self: &Rc<Self>, options: &PrintModeOptions) -> Result<i32, DiagnosticErrorInfo> {
        if self.mode == PrintMode::Json {
            if let Some(header) = self.current_session().header() {
                self.write_stdout(&serialize_json_line(&header).map_err(message_of)?)?;
            }
        }
        Rc::clone(self).bind_current().await?;
        let initial = options
            .initial_message
            .as_ref()
            .filter(|text| !text.is_empty());
        if let Some(initial) = initial {
            let options = PromptOptions {
                images: options.initial_images.clone(),
                ..PromptOptions::default()
            };
            self.current_session()
                .prompt(initial.clone(), options)
                .await?;
        }
        for message in &options.messages {
            self.current_session()
                .prompt(message.clone(), PromptOptions::default())
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
        let Some(FinalMessage::Assistant {
            stop_reason,
            error_message,
            text,
        }) = self.current_session().final_message()
        else {
            return Ok(0);
        };
        if matches!(stop_reason, StopReason::Error | StopReason::Aborted) {
            let reason = if stop_reason == StopReason::Error {
                "Request error"
            } else {
                "Request aborted"
            };
            let message = error_message
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| reason.to_owned());
            self.write_stderr(&message);
            return Ok(1);
        }
        for block in text {
            self.write_stdout(&format!("{block}\n"))?;
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
pub async fn run_print_mode_with(
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
    let outcome = run.body(&options).await;
    let status = match outcome {
        Ok(status) => status,
        Err(error) => {
            run.write_stderr(&error.message);
            1
        }
    };
    run.finish().await?;
    Ok(status)
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

    fn wait_for_idle(&self) -> PrintFuture<()> {
        let session = Rc::clone(&self.0);
        Box::pin(async move {
            session.agent().wait_for_idle().await;
            Ok(())
        })
    }

    fn navigate_tree(
        &self,
        target_id: String,
        options: NavigateTreeOptions,
    ) -> PrintFuture<NavigateTreeResult> {
        let session = Rc::clone(&self.0);
        Box::pin(async move { session.navigate_tree(&target_id, options).await })
    }

    fn reload(&self) -> PrintFuture<()> {
        let session = Rc::clone(&self.0);
        Box::pin(async move { session.reload().await })
    }

    fn header(&self) -> Option<FileEntry> {
        self.0.session_manager().borrow().get_header().cloned()
    }

    fn final_message(&self) -> Option<FinalMessage> {
        let messages = self.0.messages();
        let messages = messages.read().unwrap_or_else(PoisonError::into_inner);
        match messages.last()? {
            AgentMessage::Assistant(assistant) => {
                let assistant = assistant.read().unwrap_or_else(PoisonError::into_inner);
                Some(FinalMessage::Assistant {
                    stop_reason: assistant.stop_reason.clone(),
                    error_message: assistant.error_message.clone(),
                    text: assistant
                        .content
                        .iter()
                        .filter_map(|content| match content {
                            AssistantContent::Text(text) => Some(text.text.clone()),
                            AssistantContent::Thinking(_) | AssistantContent::ToolCall(_) => None,
                        })
                        .collect(),
                })
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

    fn new_session(&self, options: NewSessionCommandOptions) -> PrintFuture<SessionChangeResult> {
        let runtime = Rc::clone(&self.inner);
        Box::pin(async move { runtime.new_session(options).await.map_err(message_of) })
    }

    fn fork(&self, entry_id: String, options: ForkOptions) -> PrintFuture<SessionChangeResult> {
        let runtime = Rc::clone(&self.inner);
        Box::pin(async move {
            let forked = runtime.fork(&entry_id, options).await.map_err(message_of)?;
            Ok(SessionChangeResult {
                cancelled: forked.cancelled,
            })
        })
    }

    fn switch_session(
        &self,
        session_path: String,
        options: SwitchSessionOptions,
    ) -> PrintFuture<SessionChangeResult> {
        let runtime = Rc::clone(&self.inner);
        Box::pin(async move {
            runtime
                .switch_session(&session_path, None, options)
                .await
                .map_err(message_of)
        })
    }

    fn dispose(&self) -> PrintFuture<()> {
        let runtime = Rc::clone(&self.inner);
        Box::pin(async move { runtime.dispose().await })
    }
}
