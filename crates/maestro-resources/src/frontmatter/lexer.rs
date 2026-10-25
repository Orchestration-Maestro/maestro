//! Single lexical owner for document boundaries and scalar extent.

mod quoted_scalar;

use super::cst::{SourceToken, TokenKind as K};

/// Context resumed after a complete lexical unit.
#[derive(Clone, Copy)]
enum State {
    /// Outside a document.
    Stream,
    /// Beginning of a block line.
    LineStart,
    /// Block document content.
    Doc,
    /// Flow collection content.
    Flow,
}

/// Complete-input lexer with monotone byte traversal.
#[derive(Default)]
pub(super) struct Lexer<'a> {
    /// Complete normalized document text.
    source: &'a str,
    /// Next unread byte.
    pos: usize,
    /// Minimum continuation indentation.
    indent_next: usize,
    /// Current line indentation.
    indent_value: usize,
    /// Number of open flow delimiters.
    flow_level: usize,
    /// Whether a colon may immediately follow a quoted key or closer.
    flow_key: bool,
    /// Emitted lexical units.
    tokens: Vec<SourceToken>,
}

impl<'a> Lexer<'a> {
    /// Split one complete document into lexical units.
    pub fn lex(source: &'a str) -> Vec<SourceToken> {
        let mut lexer = Self {
            source,
            ..Self::default()
        };
        let mut state = State::Stream;
        while lexer.pos < source.len() {
            state = match state {
                State::Stream => lexer.parse_stream(),
                State::LineStart => lexer.parse_line_start(),
                State::Doc => lexer.parse_document(),
                State::Flow => lexer.parse_flow_collection(),
            };
        }
        lexer.tokens
    }

    /// Byte at an absolute position, including end-of-input.
    fn byte(&self, at: usize) -> Option<u8> {
        self.source.as_bytes().get(at).copied()
    }

    /// Emit a known complete range and advance once.
    fn push(&mut self, kind: K, end: usize) {
        self.tokens.push(SourceToken::new(kind, self.pos..end));
        self.pos = end;
    }

    /// Emit a zero-length context marker.
    fn marker(&mut self, kind: K) {
        self.push(kind, self.pos);
    }

    /// Consume a run of horizontal separation.
    fn push_spaces(&mut self, tabs: bool) -> usize {
        let start = self.pos;
        let mut end = start;
        while self.byte(end) == Some(b' ') || tabs && self.byte(end) == Some(b'\t') {
            end += 1;
        }
        if end > start {
            self.push(K::Space, end);
        }
        end - start
    }

    /// Emit a newline if present.
    fn push_newline(&mut self) -> bool {
        let newline = self.byte(self.pos) == Some(b'\n');
        if newline {
            self.push(K::Newline, self.pos + 1);
        }
        newline
    }

    /// Byte immediately before the next newline or input end.
    fn line_end(&self) -> usize {
        self.source[self.pos..]
            .find('\n')
            .map_or(self.source.len(), |n| self.pos + n)
    }

    /// Emit comments or directives through the line end.
    fn push_line(&mut self, kind: K) {
        let kind = if kind == K::Comment && self.byte(self.pos) != Some(b'#') {
            K::Error
        } else {
            kind
        };
        self.push(kind, self.line_end());
    }

    /// Process stream trivia, directives and implicit document starts.
    fn parse_stream(&mut self) -> State {
        if self.source[self.pos..].starts_with('\u{feff}') {
            self.push(K::ByteOrderMark, self.pos + 3);
        }
        if self.byte(self.pos) == Some(b'%') {
            self.push_line(K::Directive);
            return State::Stream;
        }
        let mut at = self.pos;
        while matches!(self.byte(at), Some(b' ' | b'\t')) {
            at += 1;
        }
        if matches!(self.byte(at), None | Some(b'#' | b'\n')) {
            self.push_spaces(true);
            if self.byte(self.pos) == Some(b'#') {
                self.push_line(K::Comment);
            }
            self.push_newline();
            return State::Stream;
        }
        self.marker(K::DocMode);
        State::LineStart
    }

