//! Controlled failures and output sources around the shared recording ports.
use maestro_tui::Terminal;
use maestro_tui::terminal::OutputErrorHandler;
use maestro_tui::tui::{LocalFuture, LogContext, RenderCallback, RenderTimer, TuiRuntime};
use std::{
    cell::{Cell, RefCell},
    io,
    path::Path,
    rc::Rc,
    time::Duration,
};

/// A single test producer's live handler, independent of terminal borrows.
#[derive(Clone, Default)]
pub(super) struct Output(Rc<RefCell<Option<OutputErrorHandler>>>);
impl Output {
    /// Notifies the current handler outside the producer's borrow.
    pub(super) fn emit(&self, error: &io::Error) {
        let handler = self.0.borrow().clone();
        if let Some(handler) = handler {
            handler(error);
        }
    }
    /// Retains one handler and returns its identity-sensitive removal.
    fn register(&self, handler: OutputErrorHandler) -> Box<dyn Fn()> {
        self.0.replace(Some(Rc::clone(&handler)));
        let slot = Rc::clone(&self.0);
        Box::new(move || {
            let matches = slot
                .borrow()
                .as_ref()
                .is_some_and(|live| Rc::ptr_eq(live, &handler));
            if matches {
                drop(slot.take());
            }
        })
    }
    /// Registers `handler` with the native producer through `native` and with this output,
    /// and returns the one removal that runs both, the native one first.
    fn attach(
        &self,
        native: impl FnOnce(OutputErrorHandler) -> Box<dyn Fn()>,
        handler: OutputErrorHandler,
    ) -> Box<dyn Fn()> {
        let native = native(Rc::clone(&handler));
        let controlled = self.register(handler);
        Box::new(move || {
            native();
            controlled();
        })
    }
    /// Whether a callback is still registered.
    pub(super) fn registered(&self) -> bool {
        self.0.borrow().is_some()
    }
}

/// A live observation at owned-drain creation.
pub(super) type DrainObservation = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// The recording terminal with one controlled drain failure and output producer.
pub(super) struct TerminalPort {
    /// An observation at owned-drain creation, before any asynchronous polling.
    pub(super) before_drain: DrainObservation,
    /// The real recording port whose remaining effects are preserved.
    pub(super) inner: Box<dyn Terminal>,
    /// The stdout failure callback installed by initialization.
    pub(super) output: Output,
    /// The drain failure, or normal owned drain behavior.
    pub(super) drain_error: Rc<Cell<Option<io::ErrorKind>>>,
    /// Complete operation order shared with process and disposal observations.
    pub(super) events: Rc<RefCell<Vec<String>>>,
}
impl Terminal for TerminalPort {
    fn start(&mut self, input: Box<dyn FnMut(&str)>, resize: Box<dyn FnMut()>) -> io::Result<()> {
        self.inner.start(input, resize)
    }
    fn stop(&mut self) -> io::Result<()> {
        self.inner.stop()
    }
    fn drain_input(
        &mut self,
        max: Option<Duration>,
        idle: Option<Duration>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<()>>>> {
        assert_eq!(max, Some(Duration::from_millis(1000)));
        assert_eq!(idle, None);
        let observe = self.before_drain.borrow().clone();
        if let Some(observe) = observe {
            observe();
        }
        self.events.borrow_mut().push("drain".into());
        match self.drain_error.get() {
            Some(kind) => Box::pin(async move { Err(io::Error::from(kind)) }),
            None => self.inner.drain_input(max, idle),
        }
    }
    fn write(&mut self, text: &str) -> io::Result<()> {
        self.inner.write(text)
    }
    fn columns(&self) -> usize {
        self.inner.columns()
    }
    fn rows(&self) -> usize {
        self.inner.rows()
    }
    fn kitty_protocol_active(&self) -> bool {
        self.inner.kitty_protocol_active()
    }
    fn move_by(&mut self, lines: isize) -> io::Result<()> {
        self.inner.move_by(lines)
    }
    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }
    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }
    fn clear_line(&mut self) -> io::Result<()> {
        self.inner.clear_line()
    }
    fn clear_from_cursor(&mut self) -> io::Result<()> {
        self.inner.clear_from_cursor()
    }
    fn clear_screen(&mut self) -> io::Result<()> {
        self.inner.clear_screen()
    }
    fn set_title(&mut self, title: &str) -> io::Result<()> {
        self.inner.set_title(title)
    }
    fn set_progress(&mut self, active: bool) -> io::Result<()> {
        self.inner.set_progress(active)
    }
    fn on_output_error(&self, handler: OutputErrorHandler) -> Box<dyn Fn()> {
        self.output
            .attach(|native| self.inner.on_output_error(native), handler)
    }
}

/// The shared manual runtime with a controlled stderr producer.
pub(super) struct RuntimePort {
    /// The unchanged scheduler, future queue and monotonic clock.
    pub(super) inner: Rc<dyn TuiRuntime>,
    /// The stderr callback installed by initialization.
    pub(super) output: Output,
}
impl TuiRuntime for RuntimePort {
    fn now(&self) -> Duration {
        self.inner.now()
    }
    fn schedule(&self, delay: Duration, callback: RenderCallback) -> Box<dyn RenderTimer> {
        self.inner.schedule(delay, callback)
    }
    fn spawn_local(&self, future: LocalFuture) {
        self.inner.spawn_local(future);
    }
    fn environment(&self, key: &str) -> Option<String> {
        self.inner.environment(key)
    }
    fn log_context(&self) -> LogContext {
        self.inner.log_context()
    }
    fn append_log(&self, path: &Path, text: &str) -> io::Result<()> {
        self.inner.append_log(path, text)
    }
    fn write_log(&self, path: &Path, text: &str) -> io::Result<()> {
        self.inner.write_log(path, text)
    }
    fn on_output_error(&self, handler: OutputErrorHandler) -> Box<dyn Fn()> {
        self.output
            .attach(|native| self.inner.on_output_error(native), handler)
    }
}
