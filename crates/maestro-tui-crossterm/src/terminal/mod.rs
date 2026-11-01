//! The process terminal: raw standard input, streamed text input, the reply-driven keyboard
//! negotiation, bracketed paste, progress and the write log. The `unix` and `windows`
//! modules own what each platform acquires, reads and writes; the rest is shared.

mod dimensions;
mod input;
mod lifecycle;
pub mod output;
#[cfg(any(windows, test))]
mod producer;
#[cfg(any(windows, test))]
mod reader;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::time::Duration;

use maestro_tui::Terminal;
use maestro_tui::terminal::OutputErrorHandler;
use tokio::task::{JoinHandle, LocalSet};
use tokio::time::Instant;

use crate::output_errors::{FailureSource, OutputErrors};
use dimensions::Dimensions;
use lifecycle::Running;
use output::{
    CLEAR_FROM_CURSOR, CLEAR_LINE, CLEAR_SCREEN, CURSOR_HIDE, CURSOR_SHOW, PROGRESS_CLEAR,
    append_log, log_destination, relative_move, title,
};
pub use platform::wait_for_input;

/// Where output goes; each platform supplies one, retained across input generations.
trait OutputPort {
    /// Prepares output for a started terminal, with any task it needs on `local`; nothing by
    /// default.
    fn open(&self, _local: &LocalSet, _shared: Weak<Shared>) -> io::Result<()> {
        Ok(())
    }

    /// Writes `text`, or accepts it for the writer.
    fn write(&self, text: &str) -> io::Result<()>;

    /// Clones active queued admission; synchronous destinations have no handle.
    fn output_writer(&self) -> Option<output::OutputWriter> {
        None
    }

    /// Ends the writer, writing what it still holds; nothing by default.
    fn finish(&self) -> io::Result<()> {
        Ok(())
    }

    /// Accepted bytes not yet written; none by default.
    fn pending(&self) -> usize {
        0
    }
}

/// Receives the input the terminal delivers, one framed chunk at a time.
type InputCallback = Box<dyn FnMut(&str)>;

/// Notified when the window size changed.
type ResizeCallback = Box<dyn FnMut()>;

/// A terminal connection over the standard input and output of the process.
///
/// See the [native terminal](crate) guide for the runtime it requires.
pub struct ProcessTerminal {
    /// The set the terminal's tasks run on, driven by the caller.
    local: Rc<LocalSet>,
    /// Where explicit writes are appended, when logging is configured.
    log: Option<PathBuf>,
    /// The state its tasks share with it.
    shared: Rc<Shared>,
    /// The live input task and the standard-input state it changed, while the terminal is
    /// started.
    running: Option<Running>,
    /// The keepalive task of an active progress indicator.
    progress: Option<JoinHandle<()>>,
}

/// State shared between the terminal and the tasks it spawns, none of it behind a guard held
/// while caller code runs.
struct Shared {
    /// Identifies the live input task; changing it retires every earlier task.
    generation: Cell<u64>,
    /// Receives input; absent while the terminal is stopped or draining, and while one call
    /// to it runs.
    on_input: RefCell<Option<InputCallback>>,
    /// Receives window-size changes; absent while the terminal is stopped, and while one call
    /// to it runs.
    on_resize: RefCell<Option<ResizeCallback>>,
    /// Whether the cursor was hidden through this terminal and not shown again.
    hidden: Cell<bool>,
    /// How many keyboard flag requests are not yet popped: one for each start.
    pushes: Cell<usize>,
    /// Whether the terminal reported the enhanced keyboard protocol on.
    enhanced: Cell<bool>,
    /// Whether the modified-key reporting is enabled, because the terminal reported no
    /// enhanced keyboard protocol.
    fallback: Cell<bool>,
    /// Whether a keyboard reply read by the live generation may still change the keyboard
    /// modes; false once a drain has begun.
    negotiating: Cell<bool>,
    /// When decoded input last arrived, whether or not it was delivered.
    activity: Cell<Instant>,
    /// The size of standard output as last observed.
    dimensions: Dimensions,
    /// The first failure of a background task, reported by the next `stop`.
    failure: RefCell<Option<io::Error>>,
    /// The callbacks registered for native output failures.
    errors: Rc<OutputErrors>,
    /// Where output goes: the live writer, retained across input generations, or a
    /// synchronous write.
    output: platform::Output,
    /// Standard byte writes routed through the current output generation.
    stdout: output::StandardOutput,
}

impl Shared {
    /// Writes `text` to the running writer, or synchronously while stopped, and announces an
    /// output failure to the registered handlers once.
    fn emit(&self, text: &str) -> io::Result<()> {
        self.write_output(text)
            .inspect_err(|error| self.errors.notify(error))
    }

    /// Writes `text` to the running writer, or synchronously while stopped, without
    /// announcing a failure; the caller retains and announces the one it gets.
    fn write_output(&self, text: &str) -> io::Result<()> {
        self.output.write(text)
    }

    /// Writes conditional controls through the same output owner.
    fn emit_if(&self, condition: bool, text: &str) -> io::Result<()> {
        if condition { self.emit(text) } else { Ok(()) }
    }

    /// Whether `generation` is the one whose task may still act.
    fn is_current(&self, generation: u64) -> bool {
        self.generation.get() == generation
    }

    /// Retires the live generation and drops its callbacks.
    fn retire(&self) {
        self.generation.set(self.generation.get() + 1);
        let _input = self.on_input.take();
        let _resize = self.on_resize.take();
    }