    /// Recognize document markers and block indentation exactly once.
    fn parse_line_start(&mut self) -> State {
        for (text, kind, next) in [
            ("---", K::DocStart, State::Doc),
            ("...", K::DocEnd, State::Stream),
        ] {
            if self.source[self.pos..].starts_with(text) && is_empty(self.byte(self.pos + 3)) {
                self.push(kind, self.pos + 3);
                self.indent_value = 0;
                self.indent_next = 0;
                return next;
            }
        }
        self.indent_value = self.push_spaces(false);
        if self.indent_next > self.indent_value && !is_empty(self.byte(self.pos + 1)) {
            self.indent_next = self.indent_value;
        }
        while matches!(self.byte(self.pos), Some(b'-' | b'?' | b':'))
            && is_empty(self.byte(self.pos + 1))
        {
            self.push_indicator();
            let count = 1 + self.push_spaces(true);
            self.indent_next = self.indent_value + 1;
            self.indent_value += count;
        }
        State::Doc
    }

    /// Recognize one block-context lexical unit.
    fn parse_document(&mut self) -> State {
        self.push_spaces(true);
        self.push_indicators();
        match self.byte(self.pos) {
            None => State::Doc,
            Some(b'#') => {
                self.push_line(K::Comment);
                self.push_newline();
                State::LineStart
            }
            Some(b'\n') => {
                self.push_newline();
                State::LineStart
            }
            Some(b'[' | b'{') => {
                self.push_indicator();
                self.flow_key = false;
                self.flow_level = 1;
                State::Flow
            }
            Some(b']' | b'}') => {
                self.push_indicator();
                State::Doc
            }
            Some(b'*') => {
                self.push_name(K::Alias);
                State::Doc
            }
            Some(b'\'' | b'"') => {
                self.parse_quoted_scalar();
                State::Doc
            }
            Some(b'|' | b'>') => {
                self.parse_block_scalar();
                State::LineStart
            }
            _ => {
                self.parse_plain_scalar();
                State::Doc
            }
        }
    }

    /// Recognize one flow-context unit and invalid unindent.
    fn parse_flow_collection(&mut self) -> State {
        if !self.flow_line_start() {
            return State::LineStart;
        }
        while self.byte(self.pos) == Some(b',') {
            self.push_indicator();
            self.push_spaces(true);
            self.flow_key = false;
        }
        self.push_indicators();
        match self.byte(self.pos) {
            None | Some(b'\n') => State::Flow,
            Some(b'#') => {
                self.push_line(K::Comment);
                State::Flow
            }
            Some(b'[' | b'{') => {
                self.push_indicator();
                self.flow_key = false;
                self.flow_level += 1;
                State::Flow
            }
            Some(b']' | b'}') => {
                self.push_indicator();
                self.flow_key = true;
                self.flow_level -= 1;
                if self.flow_level == 0 {
                    State::Doc
                } else {
                    State::Flow
                }
            }
            Some(b'*') => {
                self.push_name(K::Alias);
                State::Flow
            }
            Some(b'\'' | b'"') => {
                self.flow_key = true;
                self.parse_quoted_scalar();
                State::Flow
            }
            Some(b':')
                if self.flow_key
                    || is_empty(self.byte(self.pos + 1))
                    || self.byte(self.pos + 1) == Some(b',') =>
            {
                self.flow_key = false;
                self.push_indicator();
                self.push_spaces(true);
                State::Flow
            }
            _ => {
                self.flow_key = false;
                self.parse_plain_scalar();
                State::Flow
            }
        }
    }

    /// Consume flow line trivia and reject an invalid unindent.
    fn flow_line_start(&mut self) -> bool {
        let mut indent = None;
        loop {
            let newline = self.push_newline();
            if newline {
                self.indent_value = self.push_spaces(false);
                indent = Some(self.indent_value);
            }
            let spaces = self.push_spaces(true);
            if !newline && spaces == 0 {
                break;
            }
        }
        if let Some(indent) = indent {
            let marker = indent == 0
                && ["---", "..."].iter().any(|s| {
                    self.source[self.pos..].starts_with(s) && is_empty(self.byte(self.pos + 3))
                });
            let unindent = indent < self.indent_next && self.byte(self.pos) != Some(b'#');
            let closer = indent + 1 == self.indent_next
                && self.flow_level == 1
                && matches!(self.byte(self.pos), Some(b']' | b'}'));
            if marker || unindent && !closer {
                self.flow_level = 0;
                self.marker(K::FlowErrorEnd);
                return false;
            }
        }
        true
    }

