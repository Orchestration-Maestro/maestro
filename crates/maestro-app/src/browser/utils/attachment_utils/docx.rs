//! Word package text extraction.
use super::append_extracted_text;
use crate::browser::docx_preview::{
    DocxError, DomType, OpenXmlElementData, OpenXmlElementTree, Options, WordDocument,
};

/// Extract the ordered body text inside the literal page wrapper.
pub(super) fn process_docx(bytes: &[u8], file_name: &str) -> Result<String, DocxError> {
    let body = WordDocument::load(bytes, Options::default())?.document_body()?;
    let mut output = String::new();
    append(
        &mut output,
        &format!("<docx filename=\"{file_name}\">\n<page number=\"1\">\n"),
    )?;
    let text = body
        .as_ref()
        .map_or_else(String::new, |tree| joined_text(tree, &tree.roots, "\n"));
    append(&mut output, &text)?;
    append(&mut output, "\n</page>\n</docx>")?;
    Ok(output)
}

/// Append one extracted piece while it fits the document text bound.
fn append(output: &mut String, text: &str) -> Result<(), DocxError> {
    append_extracted_text(output, text).map_err(DocxError::Output)
}

/// Trimmed text of one element.
fn extract_text(tree: &OpenXmlElementTree, index: usize) -> String {
    raw_text(tree, index)
        .trim_matches(maestro_text::is_whitespace_scalar)
        .to_owned()
}

/// Untrimmed text of one element, following its own child rule.
fn raw_text(tree: &OpenXmlElementTree, index: usize) -> String {
    let element = &tree.nodes[index];
    match &element.data {
        OpenXmlElementData::Text { text } => text.clone(),
        OpenXmlElementData::Table(_) => table_text(tree, &element.base.children),
        OpenXmlElementData::Paragraph { .. }
        | OpenXmlElementData::Run { .. }
        | OpenXmlElementData::Hyperlink(_)
        | OpenXmlElementData::SmartTag(_)
        | OpenXmlElementData::Inserted
        | OpenXmlElementData::Deleted
        | OpenXmlElementData::SimpleField { .. }
        | OpenXmlElementData::ComplexField { .. }
        | OpenXmlElementData::Math {
            r#type: DomType::MmlMath | DomType::MmlMathParagraph,
            ..
        } => element
            .base
            .children
            .iter()
            .map(|&child| raw_text(tree, child))
            .collect(),
        _ => joined_text(tree, &element.base.children, " "),
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
    if !matches!(tree.nodes[index].data, OpenXmlElementData::Row(_)) {
        return None;
    }
    let cells: Vec<String> = tree.nodes[index]
        .base
        .children
        .iter()
        .filter(|&&cell| matches!(tree.nodes[cell].data, OpenXmlElementData::Cell(_)))
        .map(|&cell| joined_text(tree, &tree.nodes[cell].base.children, " "))
        .filter(|text| !text.is_empty())
        .collect();
    (!cells.is_empty()).then(|| cells.join(" | "))
}
