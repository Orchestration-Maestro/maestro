//! Sequence recognition for buffered terminal input.

/// The escape character that opens every control sequence.
const ESC: char = '\u{1b}';

/// The bell character, which also ends an operating system command.
const BEL: char = '\u{7}';

/// The string terminator: an escape followed by a backslash.
const STRING_TERMINATOR: &str = "\u{1b}\\";

/// Complete sequences at the start of a text, one per call.
///
/// Iteration ends at the end of the text or at an incomplete escape sequence; the
/// unconsumed rest is [`Frames::remainder`].
pub(super) struct Frames<'a> {
    /// The text not yet consumed.
    rest: &'a str,
}

impl<'a> Frames<'a> {
    /// Frames the sequences of `text`.
    pub(super) fn new(text: &'a str) -> Self {
        Self { rest: text }
    }

    /// The text after the last complete sequence.
    pub(super) fn remainder(&self) -> &'a str {
        self.rest
    }
}

impl<'a> Iterator for Frames<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let (frame, rest) = self.rest.split_at(frame_length(self.rest)?);
        self.rest = rest;
        Some(frame)
    }
}

/// The byte length of the complete sequence that starts `text`, or `None` when `text` is
/// empty or the sequence needs more input.
///
/// A character other than escape is a sequence of its own. After an escape, `[` opens a
/// control sequence, `]` an operating system command, `P` a device control string, `_` an
/// application command and `O` a single-shift sequence of one more character; any other
/// character is an alt-modified key.
fn frame_length(text: &str) -> Option<usize> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if first != ESC {
        return Some(first.len_utf8());
    }
    let introducer = chars.next()?;
    let body = chars.as_str();
    let body_length = match introducer {
        '[' => control_sequence_length(body)?,
        ']' => string_length(body, true)?,
        'P' | '_' => string_length(body, false)?,
        'O' => body.chars().next()?.len_utf8(),
        _ => 0,
    };
    Some(first.len_utf8() + introducer.len_utf8() + body_length)
}

/// The length of a control sequence after its `ESC [`.
///
/// `M` takes three more characters (the legacy mouse report), `<` begins a mouse report
/// that needs three decimal fields, and any other body ends at its first character from
/// `@` to `~`.
fn control_sequence_length(body: &str) -> Option<usize> {
    if let Some(coordinates) = body.strip_prefix('M') {
        let (index, last) = coordinates.char_indices().nth(2)?;
        return Some(1 + index + last.len_utf8());
    }
    if let Some(report) = body.strip_prefix('<') {
        return Some(1 + mouse_report_length(report)?);
    }
    let (index, last) = body.char_indices().find(|(_, c)| ('@'..='~').contains(c))?;
    Some(index + last.len_utf8())
}

/// The length of a mouse report after its `ESC [ <`: three nonempty decimal fields
/// separated by semicolons and ended by `M` or `m`.
fn mouse_report_length(report: &str) -> Option<usize> {
    let end = report.find(|c: char| !c.is_ascii_digit() && c != ';')?;
    let complete = report[end..].starts_with(['M', 'm'])
        && report[..end].split(';').map(str::is_empty).eq([false; 3]);
    complete.then_some(end + 1)
}

/// The length of a string sequence after its introducer: up to the first string
/// terminator, or the first bell when `bell_ends`.
fn string_length(body: &str, bell_ends: bool) -> Option<usize> {
    let terminator = body
        .find(STRING_TERMINATOR)
        .map(|at| at + STRING_TERMINATOR.len());
    let bell = body
        .find(BEL)
        .filter(|_| bell_ends)
        .map(|at| at + BEL.len_utf8());
    [terminator, bell].into_iter().flatten().min()
}

/// The character typed by an unmodified enhanced-keyboard report.
///
/// The report is `ESC [ code[:[shifted][:base]] u` with no modifier field and a code
/// point from 32 up that is a Unicode scalar value.
pub(super) fn unmodified_printable(sequence: &str) -> Option<char> {
    let report = sequence.strip_prefix("\u{1b}[")?.strip_suffix('u')?;
    let mut fields = report.split(':');
    let code = fields.next()?;
    let alternates = [fields.next(), fields.next()];
    let digits = |field: &str| field.bytes().all(|byte| byte.is_ascii_digit());
    let valid = digits(code)
        && fields.next().is_none()
        && alternates[0].is_none_or(digits)
        && alternates[1].is_none_or(|base| !base.is_empty() && digits(base));
    if !valid {
        return None;
    }
    let code: u32 = code.parse().ok()?;
    char::from_u32(code).filter(|_| code >= 32)
}
