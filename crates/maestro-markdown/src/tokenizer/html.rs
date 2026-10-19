//! Directive boundaries over caller-escaped literal tag spans.
use maestro_text::is_whitespace_scalar;

/// A raw directive's safe payload and original consumed byte count.
pub(super) struct Html {
    /// Bytes consumed in the original input, not in the encoded payload.
    pub(super) consumed: usize,
    /// HTML admitted by the caller's escaping policy.
    pub(super) value: String,
}

/// Block and inline directive termination rules.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Permit an unfinished directive and retain block terminators.
    Block,
    /// Require a closed inline directive.
    Inline,
}

/// Delimiter family of an HTML directive.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Directive {
    /// An authored HTML comment.
    Comment,
    /// A processing instruction.
    Processing,
    /// A declaration.
    Declaration,
    /// A CDATA section.
    Cdata,
}

/// Progress after the directive's own closing delimiter.
#[derive(PartialEq, Eq)]
enum Phase {
    /// Find the closing delimiter outside escaped tags.
    Body,
    /// A block comment also consumes its same-line suffix.
    Tail,
    /// Consume immediately following line terminators.
    Newlines,
    /// The token is complete.
    Done,
}

/// Source-ordered directive scan, with at most one pending literal tag.
struct Scanner<'a> {
    /// Original authored input.
    source: &'a str,
    /// Entry-point grammar.
    mode: Mode,
    /// Selected delimiter family.
    directive: Directive,
    /// Current byte position in the original input.
    cursor: usize,
    /// First byte after the opening delimiter.
    body_start: usize,
    /// Final renderer-facing payload.
    value: String,
    /// An opening literal tag whose closing angle has not been seen yet.
    tag: Option<String>,
    /// Current termination phase.
    phase: Phase,
}

/// Admit a block directive with the block grammar's optional indentation.
pub(super) fn block(source: &str) -> Option<Html> {
    let indent = source.bytes().take_while(|byte| *byte == b' ').count();
    if indent > 3 {
        return None;
    }
    Scanner::new(source, indent, Mode::Block)?.scan()
}

/// Admit a closed inline directive without changing its source coordinates.
pub(super) fn inline(source: &str) -> Option<Html> {
    Scanner::new(source, 0, Mode::Inline)?.scan()
}

/// Whether the caller displays this angle span as text rather than masking HTML.
pub(super) fn displayed_tag(source: &str) -> bool {
    literal(source).is_some()
}

/// Keep one block line terminator without retaining separator-only lines.
pub(super) fn block_value(mut value: String) -> String {
    let end = value.trim_end_matches('\n').len();
    if end < value.len() {
        value.truncate(end + 1);
    }
    value
}

impl Directive {
    /// Identify a source-backed opening delimiter at the current entry point.
    fn opening(source: &str, mode: Mode) -> Option<(Self, usize)> {
        if source.starts_with("<!--") {
            Some((Self::Comment, 4))
        } else if source.starts_with("<?") {
            Some((Self::Processing, 2))
        } else if source.get(..9).is_some_and(|prefix| {
            prefix == "<![CDATA[" || mode == Mode::Block && prefix.eq_ignore_ascii_case("<![CDATA[")
        }) {
            Some((Self::Cdata, 9))
        } else {
            let body = source.strip_prefix("<!")?;
            let name = body.bytes().take_while(u8::is_ascii_alphabetic).count();
            let separator = body[name..].chars().next();
            (name > 0 && (mode == Mode::Block || separator.is_some_and(is_whitespace_scalar)))
                .then_some((Self::Declaration, 2))
        }
    }

    /// The closing delimiter at a normal body position.
    const fn ending(self) -> &'static str {
        match self {
            Self::Comment => "-->",
            Self::Processing => "?>",
            Self::Declaration => ">",
            Self::Cdata => "]]>",
        }
    }
}

impl<'a> Scanner<'a> {
    /// Bind the original input and its already recognized opening delimiter.
    fn new(source: &'a str, start: usize, mode: Mode) -> Option<Self> {
        let (directive, width) = Directive::opening(&source[start..], mode)?;
        let cursor = start + width;
        Some(Self {
            source,
            mode,
            directive,
            cursor,
            body_start: cursor,
            value: source[..cursor].to_owned(),
            tag: None,
            phase: Phase::Body,
        })
    }

    /// Advance monotonically through the original bytes, never parsing an encoded copy.
    fn scan(mut self) -> Option<Html> {
        while self.cursor < self.source.len() && self.phase != Phase::Done {
            let scalar = self.source[self.cursor..].chars().next()?;
            if self.phase == Phase::Newlines && scalar != '\n' {
                break;
            }
            if self.phase == Phase::Newlines {
                self.value.push(scalar);
                self.cursor += 1;
            } else if self.phase == Phase::Tail && scalar == '\n' {
                self.flush_unclosed();
                self.phase = Phase::Newlines;
            } else if self.tag.is_some() {
                self.tag_scalar(scalar);
            } else if self.phase == Phase::Body && self.close() {
                self.phase = match self.mode {
                    Mode::Inline => Phase::Done,
                    Mode::Block if self.directive == Directive::Comment => Phase::Tail,
                    Mode::Block => Phase::Newlines,
                };
            } else {
                self.literal_scalar(scalar);
            }
        }
        self.flush_unclosed();
        if self.mode == Mode::Inline && self.phase != Phase::Done {
            return None;
        }
        let value = if self.mode == Mode::Block {
            block_value(self.value)
        } else {
            self.value
        };
        Some(Html {
            consumed: self.cursor,
            value,
        })
    }

