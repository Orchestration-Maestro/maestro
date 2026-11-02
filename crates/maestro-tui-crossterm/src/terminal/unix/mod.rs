//! The Unix parts of the process terminal: the standard-input attributes and flags it
//! acquires, the descriptor reader and window-change signal of an input task, the output
//! writer and the size of standard output.

mod descriptor;
mod tty_writer;

use std::cell::RefCell;
use std::io;
use std::rc::{Rc, Weak};

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::stdio::{stdin, stdout};
use rustix::termios::{
    InputModes, OptionalActions, OutputModes, Termios, tcgetattr, tcgetwinsize, tcsetattr,
};
use tokio::io::Interest;
use tokio::runtime::Handle;
use tokio::signal::unix::{Signal, SignalKind, signal};
use tokio::task::{JoinHandle, LocalSet};

use super::input::{Input, Wake, until};
use super::{OutputPort, Shared};
use descriptor::Source;
use tty_writer::TtyWriter;

/// The most bytes one read takes.
const READ_SIZE: usize = 4096;

/// The live input task of a generation.
pub(super) type Task = JoinHandle<()>;

/// The standard-input state a started terminal changed and must give back.
#[derive(Default)]
pub(super) struct Acquired {
    /// The attributes before raw mode, when standard input is a terminal.
    attributes: Option<Termios>,
    /// The status flags before reads became nonblocking.
    flags: Option<OFlags>,
}

impl Acquired {
    /// Puts standard input in raw mode, when it is a terminal, and makes its reads
    /// nonblocking. What is already remembered is kept, so repeating the call never records
    /// the changed state as the original.
    fn take(&mut self) -> io::Result<()> {
        if self.attributes.is_none()
            && let Ok(original) = tcgetattr(stdin())
        {
            tcsetattr(stdin(), OptionalActions::Drain, &raw(&original))?;
            self.attributes = Some(original);
        }
        if self.flags.is_none() {
            let flags = fcntl_getfl(stdin())?;
            fcntl_setfl(stdin(), flags | OFlags::NONBLOCK)?;
            self.flags = Some(flags);
        }
        Ok(())
    }

    /// Gives back what was taken, attempting the flags even when the attributes fail, and
    /// returns the first failure.
    pub(super) fn restore(&mut self) -> io::Result<()> {
        let attributes = self.attributes.take().map_or(Ok(()), |original| {
            tcsetattr(stdin(), OptionalActions::Drain, &original).map_err(io::Error::from)
        });
        let flags = self.flags.take().map_or(Ok(()), |original| {
            fcntl_setfl(stdin(), original).map_err(io::Error::from)
        });
        attributes.and(flags)
    }
}

impl Drop for Acquired {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// `original` in raw mode: no echo, line editing, signal keys, break, parity, strip, newline,
/// carriage-return or start/stop-key handling on input, reads that return as soon as one byte
/// is available, and output post-processing with newline-to-CR-LF mapping on so a bare
/// newline written to the device returns the carriage. Some C libraries' raw mode ignores
/// a break instead, so that flag is cleared here and a break reads the same on every host.
fn raw(original: &Termios) -> Termios {
    let mut raw = original.clone();
    raw.make_raw();
    raw.input_modes.remove(InputModes::IGNBRK);
    raw.output_modes
        .insert(OutputModes::OPOST | OutputModes::ONLCR);
    raw
}

/// The reader and window-change subscription of a generation that is not spawned yet.
pub(super) struct Stream {
    /// Standard input, registered for reading.
    source: Source,
    /// The window-change signal.
    resize: Signal,
}

/// Acquires standard input, then opens its reader and subscribes to window changes.
pub(super) fn acquire(acquired: &mut Acquired) -> io::Result<Stream> {
    acquired.take()?;
    Ok(Stream {
        source: Source::open(stdin(), Interest::READABLE)?,
        resize: signal(SignalKind::window_change())?,
    })
}

impl Stream {
    /// Spawns the task that reads for `input` on `local`.
    pub(super) fn spawn(self, local: &LocalSet, input: Input) -> Task {
        local.spawn_local(run(input, self.source, self.resize))
    }
}

/// Reads, frames and delivers input until the generation ends or a failure occurs.
async fn run(mut input: Input, mut source: Source, mut resize: Signal) {
    let mut chunk = [0; READ_SIZE];
    input.resized();
    while input.is_current() {
        let deadline = input.next_deadline();
        let wake = tokio::select! {
            read = source.read(&mut chunk), if input.reading() => Wake::Read(read),
            Some(()) = resize.recv() => Wake::Resize,
            () = until(deadline) => Wake::Deadline,
        };
        if let Err(error) = input.handle(wake, &chunk) {
            return input.fail(error);
        }
    }
}

/// The one live output writer, retained across input generations.
pub(super) struct Output(RefCell<Option<Rc<TtyWriter>>>);

impl Output {
    /// Output with no writer yet.
    pub(super) fn new() -> Self {
        Self(RefCell::new(None))
    }
}

impl OutputPort for Output {
    /// Registers nonblocking output with a writer task on `local`, unless one is running or
    /// standard output is a destination the reactor refuses.
    fn open(&self, local: &LocalSet, shared: Weak<Shared>) -> io::Result<()> {
        if self.0.borrow().is_none() {
            let writer = TtyWriter::new(local, shared)?;
            self.0.replace(writer.map(Rc::new));
        }
        Ok(())
    }

    /// Queues `text` on the running writer, or writes it synchronously while none runs.
    fn write(&self, text: &str) -> io::Result<()> {
        let writer = self.0.borrow().clone();
        if let Some(writer) = writer {
            writer.write(text)
        } else {
            tty_writer::write_sync(text)
        }
    }

    /// Keeps only the sendable ingress in the caller's handle.
    fn output_writer(&self) -> Option<crate::terminal::output::OutputWriter> {
        self.0
            .borrow()
            .as_ref()
            .map(|writer| writer.output_writer())
    }

    /// Takes the writer out of service before draining any retained front.
    fn finish(&self) -> io::Result<()> {
        let writer = self.0.take();
        writer.map_or(Ok(()), |writer| writer.finish())
    }

    /// Accepted bytes not yet written.
    fn pending(&self) -> usize {
        self.0
            .borrow()
            .as_ref()
            .map_or(0, |writer| writer.pending())
    }
}

/// The columns and rows of standard output, or `None` when it reports no size.
pub(super) fn window_size() -> Option<(usize, usize)> {
    let size = tcgetwinsize(stdout()).ok()?;
    Some((usize::from(size.ws_col), usize::from(size.ws_row)))
}

/// Await one nonempty raw stdin read without starting a terminal or enabling output modes.
/// Acquires on first poll and restores saved attributes and descriptor flags on completion
/// or drop. EOF remains pending until dropped. Requires a caller-driven Tokio I/O runtime.
///
/// # Errors
/// Returns runtime, acquisition, open, read or restoration errors, preserving an earlier
/// operation error when restoration also fails.
pub async fn wait_for_input() -> io::Result<()> {
    Handle::try_current().map_err(io::Error::other)?;
    let mut source = Source::open(stdin(), Interest::READABLE)?;
    let mut acquired = Acquired::default();
    let result = async {
        acquired.take()?;
        let mut chunk = [0; READ_SIZE];
        if source.read(&mut chunk).await? == 0 {
            std::future::pending::<()>().await;
        }
        Ok(())
    }
    .await;
    let restored = acquired.restore();
    result.and(restored)
}
