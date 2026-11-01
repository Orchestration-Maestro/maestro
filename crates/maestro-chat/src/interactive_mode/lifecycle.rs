//! The interactive mode's stop and exit policy.
use super::InteractiveMode;
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
    time::Duration,
};

/// Host process effects that the presentation selects a policy for.
pub(super) trait LifecycleEffects {
    /// Current wall-clock milliseconds, independent of the frame scheduler.
    fn now_ms(&self) -> i64;
    /// Calls `shutdown` for each termination or hangup signal until removal.
    ///
    /// # Errors
    /// Returns the native registration failure.
    fn register_signals(&self, shutdown: Rc<dyn Fn()>) -> io::Result<Box<dyn Fn()>>;
    /// Kills the existing tracked detached children.
    fn kill_tracked_detached_children(&self);
    /// Ends the process with the selected status.
    fn exit(&self, status: i32);
}
/// Native process effects; native default ownership stays in the process owner.
struct NativeLifecycle;
impl LifecycleEffects for NativeLifecycle {
    fn now_ms(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn register_signals(&self, shutdown: Rc<dyn Fn()>) -> io::Result<Box<dyn Fn()>> {
        maestro_app::register_signals(Rc::new(move |_| shutdown()))
    }
    #[cfg(target_arch = "wasm32")]
    fn register_signals(&self, shutdown: Rc<dyn Fn()>) -> io::Result<Box<dyn Fn()>> {
        let _ = shutdown;
        Ok(Box::new(|| ()))
    }
    fn kill_tracked_detached_children(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        maestro_app::kill_tracked_detached_children();
    }
    fn exit(&self, status: i32) {
        std::process::exit(status);
    }
}
/// What asked for a graceful shutdown, which decides the order of its steps.
#[derive(Clone, Copy)]
enum ShutdownOrigin {
    /// The user, an extension or the agent's end.
    Quit,
    /// A termination or hangup signal.
    Signal,
}
/// Retained state of the mode's exit handlers.
pub(super) struct LifecycleState {
    /// Wall time of the most recent clear branch.
    pub(super) last_sigint_time: Cell<i64>,
    /// Whether graceful or emergency exit already owns the mode.
    is_shutting_down: Cell<bool>,
    /// Removal operations independent of a terminal borrow.
    signal_cleanup_handlers: RefCell<Vec<Box<dyn Fn()>>>,
    /// The process effects, replaced only during private test construction.
    pub(super) effects: Rc<dyn LifecycleEffects>,
}
impl Default for LifecycleState {
    fn default() -> Self {
        Self {
            last_sigint_time: Cell::new(0),
            is_shutting_down: Cell::new(false),
            signal_cleanup_handlers: RefCell::default(),
            effects: Rc::new(NativeLifecycle),
        }
    }
}
impl InteractiveMode {
    /// Removes lifecycle handlers, releases the loader and extension input, then
    /// disposes footer/subscription owners and stops the initialized UI.
    /// The shared theme watcher and unrelated terminal input listeners stay live.
    ///
    /// # Errors
    /// Returns the original terminal progress or stop failure at its phase.
    pub fn stop(&self) -> io::Result<()> {
        self.unregister_signal_handlers();
        self.set_terminal_progress(false)?;
        self.clear_status_indicator(None);
        self.extension_input.clear();
        self.footer.dispose();
        self.footer_data_provider.dispose();
        if let Some(unsubscribe) = self.unsubscribe.take() {
            unsubscribe();
        }
        if self.is_initialized.get() {
            self.ui.stop()?;
            self.is_initialized.set(false);
        }
        Ok(())
    }
    /// Clears the editor unless a second clear falls inside the strict exit window.
    pub(super) fn handle_ctrl_c(self: &Rc<Self>) {
        let now = self.lifecycle.effects.now_ms();
        if i128::from(now) - i128::from(self.lifecycle.last_sigint_time.get()) < 500 {
            self.spawn_shutdown();
        } else {
            self.clear_editor();
            self.lifecycle.last_sigint_time.set(now);
        }
    }
    /// Requests exit after the editor has enforced empty input.
    pub(super) fn handle_ctrl_d(self: &Rc<Self>) {
        self.spawn_shutdown();
    }
    /// Admits shutdown before deferring the owned drain and disposal.
    pub(super) fn spawn_shutdown(self: &Rc<Self>) {
        if !self.lifecycle.is_shutting_down.get() {
            self.tool_environment.runtime.spawn_local(self.shutdown());
        }
    }
    /// Admits a shutdown that a termination signal asked for, deferring its owned disposal,
    /// drain and stop.
    fn spawn_signal_shutdown(self: &Rc<Self>) {
        if !self.lifecycle.is_shutting_down.get() {
            let shutdown = self.begin_shutdown(ShutdownOrigin::Signal);
            self.tool_environment.runtime.spawn_local(shutdown);
        }
    }
    /// Removes handlers synchronously, then owns the pending drain and disposal.
    pub(super) fn shutdown(self: &Rc<Self>) -> maestro_tui::tui::LocalFuture {
        self.begin_shutdown(ShutdownOrigin::Quit)
    }
    /// Admits the shutdown, removes the handlers and returns the rest of the sequence. A quit
    /// drains input and stops the presentation before disposing the runtime, so extension
    /// cleanup cannot repaint the last frame. A signal disposes the runtime first: the
    /// extensions' teardown does not write to the terminal, so it must not be skipped when a
    /// later terminal write fails on a dead or stalled terminal.
    fn begin_shutdown(self: &Rc<Self>, origin: ShutdownOrigin) -> maestro_tui::tui::LocalFuture {
        if self.lifecycle.is_shutting_down.replace(true) {
            return Box::pin(async { Ok(()) });
        }
        self.unregister_signal_handlers();
        let mode = Rc::clone(self);
        match origin {
            ShutdownOrigin::Quit => {
                let drain = self.drain_terminal_input();
                Box::pin(async move {
                    drain.await?;
                    mode.stop()?;
                    mode.runtime_host.dispose().await?;
                    mode.lifecycle.effects.exit(0);
                    Ok(())
                })
            }
            ShutdownOrigin::Signal => Box::pin(async move {
                mode.runtime_host.dispose().await?;
                mode.drain_terminal_input().await?;
                mode.stop()?;
                mode.lifecycle.effects.exit(0);
                Ok(())
            }),
        }
    }
    /// Starts draining the terminal's pending input within the existing 1,000 ms maximum.
    fn drain_terminal_input(&self) -> impl std::future::Future<Output = io::Result<()>> + use<> {
        self.ui
            .terminal()
            .borrow_mut()
            .drain_input(Some(Duration::from_millis(1000)), None)
    }
    /// Records an extension shutdown, deferring active runs until their settlement.
    pub(super) fn request_shutdown(self: &Rc<Self>) {
        self.shutdown_requested.set(true);
        let session = self.runtime_host.session();
        if session.is_idle() {
            self.spawn_shutdown();
            return;
        }
        let mode = Rc::clone(self);
        self.tool_environment
            .runtime
            .spawn_local(Box::pin(async move {
                session.wait_for_idle().await;
                mode.spawn_shutdown();
                Ok(())
            }));
    }
    /// Exits without writing terminal restoration or running runtime disposal.
    fn emergency_terminal_exit(&self) {
        self.lifecycle.is_shutting_down.set(true);
        self.unregister_signal_handlers();
        self.lifecycle.effects.kill_tracked_detached_children();
        self.lifecycle.effects.exit(129);
    }
    /// Installs weak signal and output callbacks before the UI starts.
    pub(super) fn register_signal_handlers(self: &Rc<Self>) -> io::Result<()> {
        self.unregister_signal_handlers();
        let mode = Rc::downgrade(self);
        let shutdown: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(mode) = mode.upgrade() {
                mode.lifecycle.effects.kill_tracked_detached_children();
                mode.spawn_signal_shutdown();
            }
        });
        let remove = self.lifecycle.effects.register_signals(shutdown)?;
        self.lifecycle
            .signal_cleanup_handlers
            .borrow_mut()
            .push(remove);
        let mode = Rc::downgrade(self);
        let handler: maestro_tui::terminal::OutputErrorHandler = Rc::new(move |error| {
            if is_dead_terminal_error(error)
                && let Some(mode) = mode.upgrade()
            {
                mode.emergency_terminal_exit();
            }
        });
        let stdout = self
            .ui
            .terminal()
            .borrow()
            .on_output_error(Rc::clone(&handler));
        let stderr = self.tool_environment.runtime.on_output_error(handler);
        self.lifecycle
            .signal_cleanup_handlers
            .borrow_mut()
            .extend([stdout, stderr]);
        Ok(())
    }
    /// Runs each remover outside the retained vector's borrow.
    pub(super) fn unregister_signal_handlers(&self) {
        let removers = self.lifecycle.signal_cleanup_handlers.take();
        for remove in removers {
            remove();
        }
    }
}
/// Whether the real native error is one of the terminal-disconnection failures.
fn is_dead_terminal_error(error: &io::Error) -> bool {
    if matches!(
        error.kind(),
        io::ErrorKind::BrokenPipe | io::ErrorKind::NotConnected
    ) {
        return true;
    }
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(rustix::io::Errno::IO.raw_os_error())
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Releases native registrations when the Rust presentation owner is released.
impl Drop for InteractiveMode {
    fn drop(&mut self) {
        self.unregister_signal_handlers();
    }
}
