//! Document settlement and direct construction of owned metadata nodes.

use super::{FrontmatterError, FrontmatterValue, lexer::Cursor, values};
use std::collections::HashMap;

/// Lexical admission rules at a node boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    /// Block collection or scalar.
    Block,
    /// Inline block value, excluding compact nested maps.
    Inline,
    /// Block mapping key, ending at a separated colon.
    Key,
    /// Flow collection member.
    Flow,
}

/// One document's cursor and completed anchors.
pub(super) struct Parser<'a> {
    /// Monotone source traversal.
    pub cursor: Cursor<'a>,
    /// Only completed values may be referenced.
    anchors: HashMap<String, FrontmatterValue>,
}

/// Parse a single extracted document with no retained source borrows.
pub(super) fn parse(source: &str) -> Result<FrontmatterValue, FrontmatterError> {
    let mut parser = Parser {
        cursor: Cursor { source, offset: 0 },
        anchors: HashMap::new(),
    };
    parser.cursor.trivia();
    let value = if parser.cursor.peek().is_none() || parser.cursor.document_end() {
        FrontmatterValue::Null
    } else {
        parser.node(0, Mode::Block)?
    };
    parser.cursor.trivia();
    if parser.cursor.document_end() {
        parser.cursor.offset += 3;
        parser.cursor.trivia();
    }
    if parser.cursor.peek().is_some() {
        return Err(parser
            .cursor
            .error("Source contains multiple documents or unexpected content"));
    }
    Ok(if value == FrontmatterValue::Null {
        FrontmatterValue::Mapping(Vec::new())
    } else {
        value
    })
}

impl Parser<'_> {
    /// Parse a node and publish its anchor after successful completion.
    pub fn node(
        &mut self,
        indent: usize,
        mode: Mode,
    ) -> Result<FrontmatterValue, FrontmatterError> {
        if mode == Mode::Block
            && self.cursor.source[..self.cursor.offset]
                .rsplit('\n')
                .next()
                .is_some_and(|s| {
                    s.bytes()
                        .take_while(|b| matches!(b, b' ' | b'\t'))
                        .any(|b| b == b'\t')
                })
        {
            return Err(self.cursor.error("Tabs cannot indent block nodes"));
        }
        if mode == Mode::Block && !self.cursor.indicator('-') && self.cursor.mapping_start() {
            return self.block_map(self.cursor.column());
        }
        let props = self.properties()?;
        let value = self.content(indent, mode, props.tag.as_ref())?;
        if let Some(anchor) = props.anchor {
            self.anchors.insert(anchor, value.clone());
        }
        Ok(value)
    }

    /// Dispatch node grammar after its properties.
    fn content(
        &mut self,
        indent: usize,
        mode: Mode,
        tag: Option<&super::resolve_props::Tag>,
    ) -> Result<FrontmatterValue, FrontmatterError> {
        let value = match self.cursor.peek() {
            Some('*') => return self.alias(),
            Some('[' | '{') => self.flow_collection()?,
            Some('-') if mode == Mode::Block && self.cursor.indicator('-') => {
                self.block_seq(self.cursor.column())?
            }
            Some('|' | '>') if mode != Mode::Flow && mode != Mode::Key => {
                let text = self.block_scalar(indent)?;
                return Ok(values::scalar(
                    text,
                    super::resolve_flow_scalar::Style::Quoted,
                    tag,
                ));
            }
            Some('\n' | '#') => {
                self.cursor.trivia();
                if self.cursor.peek().is_none() || self.cursor.column() <= indent {
                    FrontmatterValue::Null
                } else {
                    self.node(self.cursor.column(), Mode::Block)?
                }
            }
            None => FrontmatterValue::Null,
            _ => {
                let (text, style) = self.flow_scalar(indent, mode)?;
                return Ok(values::scalar(text, style, tag));
            }
        };
        Ok(value)
    }

    /// Resolve the latest preceding completed binding.
    fn alias(&mut self) -> Result<FrontmatterValue, FrontmatterError> {
        self.cursor.next();
        let name = self.property_name();
        self.anchors
            .get(name)
            .cloned()
            .ok_or_else(|| FrontmatterError::new("Unresolved YAML alias"))
    }

    /// Parse an optional value following a block indicator.
    pub fn block_value(
        &mut self,
        indent: usize,
        mode: Mode,
    ) -> Result<FrontmatterValue, FrontmatterError> {
        self.cursor.spaces();
        self.cursor.comment();
        if self.cursor.peek() == Some('\n') {
            self.cursor.trivia();
            if self.cursor.peek().is_none() || self.cursor.document_end() {
                return Ok(FrontmatterValue::Null);
            }
            let column = self.cursor.column();
            if column < indent
                || column == indent && (mode == Mode::Block || !self.cursor.indicator('-'))
            {
                return Ok(FrontmatterValue::Null);
            }
            self.node(indent, Mode::Block)
        } else if self.cursor.peek().is_none() {
            Ok(FrontmatterValue::Null)
        } else {
            self.node(indent, mode)
        }
    }

    /// Settle a block entry without discarding unexpected inline content.
    pub fn finish_entry(&mut self) -> Result<(), FrontmatterError> {
        if self.cursor.at_indent() {
            return Ok(());
        }
        self.cursor.spaces();
        self.cursor.comment();
        if !matches!(self.cursor.peek(), None | Some('\n')) {
            return Err(self.cursor.error("Unexpected content after YAML value"));
        }
        self.cursor.trivia();
        Ok(())
    }
}