    /// Classify punctuation whose extent is one byte.
    fn push_indicator(&mut self) {
        let kind = match self.byte(self.pos) {
            Some(b'-') => K::SeqItemInd,
            Some(b'?') => K::ExplicitKeyInd,
            Some(b':') => K::MapValueInd,
            Some(b'[') => K::FlowSeqStart,
            Some(b']') => K::FlowSeqEnd,
            Some(b'{') => K::FlowMapStart,
            Some(b'}') => K::FlowMapEnd,
            _ => K::Comma,
        };
        self.push(kind, self.pos + 1);
    }

    /// Consume properties and separated indicators without recursion.
    fn push_indicators(&mut self) {
        loop {
            match self.byte(self.pos) {
                Some(b'!') => self.push_tag(),
                Some(b'&') => self.push_name(K::Anchor),
                Some(b'-' | b'?' | b':')
                    if is_empty(self.byte(self.pos + 1))
                        || self.flow_level > 0 && is_flow_indicator(self.byte(self.pos + 1)) =>
                {
                    self.push_block_indicator();
                }
                _ => return,
            }
            self.push_spaces(true);
        }
    }

    /// Update continuation state for a separated structural indicator.
    fn push_block_indicator(&mut self) {
        if self.flow_level == 0 {
            self.indent_next = self.indent_value + 1;
        } else {
            self.flow_key = false;
        }
        self.push_indicator();
    }

    /// Consume an anchor or alias name through its lexical delimiter.
    fn push_name(&mut self, kind: K) {
        let mut end = self.pos + 1;
        while !is_empty(self.byte(end)) && !is_flow_indicator(self.byte(end)) {
            end += 1;
        }
        self.push(kind, end);
    }

    /// Consume a verbatim tag or shorthand URI token.
    fn push_tag(&mut self) {
        let mut end = self.pos + 1;
        if self.byte(end) == Some(b'<') {
            end += 1;
            while !is_empty(self.byte(end)) && self.byte(end) != Some(b'>') {
                end += 1;
            }
            if self.byte(end) == Some(b'>') {
                end += 1;
            }
        } else {
            end = self.shorthand_tag_end(end);
        }
        self.push(K::Tag, end);
    }

    /// Find a shorthand URI token's end with complete percent triplets.
    fn shorthand_tag_end(&self, mut end: usize) -> usize {
        while let Some(byte) = self.byte(end) {
            if byte.is_ascii_alphanumeric() || b"-#;/?:@&=+$_.!~*'()".contains(&byte) {
                end += 1;
            } else if byte == b'%'
                && self.byte(end + 1).is_some_and(|b| b.is_ascii_hexdigit())
                && self.byte(end + 2).is_some_and(|b| b.is_ascii_hexdigit())
            {
                end += 3;
            } else {
                break;
            }
        }
        end
    }

    /// Determine whether a newline continues the current scalar.
    fn continue_scalar(&self, mut at: usize) -> Option<usize> {
        if self.indent_next > 0 {
            let start = at;
            while self.byte(at) == Some(b' ') {
                at += 1;
            }
            return (self.byte(at) == Some(b'\n') || at - start >= self.indent_next).then_some(at);
        }
        if ["---", "..."]
            .iter()
            .any(|s| self.source[at..].starts_with(s) && is_empty(self.byte(at + 3)))
        {
            None
        } else {
            Some(at)
        }
    }

