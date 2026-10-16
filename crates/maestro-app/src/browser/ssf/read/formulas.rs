//! Sheet-local formula metadata from native expansion and borrowed XML.
use super::super::{CellAddress, Range, WorkbookError, decode_range};
use crate::browser::docx_preview::{XmlElement, XmlParser};
use calamine::Xlsx;
use std::{collections::BTreeMap, io::Cursor};

/// Collect owned sparse formulas without changing the native value stream.
pub(super) fn collect(
    native: &mut Xlsx<Cursor<&[u8]>>,
    name: &str,
) -> Result<BTreeMap<(u32, u32), String>, WorkbookError> {
    let mut reader = native
        .worksheet_cells_reader(name)
        .map_err(|e| WorkbookError::Reader(format!("{name}: {e}")))?;
    let mut result = BTreeMap::new();
    while let Some(cell) = reader
        .next_formula()
        .map_err(|e| WorkbookError::Reader(format!("{name}: {e}")))?
    {
        if !cell.get_value().is_empty() {
            result.insert(cell.get_position(), cell.get_value().replace("_xlfn.", ""));
        }
    }
    Ok(result)
}

/// Retain encountered array regions and select the last covering range.
pub(super) fn array_range(
    xml: &XmlParser<'_>,
    node: XmlElement<'_, '_>,
    position: CellAddress,
    arrays: &mut Vec<(Range, String)>,
) -> Result<Option<String>, WorkbookError> {
    let own = xml
        .element(node, "f")
        .filter(|f| xml.attr(*f, "t") == Some("array") && !f.text_content().is_empty())
        .and_then(|f| xml.attr(f, "ref"));
    if let Some(range) = own.filter(|r| r.contains(':')) {
        arrays.push((decode_range(range)?, range.to_owned()));
    }
    Ok(arrays
        .iter()
        .rev()
        .find(|(range, _)| {
            (range.s.r..=range.e.r).contains(&position.r)
                && (range.s.c..=range.e.c).contains(&position.c)
        })
        .map(|(_, text)| text.clone())
        .or_else(|| own.map(str::to_owned)))
}
