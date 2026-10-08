#![doc = include_str!("../../../../docs/terminal/input.md")]

mod framing;

use std::borrow::Cow;
use std::mem;
use std::time::{Duration, Instant};

use framing::{Frames, unmodified_printable};

/// The marker that opens a bracketed paste.
const PASTE_START: &str = "\u{1b}[200~";

/// The marker that closes a bracketed paste.
const PASTE_END: &str = "\u{1b}[201~";

/// The settings of a [`StdinBuffer`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StdinBufferOptions {
    /// The delay used to arm a representable deadline for an incomplete escape sequence.
    pub timeout: Duration,
}

impl Default for StdinBufferOptions {
    /// A timeout of 10 ms.
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(10),
        }
    }
}

/// One chunk of terminal input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StdinBufferInput<'a> {
    /// Text that is already decoded.
    Text(&'a str),
    /// Bytes read from a device. Each chunk is decoded on its own, so a chunk that ends
    /// inside a multibyte character is not completed by the next one.
    Bytes(&'a [u8]),
}

impl StdinBufferInput<'_> {
    /// The text of the chunk.
    ///
    /// A single byte above 127 is the legacy form of alt with the character 128 lower, so
    /// it becomes an escape followed by that character; other bytes are decoded as UTF-8,
    /// invalid parts becoming U+FFFD.
    fn decode(&self) -> Cow<'_, str> {
        match *self {
            Self::Text(text) => Cow::Borrowed(text),
            Self::Bytes(&[byte]) if byte > 127 => {
                Cow::Owned(format!("\u{1b}{}", char::from(byte - 128)))
            }
            Self::Bytes(bytes) => String::from_utf8_lossy(bytes),
        }
    }
}

/// An event a [`StdinBuffer`] produces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StdinBufferEventMap {
    /// One character, one complete escape sequence, a fragment released at its deadline
    /// or immediately before a paste starts, or the empty text of an empty normal input.
    Data(String),
    /// The text of a bracketed paste, without its markers.
    Paste(String),
}

/// Whether input is ordinary or inside a bracketed paste.
#[derive(Debug)]
enum Mode {
    /// Input is framed into sequences.
    Normal,
    /// Input is the text of a paste, collected until its end marker.
    Paste(String),
}

/// Groups terminal input into complete events, keeping what cannot be framed yet.
///
/// The buffer owns no clock: callers pass the instant of each input and call
/// [`StdinBuffer::expire`] at [`StdinBuffer::deadline`].
#[derive(Debug)]
pub struct StdinBuffer {
    /// How long a fragment waits.
    timeout: Duration,
    /// The incomplete escape sequence, which is empty during a paste.
    buffer: String,
    /// Whether a paste is open, with its text so far.
    mode: Mode,
    /// When the fragment is released, if one waits and the deadline is representable.
    deadline: Option<Instant>,
    /// The character of the last unmodified enhanced key report, whose raw duplicate is
    /// dropped next.
    printable: Option<char>,
}

impl StdinBuffer {
    /// Creates an empty buffer.
    #[must_use]
    pub fn new(options: StdinBufferOptions) -> Self {
        Self {
            timeout: options.timeout,
            buffer: String::new(),
            mode: Mode::Normal,
            deadline: None,
            printable: None,
        }
    }

    /// Consumes one input observed at `now` and returns its events in order.
    ///
    /// It cancels the pending deadline and arms a new one when a fragment remains and
    /// the deadline is representable. It never expires a due deadline itself.
    pub fn process(
        &mut self,
        data: StdinBufferInput<'_>,
        now: Instant,
    ) -> Vec<StdinBufferEventMap> {
        self.deadline = None;
        let text = data.decode();
        let mut events = Vec::new();
        if text.is_empty() && self.buffer.is_empty() {
            if matches!(self.mode, Mode::Normal) {
                self.emit_data("", &mut events);
            }
            return events;
        }
        self.buffer.push_str(&text);
        while self.consume(now, &mut events) {}
        events
    }

    /// When the waiting fragment is released, or `None` if none waits or the instant is
    /// not representable.
    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Releases the fragment as one `Data` event when `now` has reached the deadline. The
    /// deadline is then cleared, so a fragment is released at most once.
    pub fn expire(&mut self, now: Instant) -> Vec<StdinBufferEventMap> {
        if self.deadline.is_none_or(|deadline| now < deadline) {
            return Vec::new();
        }
        let mut events = Vec::new();
        for sequence in self.flush() {
            self.emit_data(&sequence, &mut events);
        }
        events
    }