    /// Find plain scalar extent, retaining internal whitespace but no trailing blanks.
    fn parse_plain_scalar(&mut self) {
        let mut at = self.pos;
        let mut end = self.pos;
        while let Some(byte) = self.byte(at) {
            let next = self.byte(at + 1);
            let empty = is_empty(Some(byte));
            let colon_end =
                byte == b':' && (is_empty(next) || self.flow_level > 0 && is_flow_indicator(next));
            let comment_end = empty && next == Some(b'#');
            let flow_end = self.flow_level > 0
                && (is_flow_indicator(Some(byte)) || empty && is_flow_indicator(next));
            if colon_end || comment_end || flow_end {
                break;
            }
            if !empty {
                end = at + 1;
            }
            if byte == b'\n'
                && let Some(next) = self.continue_scalar(at + 1)
            {
                at = at.max(next.saturating_sub(2));
            } else if byte == b'\n' {
                break;
            }
            at += 1;
        }
        self.push(K::Scalar, end);
    }

    /// Emit the header, header trivia and complete block scalar body.
    fn parse_block_scalar(&mut self) {
        let mut at = self.pos + 1;
        let mut explicit = None;
        let mut keep = false;
        while let Some(byte) = self.byte(at) {
            match byte {
                b'+' => keep = true,
                b'1'..=b'9' => explicit = Some(usize::from(byte - b'1')),
                b'-' => (),
                _ => break,
            }
            at += 1;
        }
        let mut header_end = self.pos + 1;
        while !is_empty(self.byte(header_end)) && self.byte(header_end) != Some(b'#') {
            header_end += 1;
        }
        self.push(K::BlockScalarHeader, header_end);
        self.push_spaces(true);
        if self.pos < self.line_end() {
            self.push_line(K::Comment);
        }
        self.push_newline();
        let start = self.pos;
        let (end, indent) = self.block_scalar_end(explicit);
        let end = self.block_scalar_tail(start, end, indent, keep);
        self.push(K::Scalar, end);
    }

    /// Find block content through the last sufficiently indented line.
    fn block_scalar_end(&mut self, explicit: Option<usize>) -> (usize, usize) {
        let start = self.pos;
        let mut indent = 0;
        let mut at = start;
        while let Some(byte) = self.byte(at) {
            match byte {
                b' ' => indent += 1,
                b'\n' => indent = 0,
                _ => break,
            }
            at += 1;
        }
        let minimum = explicit.map_or(indent, |n| n + self.indent_next.max(1));
        if indent < self.indent_next {
            return (
                self.source[start..at]
                    .rfind('\n')
                    .map_or(start, |n| start + n + 1),
                indent,
            );
        }
        self.indent_next = minimum;
        let mut end = start;
        while end < self.source.len() {
            let Some(content) = self.continue_scalar(end) else {
                break;
            };
            end = self.source[content..]
                .find('\n')
                .map_or(self.source.len(), |n| content + n + 1);
        }
        (end, indent)
    }

    /// Include invalid trailing tabs or trim unkept trailing blank lines.
    fn block_scalar_tail(&self, start: usize, mut end: usize, indent: usize, keep: bool) -> usize {
        let mut tail = end;
        while self.byte(tail) == Some(b' ') {
            tail += 1;
        }
        if self.byte(tail) == Some(b'\t') {
            while matches!(self.byte(tail), Some(b' ' | b'\t' | b'\n')) {
                tail += 1;
            }
            return tail;
        }
        if keep {
            return end;
        }
        while end > start && self.byte(end - 1) == Some(b'\n') {
            let previous = self.source[start..end - 1]
                .rfind('\n')
                .map_or(start, |n| start + n + 1);
            let line = &self.source[previous..end - 1];
            if previous == start || !line.bytes().all(|b| b == b' ') || line.len() > indent {
                break;
            }
            end = previous;
        }
        end
    }
}

/// YAML lexical whitespace, including end-of-input.
fn is_empty(byte: Option<u8>) -> bool {
    matches!(byte, None | Some(b' ' | b'\t' | b'\n'))
}

/// Flow punctuation which terminates unquoted content.
fn is_flow_indicator(byte: Option<u8>) -> bool {
    byte.is_some_and(|b| b",[]{}".contains(&b))
}
