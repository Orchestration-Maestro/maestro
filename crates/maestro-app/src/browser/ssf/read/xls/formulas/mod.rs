//! Owned binary expressions and retained template identity.
mod cetab;
mod ftab;
mod functions;
mod parse_rgce;
mod stringify_formula;
mod supbooks;
mod types;
use super::super::blob::Result;
use super::{
    records::{CellHeader, Payload},
    strings::Encoding,
};
use crate::browser::ssf::CellAddress;
use crate::browser::ssf::{CellObject, Range, encode_range};
use std::collections::HashMap;
pub(super) use supbooks::SupBooks;
pub(super) use types::Ptg;
/// Read one counted expression with its trailing operand data.
pub(super) fn parse(payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<Vec<Ptg>> {
    parse_rgce::parse(payload, encoding)
}
/// Render one owned expression at its consuming position.
pub(super) fn stringify(
    tokens: &[Ptg],
    position: CellAddress,
    books: &SupBooks,
    encoding: Encoding,
) -> Result<String> {
    stringify_formula::stringify(tokens, position, books, encoding)
}

/// Most recent formula identity, independent of cache materialization.
pub(super) struct FormulaCell {
    /// Original cell position and format.
    pub(super) header: CellHeader,
    /// Parsed expression.
    tokens: Vec<Ptg>,
    /// Whether the following string record is still required.
    pub(super) pending: bool,
}
/// Per-sheet retained formula ownership.
#[derive(Default)]
pub(super) struct FormulaState {
    /// Latest formula record, including string-cached formulas.
    pub(super) last: Option<FormulaCell>,
    /// Templates selected by their source anchor address.
    anchors: HashMap<(u32, u32), FormulaTemplate>,
    /// Ordered array owner, scanned only for materialized expressions.
    arrays: Vec<(Range, Vec<Ptg>)>,
}
impl FormulaState {
    /// Install one formula's identity before any later template or cache record.
    pub(super) fn formula(&mut self, header: CellHeader, tokens: Vec<Ptg>, pending: bool) {
        self.last = Some(FormulaCell {
            header,
            tokens,
            pending,
        });
    }
    /// Register a shared expression at the actual last formula producer.
    pub(super) fn shared(
        &mut self,
        payload: &mut Payload<'_, '_>,
        encoding: Encoding,
    ) -> Result<()> {
        payload.skip(8)?;
        let tokens = parse(payload, encoding)?;
        if let Some(last) = &self.last {
            self.anchors.insert(
                (last.header.r, last.header.c),
                FormulaTemplate::Shared(tokens),
            );
        }
        Ok(())
    }
    /// Project the latest formula onto its already-materialized cached cell.
    pub(super) fn apply(
        &self,
        cell: &mut CellObject,
        books: &SupBooks,
        encoding: Encoding,
    ) -> Result<()> {
        let Some(last) = &self.last else {
            return Ok(());
        };
        let position = CellAddress {
            r: last.header.r,
            c: last.header.c,
        };
        let tokens = if let Some(Ptg::Exp(anchor)) = last.tokens.first() {
            #[cfg(test)]
            super::tests::ANCHOR_WORK.with(|work| work.set(work.get() + 1));
            match self.anchors.get(&(anchor.r, anchor.c)) {
                Some(FormulaTemplate::Shared(tokens)) => tokens,
                Some(FormulaTemplate::Array(index)) if !last.pending => {
                    return self.array_cell(cell, *index, books, encoding);
                }
                _ if last.pending => &last.tokens,
                _ => return Ok(()),
            }
        } else {
            &last.tokens
        };
        cell.f = Some(stringify(tokens, position, books, encoding)?);
        if cell.f.as_ref().is_some_and(|text| !text.is_empty())
            && let Some(index) = self.arrays.iter().position(|(range, _)| {
                #[cfg(test)]
                super::tests::ARRAY_WORK.with(|work| work.set(work.get() + 1));
                range.contains(position)
            })
        {
            return self.array_cell(cell, index, books, encoding);
        }
        Ok(())
    }
}

/// One retained anchor, with array bodies owned only by the ordered array vector.
enum FormulaTemplate {
    /// Owned shared expression.
    Shared(Vec<Ptg>),
    /// Index into the ordered array owner.
    Array(usize),
}
impl FormulaState {
    /// Register an array without requiring its cache to have been inserted.
    pub(super) fn array(
        &mut self,
        payload: &mut Payload<'_, '_>,
        encoding: Encoding,
    ) -> Result<()> {
        let range = range(payload)?;
        payload.skip(6)?;
        let tokens = parse(payload, encoding)?;
        self.anchors.insert(
            (range.s.r, range.s.c),
            FormulaTemplate::Array(self.arrays.len()),
        );
        self.arrays.push((range, tokens));
        Ok(())
    }
    /// Attach the selected range, suppressing expressions on followers.
    fn array_cell(
        &self,
        cell: &mut CellObject,
        index: usize,
        books: &SupBooks,
        encoding: Encoding,
    ) -> Result<()> {
        let Some(last) = &self.last else {
            return Ok(());
        };
        let position = CellAddress {
            r: last.header.r,
            c: last.header.c,
        };
        let (range, tokens) = &self.arrays[index];
        cell.array_formula = Some(encode_range(*range));
        cell.f = if position == range.s {
            Some(stringify(tokens, position, books, encoding)?)
        } else {
            None
        };
        Ok(())
    }
}
/// Decode the six-byte rectangle used by shared and array records.
fn range(payload: &mut Payload<'_, '_>) -> Result<Range> {
    let (first, last) = (u32::from(payload.u16()?), u32::from(payload.u16()?));
    let (column, end_column) = (u32::from(payload.u8()?), u32::from(payload.u8()?));
    Ok(Range {
        s: CellAddress {
            r: first,
            c: column,
        },
        e: CellAddress {
            r: last,
            c: end_column,
        },
    })
}
