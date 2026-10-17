//! Block mappings, explicit keys and ordered typed-key construction.

use super::{
    FrontmatterError, FrontmatterValue,
    parser::{Mode, Parser},
    values::Mapping,
};

impl Parser<'_> {
    /// Parse mapping entries at one indentation level.
    pub fn block_map(&mut self, indent: usize) -> Result<FrontmatterValue, FrontmatterError> {
        let mut mapping = Mapping::default();
        loop {
            let explicit = self.cursor.indicator('?');
            let key = self.block_key(indent)?;
            self.cursor.spaces();
            let value = if self.cursor.eat(':') {
                self.entry_value(indent, explicit)?
            } else if explicit {
                FrontmatterValue::Null
            } else {
                return Err(self.cursor.error("Expected ':' after mapping key"));
            };
            mapping.insert(key, value)?;
            self.finish_entry()?;
            if self.cursor.peek().is_none()
                || self.cursor.document_end()
                || self.cursor.column() < indent
            {
                break;
            }
            if self.cursor.column() != indent || !self.cursor.mapping_start() {
                break;
            }
        }
        Ok(mapping.finish())
    }

    /// Parse the value after a mapping colon, rejecting an inline sequence.
    fn entry_value(
        &mut self,
        indent: usize,
        explicit: bool,
    ) -> Result<FrontmatterValue, FrontmatterError> {
        self.cursor.spaces();
        if !explicit && self.cursor.indicator('-') {
            return Err(self
                .cursor
                .error("Block sequence indicator on the same line as a mapping key"));
        }
        self.block_value(indent, Mode::Inline)
    }

    /// Read an implicit single-line key or an explicit possibly multiline key.
    fn block_key(&mut self, indent: usize) -> Result<FrontmatterValue, FrontmatterError> {
        let explicit = self.cursor.indicator('?');
        if explicit {
            self.cursor.next();
            self.cursor.spaces();
        }
        let start = self.cursor.offset;
        let key = if self.cursor.indicator(':')
            || matches!(self.cursor.peek(), None | Some('\n' | '#'))
        {
            FrontmatterValue::Null
        } else {
            self.node(indent, Mode::Key)?
        };
        let source = &self.cursor.source[start..self.cursor.offset];
        if !explicit
            && (source.contains('\n')
                || source.trim_end_matches([' ', '\t']).chars().count() > 1024)
        {
            return Err(self.cursor.error("Invalid implicit mapping key"));
        }
        if explicit {
            self.cursor.spaces();
            self.cursor.comment();
            if self.cursor.eat('\n') {
                self.cursor.trivia();
            }
        }
        Ok(key)
    }
}
