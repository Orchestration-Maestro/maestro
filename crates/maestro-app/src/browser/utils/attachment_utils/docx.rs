//! Word package text extraction.
use super::append_document_text;
use crate::browser::docx_preview::{
    DocxError, DomType, OpenXmlElementData, OpenXmlElementTree, Options, WordDocument,
};
use std::borrow::Cow;

/// Extract the ordered body text inside the literal page wrapper.
pub(super) fn process_docx(bytes: &[u8], file_name: &str) -> Result<String, DocxError> {
    let body = WordDocument::load(bytes, Options::default())?.document_body()?;
    let mut output = String::new();
    let mut append = |text: &str| append_document_text(&mut output, text);
    append(&format!(
        "<docx filename=\"{file_name}\">\n<page number=\"1\">\n"
    ))?;
    append(
        &body
            .as_ref()
            .map_or_else(String::new, |tree| joined_text(tree, &tree.roots, "\n")),
    )?;
    append("\n</page>\n</docx>")?;
    Ok(output)
}

/// Trimmed text of one element.
fn extract_text(tree: &OpenXmlElementTree, index: usize) -> Cow<'_, str> {
    match raw_text(tree, index) {
        Cow::Borrowed(text) => Cow::Borrowed(text.trim_matches(maestro_text::is_whitespace_scalar)),
        Cow::Owned(mut text) => {
            let whitespace = maestro_text::is_whitespace_scalar;
            let end = text.trim_end_matches(whitespace).len();
            text.truncate(end);
            let start = text.len() - text.trim_start_matches(whitespace).len();
            text.drain(..start);
            Cow::Owned(text)
        }
    }
}

/// Untrimmed text of one element under its own child rule.
///
/// Text contributes itself and a table its formatted rows. Inline containers
/// keep their children's text without a separator, embedded containers join it
/// with one space, and every other element, drawings and drawn text boxes
/// included, contributes none.
fn raw_text(tree: &OpenXmlElementTree, index: usize) -> Cow<'_, str> {
    let element = &tree.nodes[index];
    match &element.data {
        OpenXmlElementData::Text { text } => Cow::Borrowed(text),
        OpenXmlElementData::Table(_) => Cow::Owned(table_text(tree, &element.base.children)),
        OpenXmlElementData::Paragraph { .. }
        | OpenXmlElementData::Run { .. }
        | OpenXmlElementData::Hyperlink(_)
        | OpenXmlElementData::SmartTag(_)
        | OpenXmlElementData::Inserted
        | OpenXmlElementData::SimpleField { .. }
        | OpenXmlElementData::Math {
            r#type: DomType::MmlMath | DomType::MmlMathParagraph,
            ..
        } => Cow::Owned(
            element
                .base
                .children
                .iter()
                .map(|&child| raw_text(tree, child))
                .collect(),
        ),
        OpenXmlElementData::Math { .. }
        | OpenXmlElementData::Row(_)
        | OpenXmlElementData::Cell(_) => Cow::Owned(joined_text(tree, &element.base.children, " ")),
        OpenXmlElementData::DeletedText { .. }
        | OpenXmlElementData::Instruction { .. }
        | OpenXmlElementData::Break { .. }
        | OpenXmlElementData::NoBreakHyphen
        | OpenXmlElementData::Tab
        | OpenXmlElementData::Symbol { .. }
        | OpenXmlElementData::FootnoteReference { .. }
        | OpenXmlElementData::EndnoteReference { .. }
        | OpenXmlElementData::ComplexField { .. }
        | OpenXmlElementData::Deleted
        | OpenXmlElementData::AltChunk { .. }
        | OpenXmlElementData::Drawing { .. }
        | OpenXmlElementData::Image { .. }
        | OpenXmlElementData::VmlPicture
        | OpenXmlElementData::VmlElement(_)
        | OpenXmlElementData::BookmarkStart { .. }
        | OpenXmlElementData::BookmarkEnd { .. }
        | OpenXmlElementData::CommentReference(_)
        | OpenXmlElementData::CommentRangeStart(_)
        | OpenXmlElementData::CommentRangeEnd(_) => Cow::Borrowed(""),
    }
}

/// Nonempty trimmed element texts joined with one separator.
fn joined_text(tree: &OpenXmlElementTree, children: &[usize], separator: &str) -> String {
    children
        .iter()
        .map(|&index| extract_text(tree, index))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(separator)
}

/// Table text with one line per row and a bar between cells.
fn table_text(tree: &OpenXmlElementTree, rows: &[usize]) -> String {
    let lines: Vec<String> = rows
        .iter()
        .filter_map(|&index| row_text(tree, index))
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!("\n[Table]\n{}\n[/Table]\n", lines.join("\n"))
}

/// One row's nonempty cells joined by a bar.
fn row_text(tree: &OpenXmlElementTree, index: usize) -> Option<String> {
    let cells: Vec<String> = tree.nodes[index]
        .base
        .children
        .iter()
        .map(|&cell| joined_text(tree, &tree.nodes[cell].base.children, " "))
        .filter(|text| !text.is_empty())
        .collect();
    (!cells.is_empty()).then(|| cells.join(" | "))
}
