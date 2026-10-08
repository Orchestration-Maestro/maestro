//! The input task of one started generation: it reads standard input, decodes it, frames it
//! with the buffer, and delivers the events to the callbacks.

use std::fs::File;
use std::future::pending;
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::rc::Rc;

use maestro_tui::{StdinBuffer, StdinBufferEventMap, StdinBufferInput, StdinBufferOptions};
use rustix::stdio::stdin;
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;
use tokio::signal::unix::Signal;
use tokio::time::{Instant, sleep_until};

use super::Shared;
use super::string_decoder::Utf8Stream;

/// How many bytes one read takes.
const READ_SIZE: usize = 4096;

/// Where the bytes of standard input come from.
pub(super) enum Source {
    /// A descriptor the reactor can wait on: a terminal, pipe or socket.
    Reactor(AsyncFd<OwnedFd>),
    /// A descriptor the reactor refuses with a permission error, such as a regular file or
    /// `/dev/null`, which a read does not wait on.
    File(File),
}

impl Source {
    /// Opens a duplicate of standard input for reading.
    pub(super) fn open() -> io::Result<Self> {
        let duplicate = rustix::io::dup(stdin())?;
        match AsyncFd::try_with_interest(duplicate, Interest::READABLE) {
            Ok(descriptor) => Ok(Self::Reactor(descriptor)),
            Err(refused) => match refused.into_parts() {
                (descriptor, error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    Ok(Self::File(File::from(descriptor)))
                }
                (_, error) => Err(error),
            },
        }
    }

    /// Reads the next available bytes; `Ok(0)` is the end of input.
    async fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Reactor(descriptor) => loop {
                let mut ready = descriptor.readable().await?;
                let attempt = ready.try_io(|inner| {
                    rustix::io::read(inner.get_ref(), &mut *buffer).map_err(io::Error::from)
                });
                if let Ok(result) = attempt {
                    return result;
                }
            },
            Self::File(file) => {
                tokio::task::yield_now().await;
                file.read(buffer)
            }
        }
    }
}

/// What woke the input task.
enum Wake {
    /// A read finished.
    Read(io::Result<usize>),
    /// The window-change signal arrived.
    Resize,
    /// A deadline passed.
    Deadline,
}

/// The state of one input task.
pub(super) struct Input {
    /// The terminal state it shares.
    shared: Rc<Shared>,
    /// The generation it serves; it stops acting once another is current.
    generation: u64,
    /// Frames decoded text into events.
    buffer: StdinBuffer,
    /// Completes characters split across reads.
    decoder: Utf8Stream,
    /// Whether reads continue; they stop at the end of input.
    reading: bool,
    /// When a missing keyboard reply selects the fallback mode, until it is decided.
    negotiate_at: Option<Instant>,
}

impl Input {
    /// Creates the state for the task of `generation`, which decides the keyboard fallback at
    /// `negotiate_at`.
    pub(super) fn new(shared: Rc<Shared>, generation: u64, negotiate_at: Instant) -> Self {
        Self {
            shared,
            generation,
            buffer: StdinBuffer::new(StdinBufferOptions::default()),
            decoder: Utf8Stream::default(),
            reading: true,
            negotiate_at: Some(negotiate_at),
        }
    }

    /// Reads, frames and delivers input until the generation ends or a failure occurs.
    pub(super) async fn run(mut self, mut source: Source, mut resize: Signal) {
        let mut chunk = [0; READ_SIZE];
        if self.shared.dimensions.refresh() {
            self.shared.call_resize(self.generation);
        }
        while self.is_current() {
            let deadline = self.next_deadline();
            let wake = tokio::select! {
                read = source.read(&mut chunk), if self.reading => Wake::Read(read),
                Some(()) = resize.recv() => Wake::Resize,
                () = until(deadline) => Wake::Deadline,
            };
            if let Err(error) = self.handle(wake, &chunk) {
                return self.shared.fail(error);
            }
        }
    }

    /// Whether this task's generation is still the live one.
    fn is_current(&self) -> bool {
        self.shared.is_current(self.generation)
    }

    /// The earlier of the buffer deadline and the pending keyboard decision.
    fn next_deadline(&self) -> Option<Instant> {
        let buffered = self.buffer.deadline().map(Instant::from_std);
        [buffered, self.negotiate_at].into_iter().flatten().min()
    }

    /// Acts on what woke the task.
    fn handle(&mut self, wake: Wake, chunk: &[u8]) -> io::Result<()> {
        match wake {
            Wake::Read(read) => match read? {
                0 => {
                    self.reading = false;
                    let rest = self.decoder.finish();
                    self.text(&rest)
                }
                count => {
                    let text = self.decoder.push(&chunk[..count]);
                    self.text(&text)
                }
            },
            Wake::Resize => {
                if self.shared.dimensions.refresh() {
                    self.shared.call_resize(self.generation);
                }
                Ok(())
            }
            Wake::Deadline => self.expire(),
        }
    }

    /// Frames decoded text; empty text is not input.
    fn text(&mut self, text: &str) -> io::Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        let now = Instant::now();
        self.shared.activity.set(now);
        let events = self
            .buffer
            .process(StdinBufferInput::Text(text), now.into_std());
        self.dispatch(events)
    }

    /// Decides the keyboard fallback and releases the buffered fragment, each when due.
    fn expire(&mut self) -> io::Result<()> {
        let now = Instant::now();
        if self.negotiate_at.is_some_and(|at| at <= now) {
            self.negotiate_at = None;
            self.shared.fall_back()?;
        }
        let events = self.buffer.expire(now.into_std());
        self.dispatch(events)
    }

    /// Delivers events in order, stopping once the generation ends.
    fn dispatch(&self, events: Vec<StdinBufferEventMap>) -> io::Result<()> {
        for event in events {
            if !self.is_current() {
                break;
            }
            if let Some(text) = self.input_of(event)? {
                self.shared.call_input(self.generation, &text);
            }
        }
        Ok(())
    }

    /// The input `event` stands for, or `None` when it is a keyboard reply. A paste gets its
    /// markers back.
    fn input_of(&self, event: StdinBufferEventMap) -> io::Result<Option<String>> {
        Ok(match event {
            StdinBufferEventMap::Data(sequence) if self.shared.consume_reply(&sequence)? => None,
            StdinBufferEventMap::Data(sequence) => Some(sequence),
            StdinBufferEventMap::Paste(content) => Some(format!("\x1b[200~{content}\x1b[201~")),
        })
    }
}

/// Completes at `deadline`, or never when there is none.
async fn until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => pending().await,
    }
}
