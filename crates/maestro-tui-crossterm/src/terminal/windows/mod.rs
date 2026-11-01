//! The Windows parts of the process terminal: the console modes it acquires, the record
//! reader of an input task, synchronous output and the size of the console window.

use std::future::pending;
use std::io::{self, Write};
use std::sync::Mutex;

use crossterm_winapi::{
    Console, ConsoleMode, ControlKeyState, Handle as ConsoleHandle, HandleType, InputRecord,
    ScreenBuffer,
};
use tokio::runtime::Handle;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio::task::{JoinHandle, LocalSet};

use super::OutputPort;
use super::input::{Input, Wake, until};
use super::producer::{self, Arrival, Record, Records};
use super::reader::{Reader, Subscription};

/// Standard input has one reader shared by terminals and standalone waits.
static STDIN: Mutex<InputOwner> = Mutex::new(InputOwner {
    reader: None,
    mode: None,
    users: 0,
});

/// The single standard-input reader and the raw-mode acquisition its subscribers share.
struct InputOwner {
    /// Retained across terminal generations and standalone waits.
    reader: Option<Reader>,
    /// The console and original mode, while at least one acquisition is live.
    mode: Option<(ConsoleHandle, u32)>,
    /// How many acquisitions still need raw input.
    users: usize,
}

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

/// The live input task of a generation and its reader subscription.
pub(super) struct Task {
    /// The task delivering input.
    task: JoinHandle<()>,
    /// Its live subscription to standard input.
    subscription: Subscription,
}

impl Task {
    /// Stops forwarding input to this generation, then cancels its task.
    pub(super) fn abort(self) {
        drop(self.subscription);
        self.task.abort();
    }
}

/// The console modes a started terminal changed and must give back.
#[derive(Default)]
pub(super) struct Acquired {
    /// Whether this acquisition holds a share of standard input.
    input: bool,
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
        if self.input {
            return Ok(());
        }
        let mut owner = STDIN
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if owner.users == 0 {
            owner.take_mode()?;
        }
        owner.users += 1;
        self.input = true;
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
        let input = if std::mem::take(&mut self.input) {
            let mut owner = STDIN
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            owner.users -= 1;
            if owner.users == 0 {
                restore(owner.mode.take())
            } else {
                Ok(())
            }
        } else {
            Ok(())
        };
        let output = restore(self.output.take());
        input.and(output)
    }
}

impl InputOwner {
    /// Saves and acquires raw console input for the first live user; pipes need no mode.
    fn take_mode(&mut self) -> io::Result<()> {
        let Some((handle, original)) = console_mode(HandleType::InputHandle) else {
            return Ok(());
        };
        let mode = ConsoleMode::from(handle.clone());
        let raw = (original & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT))
            | ENABLE_WINDOW_INPUT;
        mode.set_mode(raw)?;
        self.mode = Some((handle, original));
        let _ = mode.set_mode(raw | ENABLE_VIRTUAL_TERMINAL_INPUT);
        Ok(())
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
                    control: control_bits(key.control_key_state),
                },
                InputRecord::WindowBufferSizeEvent(_) => Record::Resize,
                _ => Record::Other,
            })
            .collect())
    }
}

/// The bits of the console control-state mask, whose safe wrapper exposes only queries.
fn control_bits(control: ControlKeyState) -> u32 {
    (0..u32::BITS)
        .map(|shift| 1 << shift)
        .filter(|bit| control.has_state(*bit))
        .fold(0, |mask, bit| mask | bit)
}

/// The reader of a generation that is not spawned yet.
pub(super) struct Stream {
    /// Its live subscription to standard input.
    subscription: Subscription,
    /// What the shared reader delivers to this generation.
    arrivals: UnboundedReceiver<Arrival>,
}

/// Acquires console modes and subscribes to standard input.
pub(super) fn acquire(acquired: &mut Acquired) -> io::Result<Stream> {
    acquired.take()?;
    open()
}

/// Opens one subscription to the process-wide console or redirected stream reader.
fn open() -> io::Result<Stream> {
    let (sender, arrivals) = unbounded_channel();
    let mut owner = STDIN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if owner.reader.is_none() {
        owner.reader = Some(match &owner.mode {
            Some((handle, _)) => Reader::spawn(producer::console(Console::from(handle.clone())))?,
            None => Reader::spawn(producer::pipe(io::stdin()))?,
        });
    }
    let subscription = owner
        .reader
        .as_ref()
        .ok_or_else(|| io::Error::other("input reader missing"))?
        .subscribe(sender);
    Ok(Stream {
        subscription,
        arrivals,
    })
}

impl Stream {
    /// Spawns the task that delivers for `input` on `local`.
    pub(super) fn spawn(self, local: &LocalSet, input: Input) -> Task {
        Task {
            task: local.spawn_local(run(input, self.arrivals)),
            subscription: self.subscription,
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
pub(super) struct Output;

impl Output {
    /// Output to the console.
    pub(super) fn new() -> Self {
        Self
    }
}

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

/// Await one nonempty raw standard-input read without starting a terminal or enabling output modes.
/// Acquires the standard-input console on first poll and restores its saved mode on
/// completion or drop. Redirected input is read as bytes; EOF stays pending until dropped.
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
        let mut stream = open()?;
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
