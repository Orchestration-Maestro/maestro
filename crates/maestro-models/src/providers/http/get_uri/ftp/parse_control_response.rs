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
    /// The UTF-16 length of the kept lines, each with the newline that joins it.
    units: usize,
}

/// Framing state across reads: the unfinished line and any open multiline reply.
#[derive(Default)]
pub(crate) struct Frames {
    /// The text of the line whose end has not arrived.
    line: String,
    /// The multiline reply whose terminating line has not arrived.
    open: Option<Group>,
    /// The UTF-16 length of `line` and of the open group's kept lines.
    retained: usize,
    /// Characters examined, so a test can bound the traversal.
    #[cfg(test)]
    visits: std::cell::Cell<usize>,
}

impl Frames {
    /// Take every reply `text` completes, retaining an unfinished line or group.
    ///
    /// Each pushed character is examined a bounded number of times, because the text of an
    /// unfinished line is never searched or copied again.
    ///
    /// # Errors
    /// Returns [`TooLong`] when the retained text would exceed the accepted size.
    pub(crate) fn push(&mut self, text: &str) -> Result<Vec<Reply>, TooLong> {
        let units = text.encode_utf16().count();
        self.count(units);
        if self.retained + units > MAX_RETAINED {
            return Err(TooLong);
        }
        self.retained += units;
        let mut replies = Vec::new();
        let mut rest = text;
        while let Some(end) = rest.find('\n') {
            self.count(end + 1);
            self.line.push_str(&rest[..end]);
            self.count(end);
            let line = std::mem::take(&mut self.line);
            self.take_line(line, &mut replies);
            rest = &rest[end + 1..];
        }
        self.count(rest.len());
        self.line.push_str(rest);
        Ok(replies)
    }

    /// Characters examined while framing, which a bounded test compares with its input size.
    #[cfg(test)]
    pub(crate) fn visits(&self) -> usize {
        self.visits.get()
    }

    /// Record `characters` examined, which only the bounded test observes.
    fn count(&self, characters: usize) {
        #[cfg(test)]
        self.visits.set(self.visits.get() + characters);
        #[cfg(not(test))]
        let _ = (self, characters);
    }

    /// Take one complete line into the group it belongs to.
    fn take_line(&mut self, mut line: String, replies: &mut Vec<Reply>) {
        let stripped = line.strip_suffix('\r').is_some();
        if stripped {
            line.pop();
        }
        let units = line.encode_utf16().count();
        self.count(units);
        // The taken line, the newline that ended it and any carriage return before that newline
        // leave the retained text.
        self.retained -= units + 1 + usize::from(stripped);
        // The reference drops blank lines wherever they appear, including inside a group.
        if line.chars().all(maestro_text::is_whitespace_scalar) {
            self.count(units);
            return;
        }
        self.count(units);
        // A line a reply keeps costs its own text and the newline joining it to the next.
        let kept = units + 1;
        match self.open.take() {
            Some(mut group) => {
                if closes(&group.token, &line) {
                    self.retained -= group.units;
                    group.lines.push(line);
                    replies.push(reply(group.lines.join("\n")));
                } else {
                    group.lines.push(line);
                    group.units += kept;
                    self.retained += kept;
                    self.open = Some(group);
                }
            }
            None if multiline(&line) => {
                self.retained += kept;
                self.open = Some(Group {
                    token: line[..3].to_owned(),
                    lines: vec![line],
                    units: kept,
                });
            }
            None if single(&line) => replies.push(reply(line)),
            None => {}
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
