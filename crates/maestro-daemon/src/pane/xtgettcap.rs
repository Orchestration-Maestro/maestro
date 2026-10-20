//! Pane capability query replies with byte offsets for ordered PTY output.

/// Incremental capability queries, bounded to 1,024 body bytes.
#[derive(Debug, Default)]
pub(super) struct XtgettcapQueryTracker {
    /// Framing position across writes.
    state: State,
    /// Encoded capability names.
    body: Vec<u8>,
    /// Replies in request order.
    pending: Vec<XtgettcapResponse>,
}

/// One reply at its request's exclusive end offset in the current write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct XtgettcapResponse {
    /// Exclusive byte offset in the most recent observation.
    pub(super) end_offset: usize,
    /// Complete DCS response.
    pub(super) bytes: Vec<u8>,
}

/// DCS framing without interpreting other control-string payloads.
#[derive(Debug, Clone, Copy, Default)]
enum State {
    /// Outside a control string.
    #[default]
    Ground,
    /// An escape prefix.
    Escape,
    /// DCS introducer before the plus.
    Intro,
    /// DCS introducer before the query marker.
    Plus,
    /// Accepted query payload.
    Body,
    /// Possible seven-bit terminator inside a query.
    BodyEscape,
    /// Discard a string; OSC also terminates on BEL.
    Ignore {
        /// Whether BEL terminates this string.
        osc: bool,
        /// Whether an escape prefix was seen.
        escaped: bool,
    },
}

impl XtgettcapQueryTracker {
    /// Retain incomplete input and queue completed replies.
    pub(super) fn observe(&mut self, bytes: &[u8]) {
        for (index, &byte) in bytes.iter().enumerate() {
            self.byte(byte, index + 1);
            if self.body.len() > 1024 {
                self.body.clear();
                self.state = State::Ignore {
                    osc: false,
                    escaped: false,
                };
            }
        }
    }

    /// Apply one byte, keeping the current write's end offset.
    fn byte(&mut self, byte: u8, end_offset: usize) {
        match self.state {
            State::Ground => self.ground(byte),
            State::Escape => self.escape(byte),
            State::Intro => self.intro(byte, b'+', State::Plus),
            State::Plus => self.intro(byte, b'q', State::Body),
            State::Body => match byte {
                0x1b => self.state = State::BodyEscape,
                0x9c => self.finish(end_offset),
                _ => self.body.push(byte),
            },
            State::BodyEscape => match byte {
                b'\\' => self.finish(end_offset),
                0x1b => {}
                _ => {
                    self.body.clear();
                    self.state = State::Ignore {
                        osc: false,
                        escaped: false,
                    };
                }
            },
            State::Ignore { osc, escaped } => {
                self.state = if (escaped && byte == b'\\')
                    || (!escaped && (byte == 0x9c || (osc && byte == 7)))
                {
                    State::Ground
                } else {
                    State::Ignore {
                        osc,
                        escaped: byte == 0x1b,
                    }
                };
            }
        }
    }

    /// Recognize seven- and eight-bit string introductions.
    fn ground(&mut self, byte: u8) {
        self.state = match byte {
            0x1b => State::Escape,
            0x90 => {
                self.body.clear();
                State::Intro
            }
            0x9d => State::Ignore {
                osc: true,
                escaped: false,
            },
            0x98 | 0x9e | 0x9f => State::Ignore {
                osc: false,
                escaped: false,
            },
            _ => State::Ground,
        };
    }

    /// Choose the string introduced by an escape prefix.
    fn escape(&mut self, byte: u8) {
        self.body.clear();
        self.state = match byte {
            b'P' => State::Intro,
            b']' => State::Ignore {
                osc: true,
                escaped: false,
            },
            b'_' | b'^' | b'X' => State::Ignore {
                osc: false,
                escaped: false,
            },
            0x1b => State::Escape,
            _ => State::Ground,
        };
    }

    /// Accept exactly the capability-query prefix, ignoring unrelated DCS.
    fn intro(&mut self, byte: u8, expected: u8, next: State) {
        self.state = if byte == expected {
            next
        } else if byte == 0x9c {
            State::Ground
        } else {
            State::Ignore {
                osc: false,
                escaped: byte == 0x1b,
            }
        };
    }

    /// Emit one reply per supported name, retaining the request's order.
    fn finish(&mut self, end_offset: usize) {
        for cap in self.body.split(|byte| *byte == b';') {
            if let Some(bytes) = response(cap) {
                self.pending.push(XtgettcapResponse { end_offset, bytes });
            }
        }
        self.body.clear();
        self.state = State::Ground;
    }

    /// Take replies without affecting an incomplete query.
    pub(super) fn drain_pending(&mut self) -> Vec<XtgettcapResponse> {
        std::mem::take(&mut self.pending)
    }
}

/// Validate and normalize a hex-encoded capability name.
fn response(cap: &[u8]) -> Option<Vec<u8>> {
    /// Upper-case hexadecimal digits used by the protocol.
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    if cap.is_empty() || !cap.len().is_multiple_of(2) || !cap.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let cap = cap.to_ascii_uppercase();
    let value = value(&cap)?;
    let mut response = Vec::from(b"\x1bP1+r".as_slice());
    response.extend_from_slice(&cap);
    if !value.is_empty() {
        response.push(b'=');

        for &byte in value {
            response.push(HEX[usize::from(byte >> 4)]);
            response.push(HEX[usize::from(byte & 15)]);
        }
    }
    response.extend_from_slice(b"\x1b\\");
    Some(response)
}

/// Capabilities supported by the pane terminal.
fn value(cap: &[u8]) -> Option<&'static [u8]> {
    match cap {
        b"5463" | b"5375" => Some(b""),
        b"524742" => Some(b"8"),
        b"73657472676266" => Some(b"\\E[38:2:%p1%d:%p2%d:%p3%dm"),
        b"73657472676262" => Some(b"\\E[48:2:%p1%d:%p2%d:%p3%dm"),
        b"4D73" => Some(b"\\E]52;%p1%s;%p2%s\\007"),
        b"536D756C78" => Some(b"\\E[4:%p1%dm"),
        b"536574756C63" => {
            Some(b"\\E[58:2::%p1%{65536}%/%d:%p1%{256}%/%{255}%&%d:%p1%{255}%&%d%;m")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
