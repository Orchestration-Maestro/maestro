//! Borrowed source traversal and native diagnostic coordinates.

use super::FrontmatterError;

/// An advancing UTF-8 source position.
pub(super) struct Cursor<'a> {
    /// Complete extracted document.
    pub source: &'a str,
    /// Next byte boundary.
    pub offset: usize,
}

impl Cursor<'_> {
    /// Remaining authored text.
    pub fn rest(&self) -> &str {
        &self.source[self.offset..]
    }

    /// Next scalar, without consuming it.
    pub fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    /// Advance by one complete scalar.
    pub fn next(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }

    /// Consume an expected scalar.
    pub fn eat(&mut self, ch: char) -> bool {
        if self.peek() == Some(ch) {
            self.next();
            true
        } else {
            false
        }
    }

    /// Skip horizontal lexical separation only.
    pub fn spaces(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.next();
        }
    }

    /// Skip the remainder of an authored comment.
    pub fn comment(&mut self) {
        if self.peek() == Some('#') {
            while !matches!(self.peek(), None | Some('\n')) {
                self.next();
            }
        }
    }

    /// Skip separated comments, blank lines and indentation.
    pub fn trivia(&mut self) {
        loop {
            self.spaces();
            self.comment();
            if !self.eat('\n') {
                break;
            }
        }
    }

    /// Current character column, starting at zero.
    pub fn column(&self) -> usize {
        self.source[..self.offset]
            .rsplit('\n')
            .next()
            .map_or(0, |s| s.chars().count())
    }

    /// Whether the cursor follows only indentation on its line.
    pub fn at_indent(&self) -> bool {
        self.source[..self.offset]
            .rsplit('\n')
            .next()
            .is_none_or(|s| s.bytes().all(|b| b == b' '))
    }

    /// Recognize an indicator requiring lexical separation.
    pub fn indicator(&self, ch: char) -> bool {
        self.rest()
            .strip_prefix(ch)
            .is_some_and(|s| s.starts_with([' ', '\t', '\n']) || s.is_empty())
    }

    /// Recognize a document boundary rather than ordinary dotted text.
    pub fn document_end(&self) -> bool {
        self.column() == 0
            && self
                .rest()
                .strip_prefix("...")
                .is_some_and(|s| s.is_empty() || s.starts_with([' ', '\t', '\n']))
    }

    /// Attach source coordinates to an error at a byte boundary.
    pub fn error_at(&self, offset: usize, message: &str) -> FrontmatterError {
        let prefix = &self.source[..offset];
        let eof_line = offset == self.source.len() && !prefix.is_empty() && !prefix.ends_with('\n');
        let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1 + usize::from(eof_line);
        let column = if eof_line {
            1
        } else {
            prefix.rsplit('\n').next().map_or(0, |s| s.chars().count()) + 1
        };
        FrontmatterError {
            message: format!("{message} at byte {offset} line {line} column {column}"),
            line: Some(line),
            column: Some(column),
        }
    }

    /// Attach the current source position to a grammar error.
    pub fn error(&self, message: &str) -> FrontmatterError {
        self.error_at(self.offset, message)
    }

    /// Locate a block mapping separator without interpreting scalar contents.
    pub fn mapping_start(&self) -> bool {
        if self.indicator('?') || self.indicator(':') {
            return true;
        }
        let mut quote = None;
        let mut depth = 0usize;
        let mut chars = self.rest().chars().peekable();
        while let Some(ch) = chars.next() {
            match (quote, ch) {
                (Some('"'), '\\') => {
                    chars.next();
                    continue;
                }
                (Some(q), c) if q == c => {
                    quote = None;
                    continue;
                }
                (Some(_), _) => continue,
                _ => (),
            }
            match ch {
                '\n' | '#' if depth == 0 => return false,
                '\'' | '"' => quote = Some(ch),
                '[' | '{' => depth += 1,
                ']' | '}' => depth = depth.saturating_sub(1),
                ':' if depth == 0
                    && chars.peek().is_none_or(|c| matches!(c, ' ' | '\t' | '\n')) =>
                {
                    return true;
                }
                _ => (),
            }
        }
        false
    }
}
