//! The input state of one started generation: it decodes what the platform reader
//! delivers, frames it with the buffer, and delivers the events to the callbacks.

use std::future::pending;
use std::io;
use std::rc::Rc;
use std::time::Duration;

use maestro_tui::{StdinBuffer, StdinBufferEventMap, StdinBufferInput, StdinBufferOptions};
use tokio::time::{Instant, sleep_until};

use super::Shared;
use super::lifecycle::Reply;
use crate::output_errors::FailureSource;
use maestro_text::Utf8Stream;

/// How long a fragment that may start a keyboard reply waits for the rest before it is input.
const FRAGMENT: Duration = Duration::from_millis(150);

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
    /// The start of a keyboard reply the framing gave up on, and when it becomes input.
    fragment: Option<(String, Instant)>,
}

impl Input {
    /// Creates the state for the task of `generation`.
    pub(super) fn new(shared: Rc<Shared>, generation: u64) -> Self {
        Self {
            shared,
            generation,
            buffer: StdinBuffer::new(StdinBufferOptions::default()),
            decoder: Utf8Stream::default(),
            reading: true,
            fragment: None,
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

    /// The earlier of the buffer deadline and the release of a held reply fragment.
    pub(super) fn next_deadline(&self) -> Option<Instant> {
        let buffered = self.buffer.deadline().map(Instant::from_std);
        let held = self.fragment.as_ref().map(|(_, due)| *due);
        [buffered, held].into_iter().flatten().min()
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

    /// Releases a held reply fragment as input and the buffered fragment, each when due.
    fn expire(&mut self) -> io::Result<()> {
        let now = Instant::now();
        if self.fragment.as_ref().is_some_and(|(_, due)| *due <= now) {
            self.release();
        }
        let events = self.buffer.expire(now.into_std());
        self.dispatch(events)
    }

    /// Delivers events in order, stopping once the generation ends.
    fn dispatch(&mut self, events: Vec<StdinBufferEventMap>) -> io::Result<()> {
        for event in events {
            if !self.is_current() {
                break;
            }
            match event {
                StdinBufferEventMap::Data(sequence) => self.sequence(&sequence)?,
                StdinBufferEventMap::Paste(content) => {
                    let text = format!("\x1b[200~{content}\x1b[201~");
                    self.shared.call_input(self.generation, &text);
                }
            }
        }
        Ok(())
    }

    /// Routes one framed sequence: a keyboard reply is consumed, the start of one is held until
    /// the rest arrives or [`FRAGMENT`] passes, and anything else, a held fragment first, is
    /// input.
    fn sequence(&mut self, sequence: &str) -> io::Result<()> {
        if let Some((held, _)) = self.fragment.take() {
            if self.absorbs(&format!("{held}{sequence}"))? {
                return Ok(());
            }
            self.shared.call_input(self.generation, &held);
        }
        if !self.absorbs(sequence)? {
            self.shared.call_input(self.generation, sequence);
        }
        Ok(())
    }

    /// Whether `text` is a keyboard reply, which is acted on, or while negotiation is open the
    /// start of one, which is held.
    fn absorbs(&mut self, text: &str) -> io::Result<bool> {
        if let Some(reply) = Reply::parse(text) {
            self.shared.answer(reply)?;
            return Ok(true);
        }
        let holds = self.shared.negotiating.get() && Reply::is_prefix(text);
        if holds {
            self.hold(text.to_owned());
        }
        Ok(holds)
    }

    /// Keeps `fragment` until the rest of a reply arrives or [`FRAGMENT`] passes.
    fn hold(&mut self, fragment: String) {
        self.fragment = Some((fragment, Instant::now() + FRAGMENT));
    }

    /// Gives the held fragment to the input callback, unless a drain has closed negotiation.
    fn release(&mut self) {
        if let Some((fragment, _)) = self.fragment.take()
            && self.shared.negotiating.get()
        {
            self.shared.call_input(self.generation, &fragment);
        }
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
