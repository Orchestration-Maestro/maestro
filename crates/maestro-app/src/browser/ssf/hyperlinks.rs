//! Ordered modern hyperlink ranges and blank linked cell stubs.
use super::{
    CellAddress, CellObject, CellValue, ExcelDataType, Hyperlink, WorkSheet, WorkbookError,
    decode_range, encode_cell, styles::Package,
};
use crate::browser::docx_preview::{Relationship, XmlParser};
/// Assign authored relationship targets after scalar cells have been read.
pub(super) fn adopt(
    package: &Package<'_>,
    path: &str,
    xml: &XmlParser<'_>,
    sheet: &mut WorkSheet,
) -> Result<(), WorkbookError> {
    let Some(parent) = xml.element(xml.root(), "hyperlinks") else {
        return Ok(());
    };
    let relationships = package
        .archive
        .literal_relationships(path)
        .map_err(|e| WorkbookError::Xml(format!("{}: {path}: {e}", package.name)))?;
    for node in xml.elements(parent, Some("hyperlink")) {
        let Some(reference) = xml.attr(node, "ref") else {
            break;
        };
        let relationship = xml
            .attr(node, "id")
            .and_then(|id| Relationship::find_target(relationships.as_deref(), id));
        let location = xml
            .literal_attr(node, "location")
            .map_err(|e| WorkbookError::Xml(format!("{}: {path}: {e}", package.name)))?;
        let location = location.as_deref();
        let target = match relationship {
            Some(target) => format!(
                "{target}{}",
                location.map_or_else(String::new, |s| format!("#{s}"))
            ),
            None => format!("#{}", location.unwrap_or("undefined")),
        };
        let range = decode_range(reference)?;
        for r in range.s.r..=range.e.r {
            for c in range.s.c..=range.e.c {
                let cell = sheet
                    .cells
                    .entry(encode_cell(CellAddress { r, c }))
                    .or_insert_with(|| CellObject {
                        t: Some(ExcelDataType::Stub),
                        v: CellValue::Missing,
                        w: None,
                        h: None,
                        z: None,
                        l: None,
                        ..Default::default()
                    });
                cell.l = Some(Hyperlink {
                    target: target.clone(),
                });
            }
        }
    }
    Ok(())
}
