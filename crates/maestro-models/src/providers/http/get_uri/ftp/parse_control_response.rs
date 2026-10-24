//! Incremental framing of FTP control replies.

/// The largest reply text the client retains, in UTF-16 code units.
const MAX_RETAINED: usize = 2usize.pow(16);

/// One complete control reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Reply {
    /// The three-digit reply code.
    pub(crate) code: u16,
    /// The reply text, with every CRLF normalized to LF.
    pub(crate) message: String,
}

impl Reply {
    /// Whether the reply announces a positive completion.
    pub(crate) fn completed(&self) -> bool {
        (200..300).contains(&self.code)
    }

    /// Whether the reply announces a positive intermediate step.
    pub(crate) fn intermediate(&self) -> bool {
        (300..400).contains(&self.code)
    }

    /// Whether the reply is a server refusal or failure.
    pub(crate) fn refused(&self) -> bool {
        self.code >= 400
    }
}

/// A reply whose retained text exceeded [`MAX_RETAINED`].
#[derive(Debug)]
pub(crate) struct TooLong;

impl std::fmt::Display for TooLong {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "FTP control response exceeded maximum allowed size"
        )
    }
}

impl std::error::Error for TooLong {}

/// A multiline reply whose terminating line has not arrived.
struct Group {
    /// The three digits every line of the group repeats.
    token: String,
    /// The lines kept so far, including the opening line.
    lines: Vec<String>,
}

/// Framing state across reads: the unfinished line and any open multiline reply.
#[derive(Default)]
pub(crate) struct Frames {
    /// The text of the line whose end has not arrived.
    line: String,
    /// The multiline reply whose terminating line has not arrived.
    open: Option<Group>,
    /// Characters examined, so a test can bound the traversal.
    #[cfg(test)]
    visits: std::cell::Cell<usize>,
}

impl Frames {
    /// Take every reply `text` completes, retaining an unfinished line or group.
    ///
    /// # Errors
    /// Returns [`TooLong`] when the retained text would exceed the accepted size.
    pub(crate) fn push(&mut self, text: &str) -> Result<Vec<Reply>, TooLong> {
        if self.retained() + text.encode_utf16().count() > MAX_RETAINED {
            return Err(TooLong);
        }
        let mut buffer = std::mem::take(&mut self.line);
        buffer.push_str(text);
        let mut replies = Vec::new();
        let mut rest = buffer.as_str();
        while let Some(end) = rest.find('\n') {
            let (line, remaining) = rest.split_at(end);
            rest = &remaining[1..];
            self.take_line(line.strip_suffix('\r').unwrap_or(line), &mut replies);
        }
        rest.clone_into(&mut self.line);
        Ok(replies)
    }

    /// Characters examined while framing, which a bounded test compares with its input size.
    #[cfg(test)]
    pub(crate) fn visits(&self) -> usize {
        self.visits.get()
    }

    /// Take one complete line into the group it belongs to.
    fn take_line(&mut self, line: &str, replies: &mut Vec<Reply>) {
        #[cfg(test)]
        self.visits.set(self.visits.get() + line.chars().count());
        // The reference drops blank lines wherever they appear, including inside a group.
        if line.chars().all(maestro_text::is_whitespace_scalar) {
            return;
        }
        match self.open.take() {
            Some(mut group) => {
                group.lines.push(line.to_owned());
                if closes(&group.token, line) {
                    replies.push(reply(group.lines.join("\n")));
                } else {
                    self.open = Some(group);
                }
            }
            None if multiline(line) => {
                self.open = Some(Group {
                    token: line[..3].to_owned(),
                    lines: vec![line.to_owned()],
                });
            }
            None if single(line) => replies.push(reply(line.to_owned())),
            None => {}
        }
    }

    /// The UTF-16 length of everything not yet formed into a reply.
    fn retained(&self) -> usize {
        let line = self.line.encode_utf16().count();
        match &self.open {
            Some(group) => {
                line + group
                    .lines
                    .iter()
                    .map(|line| line.encode_utf16().count() + 1)
                    .sum::<usize>()
            }
            None => line,
        }
    }
}

/// Build a reply from its complete text.
fn reply(message: String) -> Reply {
    Reply {
        code: message
            .get(..3)
            .and_then(|code| code.parse().ok())
            .unwrap_or(0),
        message,
    }
}

/// Whether the line opens a multiline reply.
fn multiline(line: &str) -> bool {
    digits(line) && line.as_bytes().get(3) == Some(&b'-')
}

/// Whether the line is a complete single-line reply.
fn single(line: &str) -> bool {
    digits(line) && matches!(line.as_bytes().get(3), None | Some(b' '))
}

/// Whether the line terminates the group opened with `token`.
fn closes(token: &str, line: &str) -> bool {
    line.starts_with(token) && matches!(line.as_bytes().get(token.len()), None | Some(b' '))
}

/// Whether the line starts with three ASCII digits.
fn digits(line: &str) -> bool {
    line.as_bytes()
        .get(..3)
        .is_some_and(|code| code.iter().all(u8::is_ascii_digit))
}
