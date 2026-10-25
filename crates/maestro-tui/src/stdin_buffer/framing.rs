//! Sequence recognition for buffered terminal input.

/// Escape byte opening a control sequence.
pub(super) const ESC: u8 = 0x1b;
/// Opening paste marker.
pub(super) const PASTE_START: &[u8] = b"\x1b[200~";
/// Closing paste marker.
pub(super) const PASTE_END: &[u8] = b"\x1b[201~";

/// Source-specific framing differences over the shared byte recognizer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Policy {
    /// Already-decoded chat text and historical shape admission.
    Chat,
    /// Exact original host bytes.
    Host,
}

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
        let (frame, rest) = self
            .rest
            .split_at(byte_frame_length(self.rest.as_bytes(), Policy::Chat)?);
        self.rest = rest;
        Some(frame)
    }
}

/// Length of the next admitted raw frame, including complete unsupported escapes.
#[must_use]
pub fn host_frame_length(buffer: &[u8]) -> Option<usize> {
    if buffer.starts_with(PASTE_START) {
        let end = find_subsequence(buffer, PASTE_END)?;
        std::str::from_utf8(&buffer[PASTE_START.len()..end]).ok()?;
        return Some(end + PASTE_END.len());
    }
    let length = byte_frame_length(buffer, Policy::Host)?;
    if buffer.first() != Some(&ESC) {
        crate::keys::parse_terminal_key_sequence(std::str::from_utf8(&buffer[..length]).ok()?)?;
    } else if !buffer.starts_with(b"\x1b[M") {
        std::str::from_utf8(&buffer[..length]).ok()?;
    }
    Some(length)
}

/// One byte recognizer shared by decoded chat and retained host input.
fn byte_frame_length(buffer: &[u8], policy: Policy) -> Option<usize> {
    if *buffer.first()? != ESC {
        return first_complete_utf8_char_len(buffer);
    }
    let introducer = *buffer.get(1)?;
    match introducer {
        b'[' => csi_length(buffer, policy),
        b']' | b'P' | b'_' => control_string(buffer).and_then(ControlString::complete_len),
        b'^' | b'X' if policy == Policy::Host => {
            control_string(buffer).and_then(ControlString::complete_len)
        }
        b'O' if policy == Policy::Host => (buffer.len() >= 3).then_some(3),
        b'O' => Some(2 + first_complete_utf8_char_len(&buffer[2..])?),
        ESC if policy == Policy::Host => doubled_escape_length(buffer),
        _ => Some(1 + first_complete_utf8_char_len(&buffer[1..])?),
    }
}

/// Recognize mouse framing before the general CSI final range.
fn csi_length(buffer: &[u8], policy: Policy) -> Option<usize> {
    if buffer.starts_with(b"\x1b[M") {
        if policy == Policy::Host {
            return (buffer.len() >= 6).then_some(6);
        }
        let text = std::str::from_utf8(&buffer[3..]).ok()?;
        let (index, last) = text.char_indices().nth(2)?;
        return Some(3 + index + last.len_utf8());
    }
    if buffer.starts_with(b"\x1b[<") {
        if policy == Policy::Host {
            return find_csi_final(buffer, b"Mm");
        }
        let report = &buffer[3..];
        let end = report
            .iter()
            .position(|byte| !byte.is_ascii_digit() && *byte != b';')?;
        return (matches!(report[end], b'M' | b'm')
            && report[..end]
                .split(|byte| *byte == b';')
                .map(<[u8]>::is_empty)
                .eq([false; 3]))
        .then_some(3 + end + 1);
    }
    buffer
        .iter()
        .enumerate()
        .skip(2)
        .find(|(_, byte)| (0x40..=0x7e).contains(*byte))
        .map(|(index, _)| index + 1)
}

/// Preserve doubled-Escape Alt keys but separate a valid following mouse report.
fn doubled_escape_length(buffer: &[u8]) -> Option<usize> {
    if buffer.starts_with(b"\x1b\x1b[<")
        && let Some(length) = find_csi_final(&buffer[1..], b"Mm")
        && super::mouse::parse_host_mouse(&buffer[1..=length]).is_some()
    {
        return Some(1);
    }
    if buffer.len() >= 7 && super::mouse::parse_host_mouse(&buffer[1..7]).is_some() {
        return Some(1);
    }
    byte_frame_length(&buffer[1..], Policy::Host).map(|length| length + 1)
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

/// Termination rule for a control string or a discarded reply tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlStringFamily {
    /// OSC terminated by BEL or ST.
    Osc,
    /// Other string terminated only by ST.
    StTerminated,
    /// Discarded CSI reply through its final byte.
    HostReplyCsi,
    /// Discarded mouse tail through M or m.
    OrphanedSgrMouseTail,
}

