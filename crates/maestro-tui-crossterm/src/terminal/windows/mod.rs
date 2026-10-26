//! The Windows parts of the process terminal: the console modes it acquires, the record
//! reader of an input task, synchronous output and the size of the console window.

use std::future::pending;
use std::io::{self, Write};

use crossterm_winapi::{
    Console, ConsoleMode, Handle as ConsoleHandle, HandleType, InputRecord, ScreenBuffer,
};
use tokio::runtime::Handle;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio::task::{JoinHandle, LocalSet};

use super::OutputPort;
use super::input::{Input, Wake, until};
use super::producer::{Arrival, Producer, Record, Records};

/// Ctrl+C and the other control keys are handled by the console.
const ENABLE_PROCESSED_INPUT: u32 = 0x0001;

/// Reads return when Enter is pressed.
const ENABLE_LINE_INPUT: u32 = 0x0002;

/// Typed characters are echoed.
const ENABLE_ECHO_INPUT: u32 = 0x0004;

/// Screen-buffer size changes are reported as input records.
const ENABLE_WINDOW_INPUT: u32 = 0x0008;

/// Keys arrive as the escape sequences a terminal sends, so modified keys such as Shift+Tab
/// are told apart.
const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;

/// Escape sequences written to the console are interpreted rather than shown.
const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

/// The live input task of a generation and the thread reading for it.
pub(super) struct Task {
    /// The task delivering input.
    task: JoinHandle<()>,
    /// The record reader, absent when standard input is not a console.
    producer: Option<Producer>,
}

impl Task {
    /// Stops and joins the reader, so no record is read after this returns, then cancels the
    /// task.
    pub(super) fn abort(self) {
        drop(self.producer);
        self.task.abort();
    }
}

/// The console modes a started terminal changed and must give back.
#[derive(Default)]
pub(super) struct Acquired {
    /// The standard-input console and its mode before raw mode, when it is a console.
    input: Option<(ConsoleHandle, u32)>,
    /// The standard-output console and its mode before escape-sequence processing, when it
    /// is a console that accepted the change.
    output: Option<(ConsoleHandle, u32)>,
}

impl Acquired {
    /// Puts the standard-input console in raw mode with window-size records, then adds
    /// escape-sequence input. A console that refuses escape-sequence input stays in raw mode,
    /// and standard input that is not a console is left alone. A remembered mode is kept, so
    /// repeating the call never records the changed mode as the original.
    fn take_input(&mut self) -> io::Result<()> {
        if self.input.is_some() {
            return Ok(());
        }
        let Some((handle, original)) = console_mode(HandleType::InputHandle) else {
            return Ok(());
        };
        let mode = ConsoleMode::from(handle.clone());
        let raw = (original & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT))
            | ENABLE_WINDOW_INPUT;
        mode.set_mode(raw)?;
        self.input = Some((handle, original));
        let _ = mode.set_mode(raw | ENABLE_VIRTUAL_TERMINAL_INPUT);
        Ok(())
    }

    /// Acquires standard input, then turns on escape-sequence processing for the
    /// standard-output console when it accepts it.
    fn take(&mut self) -> io::Result<()> {
        self.take_input()?;
        if self.output.is_none()
            && let Some((handle, original)) = console_mode(HandleType::OutputHandle)
            && ConsoleMode::from(handle.clone())
                .set_mode(original | ENABLE_VIRTUAL_TERMINAL_PROCESSING)
                .is_ok()
        {
            self.output = Some((handle, original));
        }
        Ok(())
    }

    /// Gives back both saved modes, attempting output even when input fails, and returns the
    /// first failure.
    pub(super) fn restore(&mut self) -> io::Result<()> {
        let restore = |saved: Option<(ConsoleHandle, u32)>| {
            saved.map_or(Ok(()), |(handle, original)| {
                ConsoleMode::from(handle).set_mode(original)
            })
        };
        let input = restore(self.input.take());
        let output = restore(self.output.take());
        input.and(output)
    }

    /// The standard-input console, when raw mode was applied to one.
    fn console(&self) -> Option<Console> {
        self.input
            .as_ref()
            .map(|(handle, _)| Console::from(handle.clone()))
    }
}

impl Drop for Acquired {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// The standard handle of `kind` and its console mode, or `None` when it is not a console.
fn console_mode(kind: HandleType) -> Option<(ConsoleHandle, u32)> {
    let handle = ConsoleHandle::new(kind).ok()?;
    let mode = ConsoleMode::from(handle.clone()).mode().ok()?;
    Some((handle, mode))
}

impl Records for Console {
    fn pending(&self) -> io::Result<u32> {
        self.number_of_console_input_events()
    }

