//! Owned parser entry points with immutable caller grammar.
use crate::{BlockToken, Document, MarkedOptions, lexer::Lexer};
use std::{convert::Infallible, error::Error, fmt};

/// One caller-owned block grammar extension.
pub trait BlockTokenizer {
    /// Find the next possible block start, as a UTF-8 byte offset in `source`.
    fn start(&self, source: &str) -> Option<usize>;
    /// Admit a token at the beginning of `source`, or let built-in grammar continue.
    fn tokenize(&mut self, source: &str) -> Option<BlockToken>;
}

/// An extension returned a non-prefix token or an invalid UTF-8 start hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidBlockToken;
impl fmt::Display for InvalidBlockToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("block tokenizer returned an invalid input prefix or start boundary")
    }
}
impl Error for InvalidBlockToken {}

/// A validated extension port with an entry-specific error type.
pub(crate) trait Extension<E> {
    /// Validate the next possible block boundary.
    fn start(&mut self, source: &str) -> Result<Option<usize>, E>;
    /// Validate the next admitted block token.
    fn tokenize(&mut self, source: &str) -> Result<Option<BlockToken>, E>;
}

/// Boundary validation around a caller-owned tokenizer.
struct Checked<'a>(&'a mut dyn BlockTokenizer);
impl Extension<InvalidBlockToken> for Checked<'_> {
    fn start(&mut self, source: &str) -> Result<Option<usize>, InvalidBlockToken> {
        let hint = self.0.start(source);
        if hint.is_some_and(|offset| offset > source.len() || !source.is_char_boundary(offset)) {
            return Err(InvalidBlockToken);
        }
        Ok(hint)
    }
    fn tokenize(&mut self, source: &str) -> Result<Option<BlockToken>, InvalidBlockToken> {
        let token = self.0.tokenize(source);
        if token
            .as_ref()
            .is_some_and(|token| token.raw.is_empty() || !source.starts_with(&token.raw))
        {
            return Err(InvalidBlockToken);
        }
        Ok(token)
    }
}

/// Parse caller-normalized owned source with terminal grammar.
///
/// Quote and list nesting costs heap frames, never the caller's stack, so no
/// container depth limit applies.
#[must_use]
pub fn parse(source: String) -> Document {
    parse_with_options(source, MarkedOptions::Terminal)
}

/// Parse owned source using the selected consumer's grammar.
///
/// Quote and list nesting costs heap frames, never the caller's stack, so no
/// container depth limit applies.
#[must_use]
pub fn parse_with_options(source: String, options: MarkedOptions) -> Document {
    match parse_inner::<Infallible>(source, options, None) {
        Ok(document) => document,
        Err(never) => match never {},
    }
}

/// Parse through one optional caller-owned block extension before built-in grammar.
///
/// Quote and list nesting costs heap frames, never the caller's stack, so no
/// container depth limit applies.
///
/// # Errors
/// Returns an error if a token is not a nonempty input prefix, or a start hint
/// lies outside the supplied source or splits a UTF-8 character.
pub fn parse_with_tokenizer(
    source: String,
    options: MarkedOptions,
    tokenizer: Option<&mut dyn BlockTokenizer>,
) -> Result<Document, InvalidBlockToken> {
    match tokenizer {
        Some(tokenizer) => parse_inner(source, options, Some(&mut Checked(tokenizer))),
        None => parse_inner(source, options, None),
    }
}

/// Share one parse pipeline with infallible absence and validated extension errors.
fn parse_inner<E>(
    source: String,
    options: MarkedOptions,
    tokenizer: Option<&mut dyn Extension<E>>,
) -> Result<Document, E> {
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.documents += 1);
    Lexer::parse(source, options, tokenizer)
}
