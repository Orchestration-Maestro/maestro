//! Package metadata and number-format selection.
use super::super::docx_preview::{OpenXmlPackage, XmlParser};
use super::{Ssf, WorkbookError};
/// Package access retaining the supplied workbook name at failure boundaries.
pub(super) struct Package<'a> {
    /// Shared indexed archive owner.
    pub archive: OpenXmlPackage,
    /// Supplied workbook spelling.
    pub name: &'a str,
}
impl Package<'_> {
    /// Read a required part with workbook and part context.
    pub fn text(&self, part: &str) -> Result<String, WorkbookError> {
        self.archive
            .text(part)
            .map_err(|e| WorkbookError::Archive(format!("{}: {part}: {e}", self.name)))?
            .ok_or_else(|| WorkbookError::MissingPart(format!("{}: {part}", self.name)))
    }
    /// Parse through the existing XML owner after releasing archive access.
    pub fn xml<'a>(&self, text: &'a str, part: &str) -> Result<XmlParser<'a>, WorkbookError> {
        self.archive
            .parse(text, part)
            .map_err(|e| WorkbookError::Xml(format!("{}: {part}: {e}", self.name)))
    }
    /// Load explicit formats and remap high IDs to the same available slot.
    pub fn styles(&self, part: Option<&str>, ssf: &Ssf) -> Result<Vec<u32>, WorkbookError> {
        let Some(part) = part else {
            return Ok(Vec::new());
        };
        let text = self.text(part)?;
        let xml = self.xml(&text, part)?;
        let mut table = ssf
            .table
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut formats = table.clone();
        for node in xml
            .element(xml.root(), "numFmts")
            .into_iter()
            .flat_map(|parent| xml.elements(parent, Some("numFmt")))
        {
            let id = xml
                .attr(node, "numFmtId")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let code = xml.attr(node, "formatCode").unwrap_or("");
            formats.insert(id, code.to_owned());
            if id == 0 {
                continue;
            }
            let slot = if id > 392 {
                (61..=392)
                    .rev()
                    .find(|id| !formats.contains_key(id))
                    .unwrap_or(60)
            } else {
                id
            };
            formats.insert(slot, code.to_owned());
            table.insert(slot, if code == "d.m" { "d\\.m" } else { code }.to_owned());
        }
        Ok(xml
            .element(xml.root(), "cellXfs")
            .into_iter()
            .flat_map(|p| xml.elements(p, Some("xf")))
            .map(|node| {
                let id = xml
                    .attr(node, "numFmtId")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                if id > 392 {
                    (61..=392)
                        .rev()
                        .find(|slot| formats.get(slot) == formats.get(&id))
                        .unwrap_or(id)
                } else {
                    id
                }
            })
            .collect())
    }
}
