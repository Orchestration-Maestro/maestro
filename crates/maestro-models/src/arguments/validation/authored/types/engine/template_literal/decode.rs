//! Template pattern decoding for declared text nodes.
//!
//! A pattern that names a finite set of literal texts converts through those literals; any
//! other pattern, including one this decoder cannot read, stands for text itself.

use std::{borrow::Cow, rc::Rc};

use serde_json::Value;

use crate::arguments::validation::authored::schema_view::Node;

/// Decode a pattern into the node its alternatives stand for, when that set is finite.
pub(crate) fn decode<'a>(pattern: &str) -> Option<Rc<Node<'a>>> {
    let mut literals = variants(pattern)?
        .into_iter()
        .map(|literal| Rc::new(Node::Constant(Cow::Owned(Value::String(literal)))));
    let first = literals.next()?;
    let rest: Vec<Rc<Node<'a>>> = literals.collect();
    if rest.is_empty() {
        return Some(first);
    }
    let mut alternatives = vec![first];
    alternatives.extend(rest);
    Some(Rc::new(Node::Union(alternatives, None)))
}

/// Expand a pattern into the literal texts it accepts, when that set is finite.
fn variants(pattern: &str) -> Option<Vec<String>> {
    let body = pattern.strip_prefix('^')?.strip_suffix('$')?;
    if body.is_empty() {
        return None;
    }
    let mut reader = Reader {
        text: body.as_bytes(),
        index: 0,
    };
    let variants = reader.alternatives()?;
    (reader.index == body.len()).then_some(variants)
}

/// One reading position over a pattern body.
struct Reader<'a> {
    /// Pattern bytes.
    text: &'a [u8],
    /// Bytes consumed.
    index: usize,
}

impl Reader<'_> {
    /// Read alternatives separated by a vertical bar.
    fn alternatives(&mut self) -> Option<Vec<String>> {
        let mut variants = self.sequence()?;
        while self.peek() == Some(b'|') {
            self.index += 1;
            variants.extend(self.sequence()?);
        }
        Some(variants)
    }

    /// Read a sequence of pieces and their repetitions.
    fn sequence(&mut self) -> Option<Vec<String>> {
        let mut variants = vec![String::new()];
        while let Some(byte) = self.peek() {
            if byte == b'|' || byte == b')' {
                break;
            }
            let piece = self.piece()?;
            variants = join(&variants, &piece);
        }
        Some(variants)
    }

    /// Read one piece and its quantifier.
    fn piece(&mut self) -> Option<Vec<String>> {
        let atom = self.atom()?;
        match self.peek() {
            Some(b'?') => {
                self.index += 1;
                let mut variants = vec![String::new()];
                variants.extend(atom);
                Some(variants)
            }
            Some(b'{') => self.repetition(&atom),
            Some(b'*' | b'+') => None,
            _ => Some(atom),
        }
    }

    /// Read a bounded repetition of the supplied atom.
    fn repetition(&mut self, atom: &[String]) -> Option<Vec<String>> {
        let close = self.text.iter().skip(self.index).position(|b| *b == b'}')?;
        let body = std::str::from_utf8(&self.text[self.index + 1..self.index + close]).ok()?;
        self.index += close + 1;
        let (lowest, highest): (u32, Option<u32>) = match body.split_once(',') {
            Some((lowest, "")) => {
                let count: u32 = lowest.parse().ok()?;
                (count, Some(count))
            }
            Some((lowest, highest)) => (lowest.parse().ok()?, Some(highest.parse().ok()?)),
            None => {
                let count: u32 = body.parse().ok()?;
                (count, Some(count))
            }
        };
        let mut variants = Vec::new();
        let mut current = vec![String::new()];
        for count in 0..=highest? {
            if count >= lowest {
                variants.extend(current.iter().cloned());
            }
            current = join(&current, atom);
        }
        Some(variants)
    }

    /// Read one atom: a group, a class or a literal text.
    fn atom(&mut self) -> Option<Vec<String>> {
        match self.peek()? {
            b'(' => {
                self.index += 1;
                if self.text[self.index..].starts_with(b"?") {
                    self.index += 1;
                }
                let variants = self.alternatives()?;
                (self.peek() == Some(b')')).then_some(())?;
                self.index += 1;
                Some(variants)
            }
            b'[' => self.class(),
            b'\\' => self.escaped(),
            b'.' | b')' | b'|' => None,
            byte => {
                self.index += 1;
                Some(vec![character(byte).to_string()])
            }
        }
    }

    /// Read a character class that names exactly one literal text.
    fn class(&mut self) -> Option<Vec<String>> {
        let close = self.text.iter().skip(self.index).position(|b| *b == b']')?;
        let body = &self.text[self.index + 1..self.index + close];
        self.index += close + 1;
        match body {
            [single] | [b'\\', single] => Some(vec![character(*single).to_string()]),
            _ => None,
        }
    }

    /// Read an escaped literal character.
    fn escaped(&mut self) -> Option<Vec<String>> {
        let next = *self.text.get(self.index + 1)?;
        self.index += 2;
        match next {
            b'd' | b'w' | b's' | b'D' | b'W' | b'S' | b'b' | b'B' => None,
            b'n' => Some(vec!["\n".into()]),
            b't' => Some(vec!["\t".into()]),
            b'r' => Some(vec!["\r".into()]),
            byte if byte.is_ascii_alphanumeric() => None,
            byte => Some(vec![character(byte).to_string()]),
        }
    }

    /// Read the next unconsumed byte.
    fn peek(&self) -> Option<u8> {
        self.text.get(self.index).copied()
    }
}

/// Decode one pattern byte as the character it spells.
fn character(byte: u8) -> char {
    char::from(byte)
}

/// Combine two literal sets by concatenating every pair.
fn join(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .flat_map(|prefix| right.iter().map(move |suffix| format!("{prefix}{suffix}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::variants;

    /// Alternatives name every alternative, not their concatenation.
    #[test]
    fn alternatives_stay_separate() {
        assert_eq!(
            variants("^(null|true)$").unwrap_or_default(),
            ["null", "true"]
        );
    }

    /// A finite pattern names every text it accepts, however many there are.
    #[test]
    fn finite_patterns_expand_without_a_bound() {
        let literals = variants("^(a|b){11}$").unwrap_or_default();
        assert_eq!(literals.len(), 2_048);
        assert!(literals.iter().all(|literal| literal.len() == 11));
    }

    /// Unbounded repetition names no finite set of texts.
    #[test]
    fn unbounded_repetition_stays_unbounded() {
        for pattern in ["^(a|b)+$", "^(a|b)*$"] {
            assert!(variants(pattern).is_none(), "{pattern} is unbounded");
        }
    }
}
