//! Document settlement and direct construction of owned metadata nodes.

use super::{FrontmatterError, FrontmatterValue, alias_count::Anchors, lexer::Cursor, values};

/// Lexical admission rules at a node boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    /// Document root, whose children may start in the first column.
    Root,
    /// Block collection or scalar.
    Block,
    /// Inline block value, excluding compact nested maps.
    Inline,
    /// Block mapping key, ending at a separated colon.
    Key,
    /// Flow collection member.
    Flow,
}

impl Mode {
    /// Whether block collections may start at this node.
    fn is_block(self) -> bool {
        matches!(self, Self::Root | Self::Block)
    }
}

/// One document's cursor and completed anchors.
pub(super) struct Parser<'a> {
    /// Monotone source traversal.
    pub cursor: Cursor<'a>,
    /// Only completed values may be referenced.
    anchors: Anchors,
}

/// Parse a single extracted document with no retained source borrows.
pub(super) fn parse(source: &str) -> Result<FrontmatterValue, FrontmatterError> {
    let mut parser = Parser {
        cursor: Cursor { source, offset: 0 },
        anchors: Anchors::default(),
    };
    parser.cursor.trivia();
    let value = if parser.cursor.peek().is_none() || parser.cursor.document_end() {
        FrontmatterValue::Null
    } else {
        parser.node(0, Mode::Root)?
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
        if mode.is_block() && self.cursor.tab_indented() {
            return Err(self.cursor.error("Tabs cannot indent block nodes"));
        }
        if mode.is_block() && !self.cursor.indicator('-') && self.cursor.mapping_start() {
            return self.block_map(self.cursor.column());
        }
        let start = self.anchors.mark();
        let props = self.properties()?;
        let value = self.content(indent, mode, props.tag.as_ref())?;
        if let Some(anchor) = props.anchor {
            self.anchors.publish(anchor, value.clone(), start);
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
            Some('-') if mode.is_block() && self.cursor.indicator('-') => {
                self.block_seq(self.cursor.column())?
            }
            Some('|' | '>') if mode != Mode::Flow && mode != Mode::Key => {
                let text = self.block_scalar(indent)?;
                self.anchors.leaf();
                return Ok(values::scalar(
                    text,
                    super::resolve_flow_scalar::Style::Quoted,
                    tag,
                ));
            }
            Some('\n' | '#') => self.block_value(indent, mode)?,
            None => self.null(),
            _ => {
                let (text, style) = self.flow_scalar(indent, mode)?;
                self.anchors.leaf();
                return Ok(values::scalar(text, style, tag));
            }
        };
        Ok(value)
    }

    /// Resolve the latest preceding completed binding.
    fn alias(&mut self) -> Result<FrontmatterValue, FrontmatterError> {
        self.cursor.next();
        let name = self.property_name();
        self.anchors.resolve(name)
    }

    /// An absent node, counted as a scalar for alias weighing.
    fn null(&mut self) -> FrontmatterValue {
        self.anchors.leaf();
        FrontmatterValue::Null
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
                return Ok(self.null());
            }
            let column = self.cursor.column();
            if column < indent
                || column == indent
                    && mode != Mode::Root
                    && (mode == Mode::Block || !self.cursor.indicator('-'))
            {
                return Ok(self.null());
            }
            self.node(indent, Mode::Block)
        } else if self.cursor.peek().is_none() {
            Ok(self.null())
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
