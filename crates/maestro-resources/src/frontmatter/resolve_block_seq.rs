//! Block sequences, including compact entries and indentless mapping values.

use super::{
    FrontmatterError, FrontmatterValue,
    parser::{Mode, Parser},
};

impl Parser<'_> {
    /// Parse items at a single dash indentation level.
    pub fn block_seq(&mut self, indent: usize) -> Result<FrontmatterValue, FrontmatterError> {
        let mut items = Vec::new();
        loop {
            self.cursor.next();
            items.push(self.block_value(indent, Mode::Block)?);
            self.finish_entry()?;
            if self.cursor.peek().is_none()
                || self.cursor.column() != indent
                || !self.cursor.indicator('-')
            {
                break;
            }
        }
        Ok(FrontmatterValue::Sequence(items))
    }
}
