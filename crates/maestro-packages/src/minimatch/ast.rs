//! Scalar segment parsing and matching.

use std::collections::BTreeSet;

use super::brace_expressions::{Class, ParsedClass, parse_class};

/// One basic segment token.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Token {
    /// An authored literal scalar.
    Literal(char),
    /// Zero or more scalars.
    Star,
    /// Exactly one Unicode scalar.
    Question,
    /// One scalar in a prepared member union.
    Class(Class),
    /// An impossible component selection.
    Never,
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
        let mut tail = pattern;
        while let Some(c) = tail.chars().next() {
            if let Some((token, consumed)) = class_token(tail) {
                tokens.push(token);
                tail = &tail[consumed..];
                continue;
            }
            tail = &tail[c.len_utf8()..];
            let mut chars = tail.chars();
            tokens.push(match c {
                '\\' => Token::Literal(chars.next().unwrap_or('\\')),
                '*' => Token::Star,
                '?' => Token::Question,
                _ => Token::Literal(c),
            });
            tail = chars.as_str();
        }
        tokens.dedup_by(|a, b| *a == Token::Star && *b == Token::Star);
        let literal: Option<String> = tokens
            .iter()
            .map(|t| match t {
                Token::Literal(c) => Some(*c),
                Token::Star | Token::Question | Token::Class(_) | Token::Never => None,
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

/// Decodes a closed class token without consuming an unclosed opener.
fn class_token(pattern: &str) -> Option<(Token, usize)> {
    if !pattern.starts_with('[') {
        return None;
    }
    let (parsed, consumed) = parse_class(pattern);
    let token = match parsed {
        ParsedClass::Literal(c) => Token::Literal(c),
        ParsedClass::Class(class) => Token::Class(class),
        ParsedClass::Never => Token::Never,
        ParsedClass::Unclosed => return None,
    };
    Some((token, consumed))
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
    if tokens.first() == Some(&Token::Star)
        && let Some(suffix) = tokens[1..]
            .iter()
            .map(|token| match token {
                Token::Literal(c) => Some(*c),
                Token::Star | Token::Question | Token::Class(_) | Token::Never => None,
            })
            .collect::<Option<String>>()
    {
        return text.ends_with(&suffix);
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
        Some(Token::Class(class)) if class.matches(scalar) => Some(position + 1),
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