    /// Returns and removes the fragment without producing an event, and cancels the
    /// deadline. The text of a paste in progress is not part of it.
    pub fn flush(&mut self) -> Vec<String> {
        self.deadline = None;
        if self.buffer.is_empty() {
            return Vec::new();
        }
        self.printable = None;
        vec![mem::take(&mut self.buffer)]
    }

    /// Discards the fragment, the paste in progress, the deadline and the duplicate
    /// memory.
    pub fn clear(&mut self) {
        self.deadline = None;
        self.buffer.clear();
        self.mode = Mode::Normal;
        self.printable = None;
    }

    /// The fragment that waits for more input; never the text of a paste in progress.
    #[must_use]
    pub fn get_buffer(&self) -> &str {
        &self.buffer
    }

    /// Clears the buffer, which stays usable.
    pub fn destroy(&mut self) {
        self.clear();
    }

    /// Handles the buffered text once and reports whether another consumption pass is
    /// needed.
    fn consume(&mut self, now: Instant, events: &mut Vec<StdinBufferEventMap>) -> bool {
        match mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Paste(content) => self.continue_paste(content, events),
            Mode::Normal => self.frame_input(now, events),
        }
    }

    /// Frames the buffered input, keeping the incomplete rest and arming its deadline
    /// when representable, or opens a paste when a start marker is found.
    fn frame_input(&mut self, now: Instant, events: &mut Vec<StdinBufferEventMap>) -> bool {
        let mut text = mem::take(&mut self.buffer);
        if let Some(start) = text.find(PASTE_START) {
            self.begin_paste(text, start, events);
            return true;
        }
        let kept = self.emit_sequences(&text, events).len();
        text.drain(..text.len() - kept);
        self.buffer = text;
        if !self.buffer.is_empty() {
            self.deadline = now.checked_add(self.timeout);
        }
        false
    }

    /// Emits the input before the start marker at `start` and opens the paste, leaving
    /// the text after the marker as buffered input.
    fn begin_paste(
        &mut self,
        mut text: String,
        start: usize,
        events: &mut Vec<StdinBufferEventMap>,
    ) {
        let incomplete = self.emit_sequences(&text[..start], events);
        if !incomplete.is_empty() {
            self.emit_data(incomplete, events);
        }
        self.printable = None;
        self.mode = Mode::Paste(String::new());
        text.drain(..start + PASTE_START.len());
        self.buffer = text;
    }

    /// Adds the buffered text to the open paste `content`. Emits the paste if its end
    /// marker has arrived, leaving the text after the marker as buffered input, and
    /// reports whether any is left.
    fn continue_paste(
        &mut self,
        mut content: String,
        events: &mut Vec<StdinBufferEventMap>,
    ) -> bool {
        content.push_str(&mem::take(&mut self.buffer));
        let Some(end) = content.find(PASTE_END) else {
            self.mode = Mode::Paste(content);
            return false;
        };
        self.buffer = content.split_off(end + PASTE_END.len());
        content.truncate(end);
        events.push(StdinBufferEventMap::Paste(content));
        !self.buffer.is_empty()
    }

    /// Emits the complete sequences of `text` as data and returns the incomplete rest.
    fn emit_sequences<'t>(
        &mut self,
        text: &'t str,
        events: &mut Vec<StdinBufferEventMap>,
    ) -> &'t str {
        let mut frames = Frames::new(text);
        for frame in frames.by_ref() {
            self.emit_data(frame, events);
        }
        frames.remainder()
    }

    /// Emits `sequence` as data unless it is the raw duplicate of the character just
    /// reported by an enhanced key report, and remembers the character it reports.
    fn emit_data(&mut self, sequence: &str, events: &mut Vec<StdinBufferEventMap>) {
        let mut scalars = sequence.chars();
        let raw = scalars.next().filter(|_| scalars.as_str().is_empty());
        if raw.is_some() && raw == self.printable.take() {
            return;
        }
        self.printable = unmodified_printable(sequence);
        events.push(StdinBufferEventMap::Data(sequence.to_owned()));
    }
}