    /// Consume an eligible delimiter before ordinary body text.
    fn close(&mut self) -> bool {
        let rest = &self.source[self.cursor..];
        let ending = if self.directive == Directive::Comment && self.cursor == self.body_start {
            if rest.starts_with('>') {
                ">"
            } else if rest.starts_with("->") {
                "->"
            } else {
                self.directive.ending()
            }
        } else {
            self.directive.ending()
        };
        if !rest.starts_with(ending) {
            return false;
        }
        self.value.push_str(ending);
        self.cursor += ending.len();
        true
    }

    /// Encode a complete opening literal tag without exposing nested tag starts.
    fn tag_scalar(&mut self, scalar: char) {
        if let Some(tag) = &mut self.tag {
            tag.push(scalar);
        }
        self.cursor += scalar.len_utf8();
        if scalar == '>'
            && let Some(tag) = self.tag.take()
        {
            append_tag(&mut self.value, &tag);
        }
    }

    /// Select literal tags, closing tags and bare less-than spellings at this boundary.
    fn literal_scalar(&mut self, scalar: char) {
        match literal(&self.source[self.cursor..]) {
            Some(Literal::Opening) => {
                self.tag = Some("<".to_owned());
                self.cursor += 1;
            }
            Some(Literal::Closing(width)) => {
                append_tag(
                    &mut self.value,
                    &self.source[self.cursor..self.cursor + width],
                );
                self.cursor += width;
            }
            Some(Literal::Bare) => {
                self.value.push_str("&lt;");
                self.cursor += 1;
            }
            None => {
                self.value.push(scalar);
                self.cursor += scalar.len_utf8();
            }
        }
    }

    /// Escape unfinished letter-led tag starts without inventing a closing delimiter.
    fn flush_unclosed(&mut self) {
        let Some(tag) = self.tag.take() else { return };
        let mut scalars = tag.chars().peekable();
        while let Some(scalar) = scalars.next() {
            let escape = scalar == '<'
                && scalars
                    .peek()
                    .is_none_or(|next| next.is_ascii_alphabetic() || is_whitespace_scalar(*next));
            if escape {
                self.value.push_str("&lt;");
            } else {
                self.value.push(scalar);
            }
        }
    }
}

/// Literal-angle policy shared by directive recognition and inline masks.
enum Literal {
    /// A word-led opening tag, completed or unfinished.
    Opening,
    /// A complete simple closing tag, including both angles.
    Closing(usize),
    /// A less-than sign before whitespace or the end of input.
    Bare,
}

/// Classify only the tag shapes the caller displays literally.
fn literal(source: &str) -> Option<Literal> {
    let rest = source.strip_prefix('<')?;
    let next = rest.chars().next();
    if next.is_some_and(|scalar| scalar.is_ascii_alphanumeric() || scalar == '_') {
        return Some(Literal::Opening);
    }
    if let Some(name) = rest.strip_prefix('/') {
        let width = name
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            .count();
        if width > 0 && name.as_bytes().get(width) == Some(&b'>') {
            return Some(Literal::Closing(width + 3));
        }
    }
    next.is_none_or(is_whitespace_scalar)
        .then_some(Literal::Bare)
}

/// Encode the angles of one literal tag while retaining its other authored bytes.
fn append_tag(output: &mut String, tag: &str) {
    for scalar in tag.chars() {
        match scalar {
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(scalar),
        }
    }
}

/// Hide caller-escaped angle delimiters only in the emphasis mask's coordinate space, with
/// the spans the scan consumed whole.
pub(super) fn literal_mask(source: &str) -> (String, Vec<std::ops::Range<usize>>) {
    let mut bytes = source.as_bytes().to_vec();
    let mut spans = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find('<') {
        let start = cursor + relative;
        if let Some(angle) = crate::rules::INLINE_AUTOLINK
            .as_ref()
            .and_then(|rule| rule.find(&source[start..]))
        {
            cursor = start + angle.end();
            spans.push(start..cursor);
            continue;
        }
        match literal(&source[start..]) {
            Some(Literal::Opening) => {
                let Some(end) = source[start..].find('>') else {
                    break;
                };
                bytes[start] = b'a';
                bytes[start + end] = b'a';
                cursor = start + end + 1;
            }
            Some(Literal::Closing(width)) => {
                bytes[start] = b'a';
                bytes[start + width - 1] = b'a';
                cursor = start + width;
            }
            _ => cursor = start + 1,
        }
        spans.push(start..cursor);
    }
    for (offset, scalar) in source.char_indices() {
        if scalar == '<' && matches!(literal(&source[offset..]), Some(Literal::Bare)) {
            bytes[offset] = b'a';
        }
    }
    let masked = String::from_utf8(bytes).unwrap_or_else(|_| source.to_owned());
    (masked, spans)
}