    /// Retires the live generation and installs the callbacks of a new one, which may enable
    /// the keyboard modes.
    fn install(&self, on_input: InputCallback, on_resize: ResizeCallback) -> u64 {
        self.retire();
        self.negotiating.set(true);
        self.on_input.replace(Some(on_input));
        self.on_resize.replace(Some(on_resize));
        self.generation.get()
    }

    /// Gives `text` to the input callback unless draining, stopped or inside a call to it.
    fn call_input(&self, generation: u64, text: &str) {
        self.call(&self.on_input, generation, |callback| callback(text));
    }

    /// Notifies the resize callback.
    fn call_resize(&self, generation: u64) {
        self.call(&self.on_resize, generation, |callback| callback());
    }

    /// Calls the callback in `slot` with the slot's borrow released, then puts it back when
    /// `generation` is still live.
    fn call<Callback>(
        &self,
        slot: &RefCell<Option<Callback>>,
        generation: u64,
        invoke: impl FnOnce(&mut Callback),
    ) {
        let Some(mut callback) = slot.take() else {
            return;
        };
        invoke(&mut callback);
        if self.is_current(generation) {
            slot.replace(Some(callback));
        }
    }

    /// Keeps the first failure of a background task and announces it when it came from
    /// output or standard error.
    fn fail(&self, source: FailureSource, error: io::Error) {
        self.errors.notify_source(source, &error);
        self.failure.borrow_mut().get_or_insert(error);
    }
}

impl ProcessTerminal {
    /// Returns a sendable byte writer that follows start, stop and subsequent restarts.
    ///
    /// Obtaining or retaining the writer does not start the terminal.
    #[must_use]
    pub fn standard_output_writer(&self) -> output::StandardOutput {
        self.shared.stdout.clone()
    }

    /// Returns the active queued writer, or `None` while stopped or writing synchronously.
    ///
    /// The handle may cross threads; it never shares the terminal's local byte queue.
    #[must_use]
    pub fn output_writer(&self) -> Option<output::OutputWriter> {
        self.running
            .as_ref()
            .and_then(|_| self.shared.output.output_writer())
    }

    /// Creates a terminal that has acquired nothing yet. Tasks run on `local`, which the
    /// caller drives inside a Tokio runtime with the I/O, time and signal drivers enabled.
    #[must_use]
    pub fn new(local: Rc<LocalSet>) -> Self {
        Self {
            local,
            log: log_destination(),
            shared: Rc::new(Shared {
                generation: Cell::new(0),
                on_input: RefCell::new(None),
                on_resize: RefCell::new(None),
                hidden: Cell::new(false),
                pushes: Cell::new(0),
                enhanced: Cell::new(false),
                fallback: Cell::new(false),
                negotiating: Cell::new(false),
                activity: Cell::new(Instant::now()),
                dimensions: Dimensions::observe(),
                failure: RefCell::new(None),
                errors: Rc::new(OutputErrors::default()),
                output: platform::Output::new(),
                stdout: output::StandardOutput::default(),
            }),
            running: None,
            progress: None,
        }
    }
}

impl Terminal for ProcessTerminal {
    fn start(&mut self, on_input: InputCallback, on_resize: ResizeCallback) -> io::Result<()> {
        self.begin(on_input, on_resize)
    }

    fn stop(&mut self) -> io::Result<()> {
        self.end()
    }

    fn drain_input(
        &mut self,
        max: Option<Duration>,
        idle: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + 'static>> {
        self.drain(max, idle)
    }

    fn write(&mut self, data: &str) -> io::Result<()> {
        self.shared.emit(data)?;
        if let Some(log) = &self.log {
            append_log(log, data);
        }
        Ok(())
    }

    fn pending_output_bytes(&self) -> usize {
        self.shared.output.pending()
    }

    fn columns(&self) -> usize {
        self.shared.dimensions.columns()
    }

    fn rows(&self) -> usize {
        self.shared.dimensions.rows()
    }

    fn kitty_protocol_active(&self) -> bool {
        self.shared.enhanced.get()
    }

    fn move_by(&mut self, lines: isize) -> io::Result<()> {
        relative_move(lines).map_or(Ok(()), |sequence| self.shared.emit(&sequence))
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.shared.emit(CURSOR_HIDE)?;
        self.shared.hidden.set(true);
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.shared.emit(CURSOR_SHOW)?;
        self.shared.hidden.set(false);
        Ok(())
    }

    fn clear_line(&mut self) -> io::Result<()> {
        self.shared.emit(CLEAR_LINE)
    }

    fn clear_from_cursor(&mut self) -> io::Result<()> {
        self.shared.emit(CLEAR_FROM_CURSOR)
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        self.shared.emit(CLEAR_SCREEN)
    }

    fn set_title(&mut self, text: &str) -> io::Result<()> {
        self.shared.emit(&title(text))
    }

    fn set_progress(&mut self, active: bool) -> io::Result<()> {
        self.show_progress(active)
    }

    fn on_output_error(&self, handler: OutputErrorHandler) -> Box<dyn Fn()> {
        self.shared.errors.register(handler)
    }
}

impl Drop for ProcessTerminal {
    fn drop(&mut self) {
        if self.running.is_some() {
            let _ = self.end();
        } else {
            let _ = self.shared.show_hidden_cursor();
            if self.cancel_keepalive() {
                let _ = self.shared.emit(PROGRESS_CLEAR);
            }
        }
    }
}