    fn read(&self) -> io::Result<Vec<Record>> {
        Ok(self
            .read_console_input()?
            .into_iter()
            .map(|record| match record {
                InputRecord::KeyEvent(key) => Record::Key {
                    down: key.key_down,
                    key: key.virtual_key_code,
                    unit: key.u_char,
                    repeat: key.repeat_count,
                },
                InputRecord::WindowBufferSizeEvent(_) => Record::Resize,
                _ => Record::Other,
            })
            .collect())
    }
}

/// The reader of a generation that is not spawned yet.
pub(super) struct Stream {
    /// The record reader, absent when standard input is not a console.
    producer: Option<Producer>,
    /// What the reader delivers; closed at once when there is no reader.
    arrivals: UnboundedReceiver<Arrival>,
}

/// Acquires the consoles, then starts the record reader when standard input is a console.
pub(super) fn acquire(acquired: &mut Acquired) -> io::Result<Stream> {
    acquired.take()?;
    open(acquired)
}

/// Starts the record reader on the standard-input console `acquired` holds, if any.
fn open(acquired: &Acquired) -> io::Result<Stream> {
    let (sender, arrivals) = unbounded_channel();
    let producer = acquired
        .console()
        .map(|console| Producer::spawn(console, sender))
        .transpose()?;
    Ok(Stream { producer, arrivals })
}

impl Stream {
    /// Spawns the task that delivers for `input` on `local`.
    pub(super) fn spawn(self, local: &LocalSet, input: Input) -> Task {
        Task {
            task: local.spawn_local(run(input, self.arrivals)),
            producer: self.producer,
        }
    }
}

/// Frames and delivers what the reader sends until the generation ends or a failure occurs.
/// A closed reader is the end of input.
async fn run(mut input: Input, mut arrivals: UnboundedReceiver<Arrival>) {
    input.resized();
    while input.is_current() {
        let deadline = input.next_deadline();
        let (wake, bytes) = tokio::select! {
            arrival = arrivals.recv(), if input.reading() => woken(arrival),
            () = until(deadline) => (Wake::Deadline, Vec::new()),
        };
        if let Err(error) = input.handle(wake, &bytes) {
            return input.fail(error);
        }
    }
}

/// What `arrival` wakes the task with, and the bytes it carries.
fn woken(arrival: Option<Arrival>) -> (Wake, Vec<u8>) {
    match arrival {
        Some(Arrival::Bytes(bytes)) => (Wake::Read(Ok(bytes.len())), bytes),
        Some(Arrival::Resize) => (Wake::Resize, Vec::new()),
        Some(Arrival::Failed(error)) => (Wake::Read(Err(error)), Vec::new()),
        None => (Wake::Read(Ok(0)), Vec::new()),
    }
}

/// Output to the console, which accepts each write before it returns, so nothing is
/// retained, opened or drained.
#[derive(Default)]
pub(super) struct Output;

impl OutputPort for Output {
    fn write(&self, text: &str) -> io::Result<()> {
        let mut stdout = io::stdout().lock();
        stdout.write_all(text.as_bytes())?;
        stdout.flush()
    }
}

/// The columns and rows of the console window standard output shows, or `None` when standard
/// output is not a console.
pub(super) fn window_size() -> Option<(usize, usize)> {
    let handle = ConsoleHandle::new(HandleType::OutputHandle).ok()?;
    let window = ScreenBuffer::new(handle).info().ok()?.terminal_window();
    let extent = |low: i16, high: i16| usize::try_from(i32::from(high) - i32::from(low) + 1).ok();
    Some((
        extent(window.left, window.right)?,
        extent(window.top, window.bottom)?,
    ))
}

/// Await one nonempty raw console read without starting a terminal or enabling output modes.
/// Acquires the standard-input console on first poll and restores its saved mode on
/// completion or drop. Standard input that is not a console remains pending until dropped.
/// Requires a caller-driven Tokio runtime.
///
/// # Errors
/// Returns runtime, acquisition, reader or restoration errors, preserving an earlier
/// operation error when restoration also fails.
pub async fn wait_for_input() -> io::Result<()> {
    Handle::try_current().map_err(io::Error::other)?;
    let mut acquired = Acquired::default();
    let result = async {
        acquired.take_input()?;
        let mut stream = open(&acquired)?;
        loop {
            match stream.arrivals.recv().await {
                Some(Arrival::Bytes(_)) => return Ok(()),
                Some(Arrival::Failed(error)) => return Err(error),
                Some(Arrival::Resize) => {}
                None => pending::<()>().await,
            }
        }
    }
    .await;
    let restored = acquired.restore();
    result.and(restored)
}