/// Completion state of an introduced control string.
pub(super) enum ControlString {
    /// Terminated string.
    Complete {
        /// Length through the terminator.
        len: usize,
    },
    /// A string awaiting its terminator.
    Incomplete {
        /// String termination policy.
        family: ControlStringFamily,
    },
}

impl ControlString {
    /// The full length only when a string is terminated.
    fn complete_len(self) -> Option<usize> {
        match self {
            Self::Complete { len, .. } => Some(len),
            Self::Incomplete { .. } => None,
        }
    }
}

/// Recognize introduced control strings and their termination state.
pub(super) fn control_string(buffer: &[u8]) -> Option<ControlString> {
    let family = match buffer.get(..2)? {
        b"\x1b]" => ControlStringFamily::Osc,
        b"\x1bP" | b"\x1b_" | b"\x1b^" | b"\x1bX" => ControlStringFamily::StTerminated,
        _ => return None,
    };

    Some(match control_string_terminator_for_family(buffer, family) {
        Some(len) => ControlString::Complete { len },
        None => ControlString::Incomplete { family },
    })
}

/// Length of the first fully validated UTF-8 scalar.
pub(super) fn first_complete_utf8_char_len(buffer: &[u8]) -> Option<usize> {
    let width = utf8_char_width(*buffer.first()?)?;

    if buffer.len() < width {
        return None;
    }

    std::str::from_utf8(&buffer[..width]).ok()?;
    Some(width)
}

/// Whether a valid UTF-8 prefix lacks only continuation bytes.
pub(super) fn starts_with_incomplete_utf8_char(buffer: &[u8]) -> bool {
    match std::str::from_utf8(buffer) {
        Ok(_) => false,
        Err(err) => err.valid_up_to() == 0 && err.error_len().is_none(),
    }
}

/// Expected byte width from a UTF-8 lead byte.
pub(super) fn utf8_char_width(first: u8) -> Option<usize> {
    if first < 0x80 {
        Some(1)
    } else if first & 0b1110_0000 == 0b1100_0000 {
        Some(2)
    } else if first & 0b1111_0000 == 0b1110_0000 {
        Some(3)
    } else if first & 0b1111_1000 == 0b1111_0000 {
        Some(4)
    } else {
        None
    }
}

/// First BEL or ST terminator, including its bytes.
pub(super) fn osc_string_terminator(buffer: &[u8]) -> Option<usize> {
    let st = find_subsequence(buffer, b"\x1b\\").map(|idx| idx + 2);
    let bel = buffer
        .iter()
        .position(|byte| *byte == b'\x07')
        .map(|idx| idx + 1);

    match (st, bel) {
        (Some(st), Some(bel)) => Some(st.min(bel)),
        (Some(st), None) => Some(st),
        (None, Some(bel)) => Some(bel),
        (None, None) => None,
    }
}

/// First ST terminator, including its bytes.
pub(super) fn st_string_terminator(buffer: &[u8]) -> Option<usize> {
    find_subsequence(buffer, b"\x1b\\").map(|idx| idx + 2)
}

/// Find the terminator permitted by the supplied string family.
pub(super) fn control_string_terminator_for_family(
    buffer: &[u8],
    family: ControlStringFamily,
) -> Option<usize> {
    match family {
        ControlStringFamily::Osc => osc_string_terminator(buffer),
        ControlStringFamily::StTerminated => st_string_terminator(buffer),
        ControlStringFamily::HostReplyCsi => None,
        ControlStringFamily::OrphanedSgrMouseTail => buffer
            .iter()
            .position(|byte| matches!(*byte, b'M' | b'm'))
            .map(|idx| idx + 1),
    }
}

/// Find a supported CSI final after its introducer.
pub(super) fn find_csi_final(buffer: &[u8], finals: &[u8]) -> Option<usize> {
    for (idx, byte) in buffer.iter().enumerate().skip(2) {
        if finals.contains(byte) {
            return Some(idx + 1);
        }
    }
    None
}

/// Find a fixed byte marker without decoding the input.
pub(super) fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
