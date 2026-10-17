//! Flow collections and sequence pairs without an intermediate event protocol.

use super::{
    FrontmatterError, FrontmatterValue,
    parser::{Mode, Parser},
    values::Mapping,
};

/// Delimiters and member construction for a flow collection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Collection {
    /// Square-bracketed sequence.
    Sequence,
    /// Brace-delimited mapping.
    Mapping,
}

impl Collection {
    /// Required closing delimiter.
    fn close(self) -> char {
        match self {
            Self::Sequence => ']',
            Self::Mapping => '}',
        }
    }
    /// Existing native error template for an unfinished collection.
    fn error(self) -> &'static str {
        match self {
            Self::Sequence => "while parsing a flow sequence, expected ',' or ']'",
            Self::Mapping => "while parsing a flow mapping, expected ',' or '}'",
        }
    }
}

impl Parser<'_> {
    /// Parse one delimited collection, including a trailing comma.
    pub fn flow_collection(&mut self) -> Result<FrontmatterValue, FrontmatterError> {
        let kind = if self.cursor.eat('[') {
            Collection::Sequence
        } else {
            self.cursor.next();
            Collection::Mapping
        };
        let mut mapping = Mapping::default();
        let mut items = Vec::new();
        self.cursor.trivia();
        while !self.cursor.eat(kind.close()) {
            if self.cursor.peek().is_none() {
                return Err(self.cursor.error(kind.error()));
            }
            let (key, value) = self.flow_pair(kind)?;
            if kind == Collection::Mapping {
                mapping.insert(key, value)?;
            } else {
                items.push(value);
            }
            self.cursor.trivia();
            if self.cursor.eat(kind.close()) {
                break;
            }
            if !self.cursor.eat(',') {
                return Err(self.cursor.error(kind.error()));
            }
            self.cursor.trivia();
        }
        Ok(if kind == Collection::Mapping {
            mapping.finish()
        } else {
            FrontmatterValue::Sequence(items)
        })
    }

    /// Read a key/value member or a bare sequence member.
    fn flow_pair(
        &mut self,
        kind: Collection,
    ) -> Result<(FrontmatterValue, FrontmatterValue), FrontmatterError> {
        if self.cursor.indicator('?') {
            self.cursor.next();
            self.cursor.trivia();
        }
        let key = if self.cursor.peek() == Some(':') {
            FrontmatterValue::Null
        } else {
            self.node(0, Mode::Flow)?
        };
        self.cursor.trivia();
        if self.cursor.eat(':') {
            self.cursor.trivia();
            let value = if matches!(self.cursor.peek(), Some(',' | ']' | '}') | None) {
                FrontmatterValue::Null
            } else {
                self.node(0, Mode::Flow)?
            };
            if kind == Collection::Mapping {
                return Ok((key, value));
            }
            let mut pair = Mapping::default();
            pair.insert(key, value)?;
            return Ok((FrontmatterValue::Null, pair.finish()));
        }
        if kind == Collection::Mapping {
            Ok((key, FrontmatterValue::Null))
        } else {
            Ok((FrontmatterValue::Null, key))
        }
    }
}
