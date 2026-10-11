//! Scalar segment parsing and matching.

use std::collections::BTreeSet;

mod matching;
mod parse;

use super::brace_expressions::{Class, ParsedClass, parse_class, parse_ignore_class};

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
    /// A group in the flat syntax arena.
    Group(usize),
}

/// A parsed path component.
pub(super) enum Ast {
    /// Decoded text with no wildcard syntax.
    Literal(String),
    /// Tokens including at least one nonliteral token.
    Pattern(Vec<Token>),
    /// Ignore tokens permit dotted components.
    Ignore(Vec<Token>),
    /// Nested syntax held without recursive ownership.
    Groups(parse::Groups),
}

impl Ast {
    /// Parses one component without interpreting separators.
    pub(super) fn parse(pattern: &str) -> Self {
        if let Some(groups) = parse::Groups::parse(pattern) {
            return Self::Groups(groups);
        }
        Self::basic(pattern)
    }

    /// Parses ignore tokens without extended groups or public dot policy.
    pub(super) fn parse_ignore(pattern: &str) -> Self {
        Self::basic_mode(pattern, true)
    }

    /// Parses public scalar tokens without recognizing groups.
    fn basic(pattern: &str) -> Self {
        Self::basic_mode(pattern, false)
    }

    /// Parses scalar tokens in the selected dialect.
    fn basic_mode(pattern: &str, ignore: bool) -> Self {
        let mut tokens = Vec::new();
        let mut tail = pattern;
        while let Some(c) = tail.chars().next() {
            if let Some((token, consumed)) = class_token(tail, ignore) {
                tokens.push(token);
                tail = &tail[consumed..];
                continue;
            }
            tail = &tail[c.len_utf8()..];
            let mut chars = tail.chars();
            tokens.push(match c {
                '\\' => match chars.next() {
                    Some(c) => Token::Literal(if ignore { super::canonical(c) } else { c }),
                    None if ignore => Token::Never,
                    None => Token::Literal('\\'),
                },
                '*' => Token::Star,
                '?' => Token::Question,
                _ => Token::Literal(if ignore { super::canonical(c) } else { c }),
            });
            tail = chars.as_str();
        }
        tokens.dedup_by(|a, b| *a == Token::Star && *b == Token::Star);
        let literal: Option<String> = tokens
            .iter()
            .map(|t| match t {
                Token::Literal(c) => Some(*c),
                Token::Star
                | Token::Question
                | Token::Class(_)
                | Token::Never
                | Token::Group(_) => None,
            })
            .collect();
        literal.map_or_else(
            || {
                if ignore {
                    Self::Ignore(tokens)
                } else {
                    Self::Pattern(tokens)
                }
            },
            Self::Literal,
        )
    }

    /// Borrows decoded literal text for native root identity.
    pub(super) fn literal(&self) -> Option<&str> {
        match self {
            Self::Literal(text) => Some(text),
            Self::Pattern(_) | Self::Ignore(_) | Self::Groups(_) => None,
        }
    }

    /// Tests one nonseparator component.
    pub(super) fn matches(&self, text: &str) -> bool {
        match self {
            Self::Literal(literal) => literal == text,
            Self::Pattern(tokens) => match_pattern(tokens, text, false),
            Self::Ignore(tokens) => match_pattern(tokens, text, true),
            Self::Groups(groups) => matching::matches(groups, text),
        }
    }
}

/// Decodes a class token, including named range endpoint rejection.
/// Unclosed openers fall back only in the public dialect; ignore openers cannot match.
fn class_token(pattern: &str, ignore: bool) -> Option<(Token, usize)> {
    if !pattern.starts_with('[') {
        return None;
    }
    let (parsed, consumed) = if ignore {
        parse_ignore_class(pattern)
    } else {
        parse_class(pattern)
    };
    let token = match parsed {
        ParsedClass::Literal(c) => Token::Literal(if ignore { super::canonical(c) } else { c }),
        ParsedClass::Class(class) => Token::Class(class),
        ParsedClass::Never => Token::Never,
        ParsedClass::Unclosed if ignore => return Some((Token::Never, pattern.len())),
        ParsedClass::Unclosed => return None,
    };
    Some((token, consumed))
}

/// Consumes scalar text through active token positions.
fn match_pattern(tokens: &[Token], text: &str, dot: bool) -> bool {
    if text.is_empty()
        || (!dot
            && (text == "."
                || text == ".."
                || (text.starts_with('.') && tokens.first() != Some(&Token::Literal('.')))))
    {
        return false;
    }
    if tokens.first() == Some(&Token::Star)
        && let Some(suffix) = tokens[1..]
            .iter()
            .map(|token| match token {
                Token::Literal(c) => Some(*c),
                Token::Star
                | Token::Question
                | Token::Class(_)
                | Token::Never
                | Token::Group(_) => None,
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
