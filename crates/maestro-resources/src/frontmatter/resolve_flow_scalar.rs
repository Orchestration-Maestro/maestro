//! Scalar text, line folding and Unicode-safe double-quoted escapes.

use super::{
    FrontmatterError,
    parser::{Mode, Parser},
};

/// Whether ordinary schema resolution applies to untagged text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Style {
    /// Unquoted text uses the core scalar schema.
    Plain,
    /// Quoted and block text remain strings without a selected tag.
    Quoted,
}

impl Parser<'_> {
    /// Decode scalar text exactly once at its lexical boundary.
    pub fn flow_scalar(
        &mut self,
        indent: usize,
        mode: Mode,
    ) -> Result<(String, Style), FrontmatterError> {
        match self.cursor.peek() {
            Some('\'' | '"') => Ok((self.quoted()?, Style::Quoted)),
            Some('@' | '`' | '%' | ',' | ']' | '}' | '|' | '>') => {
                Err(self.cursor.error("Invalid plain scalar start"))
            }
            _ => Ok((self.plain(indent, mode)?, Style::Plain)),
        }
    }

    /// Read one plain scalar, leaving collection punctuation unconsumed.
    fn plain(&mut self, indent: usize, mode: Mode) -> Result<String, FrontmatterError> {
        let mut text = String::new();
        let mut separated = true;
        while let Some(ch) = self.cursor.peek() {
            if ch == '#' && separated
                || mode == Mode::Flow && matches!(ch, ',' | '[' | ']' | '{' | '}')
            {
                break;
            }
            if ch == ':'
                && (self.cursor.indicator(':')
                    || mode == Mode::Flow && self.cursor.rest()[1..].starts_with([',', ']', '}']))
            {
                break;
            }
            if ch == '\n' && mode == Mode::Key {
                break;
            }
            if ch == '\n' && !self.plain_continuation(indent, mode, &mut text)? {
                break;
            }
            if ch == '\n' {
                separated = true;
                continue;
            }
            separated = matches!(ch, ' ' | '\t');
            text.push(ch);
            self.cursor.next();
        }
        text.truncate(text.trim_end_matches([' ', '\t']).len());
        Ok(text)
    }

    /// Consume a folded continuation only while it belongs to this scalar.
    fn plain_continuation(
        &mut self,
        indent: usize,
        mode: Mode,
        text: &mut String,
    ) -> Result<bool, FrontmatterError> {
        let saved = self.cursor.offset;
        self.cursor.trivia();
        if self.cursor.peek().is_none()
            || self.cursor.document_end()
            || mode != Mode::Flow && self.cursor.column() <= indent
        {
            return Ok(false);
        }
        if mode != Mode::Flow && self.cursor.mapping_start() {
            return Err(self
                .cursor
                .error("Unexpected mapping in scalar continuation"));
        }
        let breaks = self.cursor.source[saved..self.cursor.offset]
            .bytes()
            .filter(|b| *b == b'\n')
            .count();
        text.truncate(text.trim_end_matches([' ', '\t']).len());
        if breaks == 1 {
            text.push(' ');
        } else {
            text.extend(std::iter::repeat_n('\n', breaks - 1));
        }
        Ok(true)
    }

    /// Read a quoted scalar and enforce separation after its closing quote.
    fn quoted(&mut self) -> Result<String, FrontmatterError> {
        let start = self.cursor.offset;
        let quote = self
            .cursor
            .next()
            .ok_or_else(|| self.cursor.error("Missing quote"))?;
        let mut text = String::new();
        loop {
            let Some(ch) = self.cursor.next() else {
                return Err(self.cursor.error_at(start, "Unclosed quoted scalar"));
            };
            if ch == quote && quote == '\'' && self.cursor.eat('\'') {
                text.push('\'');
                continue;
            }
            if ch == quote {
                break;
            }
            if ch == '\\' && quote == '"' {
                text.extend(self.escape(start)?);
            } else if ch == '\n' {
                text.truncate(text.trim_end_matches([' ', '\t']).len());
                self.fold_newline(&mut text);
            } else {
                text.push(ch);
            }
        }
        let end = self.cursor.offset;
        self.cursor.spaces();
        if self.cursor.peek() == Some('#') && end == self.cursor.offset {
            return Err(self.cursor.error("Expected separation before comment"));
        }
        if self
            .cursor
            .peek()
            .is_some_and(|c| !matches!(c, '\n' | '#' | ':' | ',' | ']' | '}'))
        {
            let kind = if quote == '"' { "double" } else { "single" };
            return Err(self.cursor.error(&format!(
                "invalid trailing content after {kind}-quoted scalar"
            )));
        }
        Ok(text)
    }

    /// Fold a consumed ordinary newline and following indentation.
    fn fold_newline(&mut self, text: &mut String) {
        let mut breaks = 0;
        loop {
            self.cursor.spaces();
            if !self.cursor.eat('\n') {
                break;
            }
            breaks += 1;
        }
        if breaks == 0 {
            text.push(' ');
        } else {
            text.extend(std::iter::repeat_n('\n', breaks));
        }
    }

    /// Decode one escape, reporting native failures at the scalar's opening quote.
    fn escape(&mut self, start: usize) -> Result<Option<char>, FrontmatterError> {
        let ch = self
            .cursor
            .next()
            .ok_or_else(|| self.escape_error(start, "found unknown escape character"))?;
        let decoded = match ch {
            '0' => '\0',
            'a' => '\u{7}',
            'b' => '\u{8}',
            't' | '\t' => '\t',
            'n' => '\n',
            'v' => '\u{b}',
            'f' => '\u{c}',
            'r' => '\r',
            'e' => '\u{1b}',
            ' ' | '"' | '/' | '\\' => ch,
            'N' => '\u{85}',
            '_' => '\u{a0}',
            'L' => '\u{2028}',
            'P' => '\u{2029}',
            '\n' => {
                self.cursor.spaces();
                return Ok(None);
            }
            'x' | 'u' | 'U' => return self.unicode_escape(ch, start).map(Some),
            _ => return Err(self.escape_error(start, "found unknown escape character")),
        };
        Ok(Some(decoded))
    }

    /// Read hexadecimal digits without accepting partial escapes.
    fn hex(&mut self, width: usize) -> Option<u32> {
        let end = self.cursor.offset.checked_add(width)?;
        let digits = self.cursor.source.get(self.cursor.offset..end)?;
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        self.cursor.offset = end;
        u32::from_str_radix(digits, 16).ok()
    }

    /// Decode a scalar or an adjacent pair of four-digit surrogate escapes.
    fn unicode_escape(&mut self, kind: char, start: usize) -> Result<char, FrontmatterError> {
        let width = match kind {
            'x' => 2,
            'u' => 4,
            _ => 8,
        };
        let value = self
            .hex(width)
            .ok_or_else(|| self.escape_error(start, "did not find expected hexadecimal number"))?;
        let invalid = || self.escape_error(start, "found invalid Unicode character escape code");
        if kind == 'u' && (0xd800..=0xdbff).contains(&value) {
            if !self.cursor.rest().starts_with("\\u") {
                return Err(invalid());
            }
            self.cursor.offset += 2;
            let low = self
                .hex(4)
                .filter(|v| (0xdc00..=0xdfff).contains(v))
                .ok_or_else(|| {
                    self.escape_error(start, "found invalid Unicode character escape code")
                })?;
            return char::from_u32(0x10000 + ((value - 0xd800) << 10) + low - 0xdc00).ok_or_else(
                || self.escape_error(start, "found invalid Unicode character escape code"),
            );
        }
        char::from_u32(value).ok_or_else(invalid)
    }

    /// Preserve existing quoted-scalar diagnostic templates.
    fn escape_error(&self, start: usize, cause: &str) -> FrontmatterError {
        self.cursor
            .error_at(start, &format!("while parsing a quoted scalar, {cause}"))
    }
}
