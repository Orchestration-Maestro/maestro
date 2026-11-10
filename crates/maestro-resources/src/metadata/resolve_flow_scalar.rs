//! Decode scalar token contents without finding lexical boundaries.

use super::{
    FrontmatterError,
    composer::Composer,
    cst::{SourceToken, TokenKind as K},
};

/// Cause reported for an escape that names no Unicode scalar.
const INVALID_ESCAPE: &str = "found invalid Unicode character escape code";

/// Drop literal trailing blanks, never reaching into decoded escapes before `floor`.
fn trim_blanks(text: &mut String, floor: usize) {
    text.truncate(text.trim_end_matches([' ', '\t']).len().max(floor));
}

/// Whether ordinary schema resolution applies to untagged text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Style {
    /// Unquoted text uses the core scalar schema.
    Plain,
    /// Quoted and block text remain strings without a selected tag.
    Quoted,
}

impl Composer<'_> {
    /// Decode one complete scalar token with no access to neighbouring text.
    pub fn resolve_flow_scalar(
        &self,
        token: &SourceToken,
    ) -> Result<(String, Style), FrontmatterError> {
        let text = self.text(token);
        if token.kind == K::Scalar {
            if text.starts_with(['\t', ',', '%', '|', '>', '@', '`']) {
                return Err(self.error_at(token.offset, "Invalid plain scalar start"));
            }
            return Ok((fold_lines(text), Style::Plain));
        }
        let quote = if token.kind == K::SingleQuotedScalar {
            '\''
        } else {
            '"'
        };
        let inner = text
            .strip_prefix(quote)
            .and_then(|text| text.strip_suffix(quote))
            .ok_or_else(|| self.error_at(token.offset, "Unclosed quoted scalar"))?;
        if quote == '\'' {
            return Ok((fold_lines(inner).replace("''", "'"), Style::Quoted));
        }
        let mut decoder = Decoder {
            chars: inner.chars(),
            composer: self,
            offset: token.offset,
        };
        let mut value = String::new();
        let mut escaped = 0;
        while let Some(ch) = decoder.chars.next() {
            match ch {
                '\\' => {
                    value.extend(decoder.escape()?);
                    escaped = value.len();
                }
                '\n' => {
                    trim_blanks(&mut value, escaped);
                    decoder.fold_newline(&mut value);
                }
                _ => value.push(ch),
            }
        }
        Ok((value, Style::Quoted))
    }
}

/// Fold complete plain or single-quoted token lines.
fn fold_lines(text: &str) -> String {
    let Some((first, rest)) = text.split_once('\n') else {
        return text.into();
    };
    let mut value = first.trim_end_matches([' ', '\t']).to_owned();
    let mut sep = " ";
    let mut lines = rest.split('\n').peekable();
    while let Some(line) = lines.next() {
        let line = line.trim_start_matches([' ', '\t']);
        if lines.peek().is_none() {
            value.push_str(sep);
            value.push_str(line);
            break;
        }
        let line = line.trim_end_matches([' ', '\t']);
        if line.is_empty() {
            if sep == "\n" {
                value.push('\n');
            } else {
                sep = "\n";
            }
        } else {
            value.push_str(sep);
            value.push_str(line);
            sep = " ";
        }
    }
    value
}

/// Double-quoted token-local decoding state.
struct Decoder<'a, 'b> {
    /// Only the contents of the current scalar token.
    chars: std::str::Chars<'a>,
    /// Source-coordinate formatter.
    composer: &'b Composer<'a>,
    /// Opening quote's byte position.
    offset: usize,
}

impl Decoder<'_, '_> {
    /// Consume literal indentation inside this token.
    fn spaces(&mut self) {
        while self
            .chars
            .clone()
            .next()
            .is_some_and(|ch| matches!(ch, ' ' | '\t'))
        {
            self.chars.next();
        }
    }

    /// Fold a consumed ordinary newline and following indentation.
    fn fold_newline(&mut self, text: &mut String) {
        let mut breaks = 0;
        loop {
            self.spaces();
            if self.chars.clone().next() != Some('\n') {
                break;
            }
            self.chars.next();
            breaks += 1;
        }
        if breaks == 0 {
            text.push(' ');
        } else {
            text.extend(std::iter::repeat_n('\n', breaks));
        }
    }

    /// Decode one escape, reporting failures at the scalar's opening quote.
    fn escape(&mut self) -> Result<Option<char>, FrontmatterError> {
        let ch = self
            .chars
            .next()
            .ok_or_else(|| self.escape_error("found unknown escape character"))?;
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
                self.spaces();
                return Ok(None);
            }
            'x' | 'u' | 'U' => return self.unicode_escape(ch).map(Some),
            _ => return Err(self.escape_error("found unknown escape character")),
        };
        Ok(Some(decoded))
    }

    /// Read complete hexadecimal escapes from the token-local character iterator.
    fn hex(&mut self, width: usize) -> Option<u32> {
        let mut value = 0;
        for _ in 0..width {
            let ch = self.chars.next()?;
            if !ch.is_ascii_hexdigit() {
                return None;
            }
            value = value * 16 + ch.to_digit(16)?;
        }
        Some(value)
    }

    /// Decode a scalar or adjacent pair of four-digit surrogate escapes.
    fn unicode_escape(&mut self, kind: char) -> Result<char, FrontmatterError> {
        let width = match kind {
            'x' => 2,
            'u' => 4,
            _ => 8,
        };
        let value = self
            .hex(width)
            .ok_or_else(|| self.escape_error("did not find expected hexadecimal number"))?;
        if kind == 'u' && (0xd800..=0xdbff).contains(&value) {
            if self.chars.next() != Some('\\') || self.chars.next() != Some('u') {
                return Err(self.escape_error(INVALID_ESCAPE));
            }
            let low = self
                .hex(4)
                .filter(|v| (0xdc00..=0xdfff).contains(v))
                .ok_or_else(|| self.escape_error(INVALID_ESCAPE))?;
            return char::from_u32(0x10000 + ((value - 0xd800) << 10) + low - 0xdc00)
                .ok_or_else(|| self.escape_error(INVALID_ESCAPE));
        }
        char::from_u32(value).ok_or_else(|| self.escape_error(INVALID_ESCAPE))
    }

    /// Preserve existing quoted-scalar diagnostic templates.
    fn escape_error(&self, cause: &str) -> FrontmatterError {
        self.composer.error_at(
            self.offset,
            &format!("while parsing a quoted scalar, {cause}"),
        )
    }
}
