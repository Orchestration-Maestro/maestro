//! Bounded routing of application program command strings.

use crate::kitty::graphics::command::{Command, Parser};

/// Supported application program command protocols.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    /// Image commands introduced by `G`.
    Kitty,
}

impl Protocol {
    /// Default encoded-payload limit for this protocol.
    #[must_use]
    pub const fn default_max_bytes(self) -> usize {
        match self {
            Self::Kitty => 65 * 1024 * 1024,
        }
    }
}

/// Progress through an APC string.
pub enum State {
    /// No string is being parsed.
    Inactive,
    /// The first protocol byte has not arrived.
    Identify,
    /// Unsupported or malformed input is discarded until the string ends.
    Ignore,
    /// Image control fields and payload are being parsed.
    Kitty(Box<Parser>),
}

/// Incremental APC router, retained between start and end actions.
pub struct Handler {
    /// Current routing state.
    pub state: State,
    /// Maximum encoded image payload bytes, sampled when `G` is recognized.
    pub max_bytes: usize,
    /// Whether future strings may select the image protocol.
    enabled: bool,
}

impl Default for Handler {
    fn default() -> Self {
        Self {
            state: State::Inactive,
            max_bytes: Protocol::Kitty.default_max_bytes(),
            enabled: true,
        }
    }
}

impl Handler {
    /// Begin a string, dropping any unfinished parser and payload.
    pub fn start(&mut self) {
        self.state = State::Identify;
    }

    /// Enable or disable recognition for future strings, not an active parser.
    pub fn enable(&mut self, protocol: Protocol, enabled: bool) {
        match protocol {
            Protocol::Kitty => self.enabled = enabled,
        }
    }

    /// Route one byte, dropping the parser immediately on a parse error.
    pub fn feed(&mut self, byte: u8) {
        match &mut self.state {
            State::Identify => {
                self.state = if byte == b'G' && self.enabled {
                    State::Kitty(Box::new(Parser::init(self.max_bytes)))
                } else {
                    State::Ignore
                };
            }
            State::Kitty(parser) => {
                if parser.feed(byte).is_err() {
                    self.state = State::Ignore;
                }
            }
            State::Inactive | State::Ignore => {}
        }
    }

    /// Route a slice with bulk payload feeding after protocol identification.
    pub fn feed_slice(&mut self, mut bytes: &[u8]) {
        if matches!(self.state, State::Identify)
            && let Some((&first, rest)) = bytes.split_first()
        {
            self.feed(first);
            bytes = rest;
        }
        if let State::Kitty(parser) = &mut self.state
            && parser.feed_slice(bytes).is_err()
        {
            self.state = State::Ignore;
        }
    }

    /// Finish a string and release its parser, returning a valid owned command.
    #[must_use]
    pub fn end(&mut self) -> Option<Command> {
        match std::mem::replace(&mut self.state, State::Inactive) {
            State::Kitty(parser) => parser.complete().ok(),
            State::Inactive | State::Identify | State::Ignore => None,
        }
    }
}
