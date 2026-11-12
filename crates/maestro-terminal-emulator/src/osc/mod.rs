//! Owned operating-system command decoding.

mod parsers;

pub use parsers::{color, semantic_prompt};

/// Owned semantic prompt command.
pub type SemanticPrompt = semantic_prompt::Command;

/// One decoded operating-system command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Window title bytes.
    ChangeWindowTitle(Vec<u8>),
    /// Semantic prompt phase and unvalidated options.
    SemanticPrompt(SemanticPrompt),
    /// Colour requests with their reply terminator.
    ColorOperation {
        /// Selected OSC colour operation.
        op: color::Operation,
        /// Ordered requests.
        requests: color::List,
        /// Requested reply terminator.
        terminator: Terminator,
    },
    /// Selection byte and undecoded clipboard payload.
    ClipboardContents {
        /// Clipboard selection.
        kind: u8,
        /// Raw contents or query bytes.
        data: Vec<u8>,
    },
    /// Start a hyperlink with independently owned option and URI bytes.
    HyperlinkStart {
        /// Optional nonempty identifier.
        id: Option<Vec<u8>>,
        /// Unvalidated URI.
        uri: Vec<u8>,
    },
    /// End the current hyperlink.
    HyperlinkEnd,
    /// Unvalidated working-directory bytes.
    ReportPwd {
        /// Raw directory report.
        value: Vec<u8>,
    },
    /// Colour-protocol requests.
    KittyColorProtocol(crate::kitty::color::Osc),
}

/// Reply terminator selected by the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terminator {
    /// Escape followed by backslash.
    St,
    /// Bell byte.
    Bel,
}
impl Terminator {
    /// Classify a missing or non-BEL terminator as ST.
    #[must_use]
    pub const fn init(ch: Option<u8>) -> Self {
        match ch {
            Some(7) => Self::Bel,
            _ => Self::St,
        }
    }
    /// Return the exact reply bytes.
    #[must_use]
    pub const fn string(self) -> &'static [u8] {
        match self {
            Self::St => b"\x1b\\",
            Self::Bel => b"\x07",
        }
    }
}

/// Prefix admission and trailing capture state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Waiting for the first command digit.
    Start,
    /// Invalid until reset.
    Invalid,
    /// An admitted numeric prefix.
    Prefix(u16),
    /// Capturing a command's trailing bytes.
    Capture(u16),
}

/// Stateful OSC byte decoder with owned returned payloads.
pub struct Parser {
    /// Availability of allocating captures and request lists.
    pub alloc: bool,
    /// Prefix/capture state.
    state: State,
    /// Fixed capture, including any leaf sentinel.
    buffer: [u8; Self::MAX_BUF],
    /// Used fixed capture length.
    len: usize,
    /// An allocating capture when the command permits it.
    growing: Option<Vec<u8>>,
}

impl Parser {
    /// Capacity of a fixed capture, including any leaf sentinel.
    pub const MAX_BUF: usize = 2048;

    /// Construct a decoder with the selected allocation availability.
    #[must_use]
    pub const fn init(alloc: bool) -> Self {
        Self {
            alloc,
            state: State::Start,
            buffer: [0; Self::MAX_BUF],
            len: 0,
            growing: None,
        }
    }

    /// Discard capture and restore prefix admission.
    pub fn reset(&mut self) {
        self.state = State::Start;
        self.len = 0;
        self.growing = None;
    }

    /// Consume one prefix or payload byte.
    pub fn next(&mut self, c: u8) {
        self.state = match self.state {
            State::Start if c.is_ascii_digit() => State::Prefix(u16::from(c - b'0')),
            State::Prefix(n)
                if c == b';'
                    && matches!(n, 0 | 2 | 4 | 5 | 7 | 8 | 9 | 10..=19 | 21 | 52 | 104 | 105 | 110..=119 | 133 | 1337) =>
            {
                if matches!(n, 4 | 5 | 10..=19 | 21 | 104 | 105 | 110..=119) && !self.alloc {
                    return self.state = State::Invalid;
                }
                if n == 52 && self.alloc {
                    self.growing = Some(Vec::new());
                }
                State::Capture(n)
            }
            State::Prefix(n) if c.is_ascii_digit() => {
                let next = n * 10 + u16::from(c - b'0');
                if matches!(
                    (n, next),
                    (1, 10..=19)
                        | (2, 21)
                        | (5, 52)
                        | (10, 104 | 105)
                        | (11, 110..=119)
                        | (13, 133)
                        | (133, 1337)
                ) {
                    State::Prefix(next)
                } else {
                    State::Invalid
                }
            }
            State::Capture(n) => {
                if self.push(c) {
                    State::Capture(n)
                } else {
                    State::Invalid
                }
            }
            _ => State::Invalid,
        };
    }

    /// Append a capture byte, respecting fixed capacity.
    fn push(&mut self, c: u8) -> bool {
        if let Some(data) = &mut self.growing {
            data.push(c);
        } else if let Some(slot) = self.buffer.get_mut(self.len) {
            *slot = c;
            self.len += 1;
        } else {
            return false;
        }
        true
    }

    /// Read bytes after appending the leaf's required sentinel.
    fn terminated(&mut self) -> Option<&[u8]> {
        if !matches!(self.state, State::Capture(_)) || !self.push(0) {
            self.state = State::Invalid;
            return None;
        }
        let data = self.data();
        Some(&data[..data.len() - 1])
    }

    /// Read captured bytes without adding a sentinel.
    fn data(&self) -> &[u8] {
        self.growing.as_deref().unwrap_or(&self.buffer[..self.len])
    }

    /// Finalize captured bytes into an independently owned command.
    pub fn end(&mut self, terminator_ch: Option<u8>) -> Option<Command> {
        let rejects_capture = matches!(
            self.state,
            State::Prefix(_) | State::Capture(0 | 2 | 7 | 8 | 52 | 133)
        );
        let result = match self.state {
            State::Capture(4 | 5 | 10..=19 | 104 | 105 | 110..=119)
            | State::Prefix(4 | 5 | 10..=19 | 104 | 105 | 110..=119) => {
                color::parse(self, terminator_ch)
            }
            State::Capture(0 | 2) => parsers::change_window_title::parse(self, terminator_ch),
            State::Capture(52) => parsers::clipboard_operation::parse(self, terminator_ch),
            State::Capture(9) => parsers::osc9::parse(self, terminator_ch),
            State::Capture(7) => parsers::report_pwd::parse(self, terminator_ch),
            State::Capture(1337) => parsers::iterm2::parse(self, terminator_ch),
            State::Capture(21) => parsers::kitty_color::parse(self, terminator_ch),
            State::Capture(133) => semantic_prompt::parse(self, terminator_ch),
            State::Capture(8) => parsers::hyperlink::parse(self, terminator_ch),
            _ => None,
        };
        if result.is_none() && rejects_capture {
            self.state = State::Invalid;
        }
        result
    }
}

#[cfg(test)]
mod tests;
