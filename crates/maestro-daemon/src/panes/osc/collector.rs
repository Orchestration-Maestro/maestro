//! Bounded OSC framing shared by retained evidence consumers.

/// Complete OSC bodies, excluding framing and ignored control strings.
#[derive(Debug)]
pub(super) struct OscStreamCollector {
    /// Current framing state.
    state: State,
    /// Incomplete OSC body.
    body: Vec<u8>,
    /// Maximum retained body size for this consumer.
    limit: usize,
}

/// Current string framing position.
#[derive(Debug, Clone, Copy, Default)]
enum State {
    /// Outside a string.
    #[default]
    Ground,
    /// Escape introducer.
    Escape,
    /// OSC body with a possible trailing escape.
    Body(
        /// Whether the previous byte was escape.
        bool,
    ),
    /// Ignore a different string or an oversized OSC body.
    Ignore {
        /// Whether BEL terminates this string.
        bel: bool,
        /// Whether the previous byte was escape.
        escaped: bool,
    },
}

impl OscStreamCollector {
    /// Choose the retained body bound.
    pub(super) fn new(limit: usize) -> Self {
        Self {
            state: State::Ground,
            body: Vec::new(),
            limit,
        }
    }

    /// Observe the accepted body while its terminator remains incomplete.
    pub(super) fn in_progress_body(&self) -> Option<&[u8]> {
        matches!(self.state, State::Body(_)).then_some(self.body.as_slice())
    }

    /// Deliver only completed, bounded OSC bodies.
    pub(super) fn observe(&mut self, bytes: &[u8], mut receive: impl FnMut(&[u8], usize)) {
        for (index, &byte) in bytes.iter().enumerate() {
            self.byte(byte, &mut |body| receive(body, index + 1));
        }
    }

    /// Apply one byte without interpreting a command body.
    fn byte(&mut self, byte: u8, receive: &mut impl FnMut(&[u8])) {
        match self.state {
            State::Ground if byte == 0x1b => self.state = State::Escape,
            State::Ground => {}
            State::Escape => self.escape(byte),
            State::Body(false) => self.body_byte(byte, receive),
            State::Body(true) => self.escaped_body_byte(byte, receive),
            State::Ignore { bel, escaped } => {
                self.state = if (escaped && byte == b'\\') || (!escaped && bel && byte == 7) {
                    State::Ground
                } else {
                    State::Ignore {
                        bel,
                        escaped: byte == 0x1b,
                    }
                };
            }
        }
    }

    /// Start only OSC and ignore embedded introductions in other strings.
    fn escape(&mut self, byte: u8) {
        self.state = match byte {
            b']' => {
                self.body.clear();
                State::Body(false)
            }
            0x1b => State::Escape,
            b'P' | b'_' | b'^' | b'X' => State::Ignore {
                bel: false,
                escaped: false,
            },
            _ => State::Ground,
        };
    }

    /// Finish a BEL-terminated body or retain a possible ST prefix.
    fn body_byte(&mut self, byte: u8, receive: &mut impl FnMut(&[u8])) {
        match byte {
            7 => self.finish(receive),
            0x1b => self.state = State::Body(true),
            _ => self.push(byte),
        }
    }

    /// Preserve nonterminating escape bytes as payload.
    fn escaped_body_byte(&mut self, byte: u8, receive: &mut impl FnMut(&[u8])) {
        match byte {
            b'\\' => self.finish(receive),
            7 => {
                self.push(0x1b);
                if matches!(self.state, State::Body(_)) {
                    self.finish(receive);
                } else {
                    self.state = State::Ground;
                }
            }
            0x1b => {
                self.push(0x1b);
                self.state = match self.state {
                    State::Body(_) => State::Body(true),
                    State::Ignore { bel, .. } => State::Ignore { bel, escaped: true },
                    state => state,
                };
            }
            _ => {
                self.push(0x1b);
                if matches!(self.state, State::Body(_)) {
                    self.push(byte);
                }
            }
        }
    }

    /// Drop the whole body once its retained size is exceeded.
    fn push(&mut self, byte: u8) {
        self.body.push(byte);
        self.state = if self.body.len() > self.limit {
            self.body.clear();
            State::Ignore {
                bel: true,
                escaped: false,
            }
        } else {
            State::Body(false)
        };
    }

    /// Deliver a completed body before accepting another introduction.
    fn finish(&mut self, receive: &mut impl FnMut(&[u8])) {
        receive(&self.body);
        self.body.clear();
        self.state = State::Ground;
    }
}

impl Default for OscStreamCollector {
    fn default() -> Self {
        Self::new(4096)
    }
}
