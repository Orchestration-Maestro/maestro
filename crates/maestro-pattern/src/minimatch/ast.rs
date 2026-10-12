//! Scalar segment parsing and matching.

use std::collections::BTreeSet;

mod matching;
mod parse;

use super::brace_expressions::{Class, ParsedClass, parse_class, parse_ignore_class};

/// One basic segment token.
#[derive(Debug)]
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
    /// A library-owned case predicate consuming one scalar.
    Folded(regress::Regex),
    /// An ordinary case-folded literal retaining scalar identity.
    FoldedLiteral(char),
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
        tokens.dedup_by(|a, b| matches!(a, Token::Star) && matches!(b, Token::Star));
        let literal: Option<String> = tokens
            .iter()
            .map(|t| match t {
                Token::Literal(c) => Some(*c),
                Token::Star
                | Token::Question
                | Token::Class(_)
                | Token::Never
                | Token::Folded(_)
                | Token::FoldedLiteral(_)
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

    /// Compiles case predicates once without changing authored syntax.
    pub(super) fn fold_case(&mut self) {
        if let Self::Literal(text) = self
            && text.to_uppercase() != text.to_lowercase()
        {
            *self = Self::Pattern(text.chars().map(Token::Literal).collect());
        }
        let unicode = match self {
            Self::Pattern(tokens) => tokens.iter().any(Token::unicode),
            Self::Groups(groups) => groups.sequences.iter().flatten().any(Token::unicode),
            Self::Literal(_) | Self::Ignore(_) => false,
        };
        match self {
            Self::Pattern(tokens) => tokens.iter_mut().for_each(|t| t.fold(unicode)),
            Self::Groups(groups) => groups
                .sequences
                .iter_mut()
                .flatten()
                .for_each(|t| t.fold(unicode)),
            Self::Literal(_) | Self::Ignore(_) => {}
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
                || (text.starts_with('.') && !matches!(tokens.first(), Some(Token::Literal('.'))))))
    {
        return false;
    }
    if matches!(tokens.first(), Some(Token::Star))
        && let Some(suffix) = tokens[1..]
            .iter()
            .map(|token| match token {
                Token::Literal(c) => Some(*c),
                Token::Star
                | Token::Question
                | Token::Class(_)
                | Token::Never
                | Token::Folded(_)
                | Token::FoldedLiteral(_)
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
        Some(Token::FoldedLiteral(c)) if *c == super::canonical(scalar) => Some(position + 1),
        Some(Token::Folded(predicate)) => {
            let mut bytes = [0; 4];
            predicate
                .find(scalar.encode_utf8(&mut bytes))
                .map(|_| position + 1)
        }
        _ => None,
    }
}

/// Advances zero-length transitions after adjacent stars are collapsed.
fn close_stars(tokens: &[Token], reachable: &mut BTreeSet<usize>) {
    let advances: Vec<_> = reachable
        .iter()
        .copied()
        .filter(|i| matches!(tokens.get(*i), Some(Token::Star)))
        .map(|i| i + 1)
        .collect();
    reachable.extend(advances);
}

/// Case-predicate preparation shared by plain and grouped syntax.
impl Token {
    /// Whether a named class enables Unicode case equivalence.
    fn unicode(&self) -> bool {
        matches!(self, Self::Class(class) if class.unicode())
    }
    /// Replaces a literal or class by a library-owned scalar predicate.
    fn fold(&mut self, unicode: bool) {
        if !unicode {
            match self {
                Self::Literal('.') => return,
                Self::Literal(c) => *self = Self::FoldedLiteral(super::canonical(*c)),
                Self::Class(class) => class.fold_ordinary_case(),
                _ => {}
            }
            return;
        }
        let expression = match self {
            Self::Literal('.') => return,
            Self::Literal(c) => format!("^{}$", scalar_expression(*c)),
            Self::Class(class) => class.expression(),
            _ => return,
        };
        if let Ok(predicate) =
            regress::Regex::with_flags(&expression, if unicode { "iu" } else { "i" })
        {
            *self = Self::Folded(predicate);
        }
    }
}
/// Quotes a scalar as a literal in a regular-expression atom or member.
pub(super) fn scalar_expression(c: char) -> String {
    if c.is_ascii() {
        format!("\\x{:02x}", u32::from(c))
    } else {
        c.to_string()
    }
}
