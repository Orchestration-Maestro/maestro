//! The input state of one started generation: it decodes what the platform reader
//! delivers, frames it with the buffer, and delivers the events to the callbacks.

use std::future::pending;
use std::io;
use std::rc::Rc;

use maestro_tui::{StdinBuffer, StdinBufferEventMap, StdinBufferInput, StdinBufferOptions};
use tokio::time::{Instant, sleep_until};

use super::Shared;
use crate::output_errors::FailureSource;
use maestro_text::Utf8Stream;

/// What woke the input task.
pub(super) enum Wake {
    /// A read finished.
    Read(io::Result<usize>),
    /// The window size may have changed.
    Resize,
    /// A deadline passed.
    Deadline,
}

/// The state of one input task; the platform reader drives it.
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

    /// Refreshes the window size and notifies the resize callback when it changed.
    pub(super) fn resized(&self) {
        if self.shared.dimensions.refresh() {
            self.shared.call_resize(self.generation);
        }
    }

    /// Whether reads continue; they stop at the end of input.
    pub(super) fn reading(&self) -> bool {
        self.reading
    }

    /// Keeps `error`, which ended the task.
    pub(super) fn fail(&self, error: io::Error) {
        self.shared.fail(FailureSource::Input, error);
    }

    /// Whether this task's generation is still the live one.
    pub(super) fn is_current(&self) -> bool {
        self.shared.is_current(self.generation)
    }

    /// The earlier of the buffer deadline and the pending keyboard decision.
    pub(super) fn next_deadline(&self) -> Option<Instant> {
        let buffered = self.buffer.deadline().map(Instant::from_std);
        [buffered, self.negotiate_at].into_iter().flatten().min()
    }

    /// Acts on what woke the task; a read of `count` bytes put them at the start of `chunk`.
    pub(super) fn handle(&mut self, wake: Wake, chunk: &[u8]) -> io::Result<()> {
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
                self.resized();
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
pub(super) async fn until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => pending().await,
    }
}

#[cfg(test)]
mod tests;
