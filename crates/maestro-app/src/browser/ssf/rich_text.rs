//! Modern shared and inline string markup at the XML boundary.
use super::{WorkbookError, styles::Package};
use crate::browser::docx_preview::integer_prefix;
use crate::browser::docx_preview::{XmlElement, XmlParser};
use maestro_text::markup::{EscapeMode, escape};
use num_traits::ToPrimitive;
use std::fmt::Write as _;

/// A shared string's plain text and preview markup.
pub(super) struct SharedString {
    /// Text stored as the cell value.
    pub(super) text: String,
    /// Generated preview markup.
    pub(super) html: String,
}

/// Resolve a shared-string index for both the scalar and preview projections.
pub(super) fn shared_string<'a>(raw: &str, shared: &'a [SharedString]) -> Option<&'a SharedString> {
    integer_prefix(raw, 10)
        .to_usize()
        .and_then(|index| shared.get(index))
}

/// Decode direct text, or direct rich runs, excluding phonetic runs.
pub(super) fn text(xml: &XmlParser<'_>, node: XmlElement<'_, '_>) -> String {
    let raw = match xml.element(node, "t") {
        Some(text) => text.text_content(),
        None => xml
            .elements(node, Some("r"))
            .filter_map(|run| xml.element(run, "t"))
            .map(|text| text.text_content())
            .collect(),
    };
    super::read::string_cell(&raw)
}
/// Load shared string markup once without reopening the workbook archive.
pub(super) fn shared(
    package: &Package<'_>,
    part: Option<&str>,
) -> Result<Vec<SharedString>, WorkbookError> {
    let Some(part) = part else {
        return Ok(Vec::new());
    };
    let source = package.text(part)?;
    let xml = package.xml(&source, part)?;
    Ok(xml
        .elements(xml.root(), Some("si"))
        .map(|node| SharedString {
            text: text(&xml, node),
            html: html(&xml, node),
        })
        .collect())
}
/// Project text once, keeping literal workbook content out of markup syntax.
pub(super) fn html(xml: &XmlParser<'_>, node: XmlElement<'_, '_>) -> String {
    if let Some(text) = xml.element(node, "t") {
        return escaped(&text.text_content());
    }
    let mut out = String::new();
    for run in xml.elements(node, Some("r")) {
        let text = xml
            .element(run, "t")
            .map_or_else(String::new, |t| escaped(&t.text_content()));
        if text.is_empty() {
            continue;
        }
        let Some(props) = xml.element(run, "rPr") else {
            out.push_str(&text);
            continue;
        };
        let mut style = String::new();
        if enabled(xml, props, "u") {
            style.push_str("text-decoration: underline;");
        }
        if let Some(size) = xml.element_attr(props, "sz", "val")
            && size.parse::<f64>().is_ok_and(|n| n.is_finite() && n > 0.)
        {
            let _ = write!(
                style,
                "font-size:{}pt;",
                escape(size, EscapeMode::XmlAttribute)
            );
        }
        let _ = write!(out, "<span style=\"{style}\">");
        let mut tags = Vec::new();
        for (property, tag) in [("b", "b"), ("i", "i"), ("strike", "s")] {
            if enabled(xml, props, property) {
                tags.push(tag);
            }
        }
        match xml.element_attr(props, "vertAlign", "val") {
            Some("superscript" | "super") => tags.push("sup"),
            Some("subscript") => tags.push("sub"),
            _ => {}
        }
        for tag in &tags {
            let _ = write!(out, "<{tag}>");
        }
        out.push_str(&text);
        for tag in &tags {
            let _ = write!(out, "</{tag}>");
        }
        out.push_str("</span>");
    }
    out
}
/// Honor explicit XML off-values without treating nonempty strings as true.
fn enabled(xml: &XmlParser<'_>, props: XmlElement<'_, '_>, name: &str) -> bool {
    xml.element(props, name)
        .is_some_and(|node| !matches!(xml.attr(node, "val"), Some("0" | "false" | "none")))
}
/// Decode authored string escapes once before escaping literal text.
fn escaped(text: &str) -> String {
    escape(
        &super::read::string_cell(text).replace("\r\n", "\n"),
        EscapeMode::SpreadsheetHtml,
    )
}
/// Adopt the default reader's generated HTML without inventing number formats.
pub(super) fn adopt(
    xml: &XmlParser<'_>,
    node: XmlElement<'_, '_>,
    cell: &mut super::CellObject,
    shared: &[SharedString],
) {
    cell.h = match xml.attr(node, "t") {
        Some("s") => xml
            .element(node, "v")
            .and_then(|n| shared_string(&n.text_content(), shared))
            .map(|string| string.html.clone()),
        Some("inlineStr") => xml.element(node, "is").map(|n| html(xml, n)),
        _ => {
            if let super::CellValue::Text(text) = &cell.v {
                Some(escape(text, EscapeMode::SpreadsheetHtml))
            } else {
                None
            }
        }
    };
}
