//! Quoted scalar boundaries and continuation admission.

use super::{K, Lexer};

impl Lexer<'_> {
    /// Find a quoted token's extent without decoding its contents.
    pub(super) fn parse_quoted_scalar(&mut self) {
        let quote = self.byte(self.pos);
        let mut end = self.pos + 1;
        let mut closed = false;
        while let Some(byte) = self.byte(end) {
            if Some(byte) == quote && quote == Some(b'\'') && self.byte(end + 1) == quote {
                end += 2;
                continue;
            }
            if Some(byte) == quote {
                end += 1;
                closed = true;
                break;
            }
            if byte == b'\\'
                && quote == Some(b'"')
                && self.byte(end + 1).is_some_and(|next| next != b'\n')
            {
                end += 2;
                continue;
            }
            if byte == b'\n' && self.continue_scalar(end + 1).is_none() {
                break;
            }
            end += 1;
        }
        self.push(
            if !closed {
                K::UnclosedQuotedScalar
            } else if quote == Some(b'\'') {
                K::SingleQuotedScalar
            } else {
                K::DoubleQuotedScalar
            },
            end,
        );
    }
}
