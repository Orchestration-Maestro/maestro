//! The Unix process terminal: raw standard input, streamed text input, the enhanced keyboard
//! negotiation, bracketed paste, progress and the write log.

mod dimensions;
mod input;
mod lifecycle;
mod output;
mod string_decoder;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use maestro_tui::Terminal;
use tokio::task::{JoinHandle, LocalSet};
use tokio::time::Instant;

use dimensions::Dimensions;
use lifecycle::Running;
use output::{
    CLEAR_FROM_CURSOR, CLEAR_LINE, CLEAR_SCREEN, CURSOR_HIDE, CURSOR_SHOW, PROGRESS_CLEAR,
    append_log, emit, log_destination, relative_move, title,
};

/// Receives the input the terminal delivers, one framed chunk at a time.
type InputCallback = Box<dyn FnMut(&str)>;

/// Notified when the window size changed.
type ResizeCallback = Box<dyn FnMut()>;

/// A Unix terminal connection over the standard descriptors of the process.
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
    /// Whether the enhanced keyboard protocol is enabled, after the terminal reported support.
    enhanced: Cell<bool>,
    /// Whether the modified-key reporting is enabled, because no support report arrived in time.
    fallback: Cell<bool>,
    /// Whether a support reply read by the live generation may still enable the enhanced
    /// keyboard modes; false once a drain has begun.
    negotiating: Cell<bool>,
    /// When decoded input last arrived, whether or not it was delivered.
    activity: Cell<Instant>,
    /// The size of standard output as last observed.
    dimensions: Dimensions,
    /// The first failure of a background task, reported by the next `stop`.
    failure: RefCell<Option<io::Error>>,
}

impl Shared {
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

    /// Keeps the first failure of a background task.
    fn fail(&self, error: io::Error) {
        self.failure.borrow_mut().get_or_insert(error);
    }
}

impl ProcessTerminal {
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
                enhanced: Cell::new(false),
                fallback: Cell::new(false),
                negotiating: Cell::new(false),
                activity: Cell::new(Instant::now()),
                dimensions: Dimensions::observe(),
                failure: RefCell::new(None),
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
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + '_>> {
        self.drain(max, idle)
    }

    fn write(&mut self, data: &str) -> io::Result<()> {
        emit(data)?;
        if let Some(log) = &self.log {
            append_log(log, data);
        }
        Ok(())
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
        relative_move(lines).map_or(Ok(()), |sequence| emit(&sequence))
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        emit(CURSOR_HIDE)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        emit(CURSOR_SHOW)
    }

    fn clear_line(&mut self) -> io::Result<()> {
        emit(CLEAR_LINE)
    }

    fn clear_from_cursor(&mut self) -> io::Result<()> {
        emit(CLEAR_FROM_CURSOR)
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        emit(CLEAR_SCREEN)
    }

    fn set_title(&mut self, text: &str) -> io::Result<()> {
        emit(&title(text))
    }

    fn set_progress(&mut self, active: bool) -> io::Result<()> {
        self.show_progress(active)
    }
}

impl Drop for ProcessTerminal {
    fn drop(&mut self) {
        if self.running.is_some() {
            let _ = self.end();
        } else if self.cancel_keepalive() {
            let _ = emit(PROGRESS_CLEAR);
        }
    }
}
