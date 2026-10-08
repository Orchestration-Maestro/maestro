//! A terminal that records every effect the frame writer asks of it.

use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use maestro_tui::Terminal;
use maestro_tui::tui::TerminalHandle;

/// Callback that receives the bytes of each recorded effect.
type Tap = Box<dyn FnMut(&str)>;
/// Input callback shared so it can run while no state borrow is held.
type InputCallback = Rc<RefCell<Box<dyn FnMut(&str)>>>;
/// Resize callback shared so it can run while no state borrow is held.
type ResizeCallback = Rc<RefCell<Box<dyn FnMut()>>>;

/// What the terminal has been asked to do and how it is configured.
struct State {
    /// Width in columns.
    columns: usize,
    /// Height in rows.
    rows: usize,
    /// Writes and cursor visibility escapes in the order requested.
    writes: Vec<String>,
    /// Lifecycle events in the order requested.
    events: Vec<&'static str>,
    /// Receiver of input while started.
    on_input: Option<InputCallback>,
    /// Receiver of resize notices while started.
    on_resize: Option<ResizeCallback>,
    /// Observer of every recorded byte string.
    tap: Option<Tap>,
}

/// Shared handle to a recording terminal; clones observe the same record.
#[derive(Clone)]
pub struct RecordingTerminal {
    /// State shared by every clone.
    state: Rc<RefCell<State>>,
}

impl RecordingTerminal {
    /// Creates a terminal of the given size that has recorded nothing.
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            state: Rc::new(RefCell::new(State {
                columns,
                rows,
                writes: Vec::new(),
                events: Vec::new(),
                on_input: None,
                on_resize: None,
                tap: None,
            })),
        }
    }

    /// The terminal as the writer receives it.
    pub fn handle(&self) -> TerminalHandle {
        Rc::new(RefCell::new(self.clone()))
    }

    /// Passes every byte string recorded from now on to `tap`.
    pub fn set_tap(&self, tap: impl FnMut(&str) + 'static) {
        self.state.borrow_mut().tap = Some(Box::new(tap));
    }

    /// Writes and cursor visibility escapes in the order requested.
    pub fn writes(&self) -> Vec<String> {
        self.state.borrow().writes.clone()
    }

    /// Forgets the writes recorded so far.
    pub fn clear_writes(&self) {
        self.state.borrow_mut().writes.clear();
    }

    /// Lifecycle events (`start`, `stop`) in the order requested.
    pub fn events(&self) -> Vec<&'static str> {
        self.state.borrow().events.clone()
    }

    /// Delivers input to the started frame writer; a stopped terminal drops it.
    pub fn send_input(&self, data: &str) {
        let callback = self.state.borrow().on_input.clone();
        if let Some(callback) = callback {
            (callback.borrow_mut())(data);
        }
    }

    /// Changes the size and notifies the started frame writer.
    pub fn resize(&self, columns: usize, rows: usize) {
        {
            let mut state = self.state.borrow_mut();
            state.columns = columns;
            state.rows = rows;
        }
        self.notify_resize();
    }

    /// The current size as `(columns, rows)`.
    pub fn size(&self) -> (usize, usize) {
        let state = self.state.borrow();
        (state.columns, state.rows)
    }

    /// Notifies the started frame writer without changing the size.
    pub fn notify_resize(&self) {
        let callback = self.state.borrow().on_resize.clone();
        if let Some(callback) = callback {
            (callback.borrow_mut())();
        }
    }

    /// Records `data` and passes it to the tap without holding the state borrow.
    fn record(&self, data: &str) {
        let tap = {
            let mut state = self.state.borrow_mut();
            state.writes.push(data.to_owned());
            state.tap.take()
        };
        if let Some(mut tap) = tap {
            tap(data);
            self.state.borrow_mut().tap = Some(tap);
        }
    }

    /// Passes `data` to the tap without recording it as a write.
    pub fn emulate(&self, data: &str) {
        let tap = self.state.borrow_mut().tap.take();
        if let Some(mut tap) = tap {
            tap(data);
            self.state.borrow_mut().tap = Some(tap);
        }
    }

    /// Stores the callbacks and records the start event.
    fn begin(&self, on_input: Box<dyn FnMut(&str)>, on_resize: Box<dyn FnMut()>) {
        let mut state = self.state.borrow_mut();
        state.events.push("start");
        state.on_input = Some(Rc::new(RefCell::new(on_input)));
        state.on_resize = Some(Rc::new(RefCell::new(on_resize)));
    }

    /// Drops the callbacks and records the stop event.
    fn end(&self) {
        let mut state = self.state.borrow_mut();
        state.events.push("stop");
        state.on_input = None;
        state.on_resize = None;
    }
}

impl Terminal for RecordingTerminal {
    fn start(
        &mut self,
        on_input: Box<dyn FnMut(&str)>,
        on_resize: Box<dyn FnMut()>,
    ) -> io::Result<()> {
        self.begin(on_input, on_resize);
        Ok(())
    }

    fn stop(&mut self) -> io::Result<()> {
        self.end();
        Ok(())
    }

    fn drain_input(
        &mut self,
        _max: Option<Duration>,
        _idle: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn write(&mut self, data: &str) -> io::Result<()> {
        self.record(data);
        Ok(())
    }

    fn columns(&self) -> usize {
        self.state.borrow().columns
    }

    fn rows(&self) -> usize {
        self.state.borrow().rows
    }

    fn kitty_protocol_active(&self) -> bool {
        true
    }

    fn move_by(&mut self, lines: isize) -> io::Result<()> {
        match lines.cmp(&0) {
            std::cmp::Ordering::Greater => self.emulate(&format!("\x1b[{lines}B")),
            std::cmp::Ordering::Less => self.emulate(&format!("\x1b[{}A", -lines)),
            std::cmp::Ordering::Equal => {}
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.record("\x1b[?25l");
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.record("\x1b[?25h");
        Ok(())
    }

    fn clear_line(&mut self) -> io::Result<()> {
        self.emulate("\x1b[K");
        Ok(())
    }

    fn clear_from_cursor(&mut self) -> io::Result<()> {
        self.emulate("\x1b[J");
        Ok(())
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        self.emulate("\x1b[2J\x1b[H");
        Ok(())
    }

    fn set_title(&mut self, title: &str) -> io::Result<()> {
        self.emulate(&format!("\x1b]0;{title}\x07"));
        Ok(())
    }

    fn set_progress(&mut self, _active: bool) -> io::Result<()> {
        Ok(())
    }
}
