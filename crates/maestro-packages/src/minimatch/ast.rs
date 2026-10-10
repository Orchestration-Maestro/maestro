//! Scalar segment parsing and matching.

use std::collections::BTreeSet;

/// One basic segment token.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Token {
    /// An authored literal scalar.
    Literal(char),
    /// Zero or more scalars.
    Star,
    /// Exactly one Unicode scalar.
    Question,
}

/// A parsed path component.
pub(super) enum Ast {
    /// Decoded text with no wildcard syntax.
    Literal(String),
    /// Tokens including at least one wildcard.
    Pattern(Vec<Token>),
}

impl Ast {
    /// Parses one component without interpreting separators.
    pub(super) fn parse(pattern: &str) -> Self {
        let mut tokens = Vec::new();
        let mut chars = pattern.chars();
        while let Some(c) = chars.next() {
            tokens.push(match c {
                '\\' => Token::Literal(chars.next().unwrap_or('\\')),
                '*' => Token::Star,
                '?' => Token::Question,
                _ => Token::Literal(c),
            });
        }
        tokens.dedup_by(|a, b| *a == Token::Star && *b == Token::Star);
        let literal: Option<String> = tokens
            .iter()
            .map(|t| match t {
                Token::Literal(c) => Some(*c),
                Token::Star | Token::Question => None,
            })
            .collect();
        literal.map_or(Self::Pattern(tokens), Self::Literal)
    }

    /// Borrows decoded literal text for native root identity.
    pub(super) fn literal(&self) -> Option<&str> {
        match self {
            Self::Literal(text) => Some(text),
            Self::Pattern(_) => None,
        }
    }

    /// Tests one nonseparator component.
    pub(super) fn matches(&self, text: &str) -> bool {
        match self {
            Self::Literal(literal) => literal == text,
            Self::Pattern(tokens) => match_pattern(tokens, text),
        }
    }
}

/// Consumes scalar text through active token positions.
fn match_pattern(tokens: &[Token], text: &str) -> bool {
    if text.is_empty()
        || text == "."
        || text == ".."
        || (text.starts_with('.') && tokens.first() != Some(&Token::Literal('.')))
    {
        return false;
    }
    let mut reachable = BTreeSet::from([0]);
    close_stars(tokens, &mut reachable);
    for c in text.chars() {
        let mut next = BTreeSet::new();
        for &i in &reachable {
            next.extend(advance(tokens, i, c));
        }
        close_stars(tokens, &mut next);
        reachable = next;
    }
    reachable.contains(&tokens.len())
}

/// Consumes a scalar at an active token position.
fn advance(tokens: &[Token], position: usize, scalar: char) -> Option<usize> {
    match tokens.get(position) {
        Some(Token::Star) => Some(position),
        Some(Token::Question) => Some(position + 1),
        Some(Token::Literal(c)) if *c == scalar => Some(position + 1),
        _ => None,
    }
}

/// Advances zero-length transitions after adjacent stars are collapsed.
fn close_stars(tokens: &[Token], reachable: &mut BTreeSet<usize>) {
    let advances: Vec<_> = reachable
        .iter()
        .copied()
        .filter(|i| tokens.get(*i) == Some(&Token::Star))
        .map(|i| i + 1)
        .collect();
    reachable.extend(advances);
}
