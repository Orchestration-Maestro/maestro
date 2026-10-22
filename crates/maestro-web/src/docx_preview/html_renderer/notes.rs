//! Note references and the note lists that close a page.
use super::HtmlRenderer;
use crate::docx_preview::html_element::HtmlElement;
use maestro_app::browser::docx_preview::WmlBaseNote;

impl<'a> HtmlRenderer<'a> {
    /// Render a list of the given notes, or nothing when none were found.
    pub(super) fn render_notes(&mut self, notes: Vec<&'a WmlBaseNote>) -> Option<HtmlElement> {
        if notes.is_empty() {
            return None;
        }
        let mut list = HtmlElement::new("ol");
        for note in notes {
            let mut item = HtmlElement::new("li");
            item.append(self.render_elements(&note.children, &note.children.roots));
            list.push_element(item);
        }
        Some(list)
    }

    /// Number a footnote reference by its place among the page's footnote references.
    pub(super) fn render_footnote_reference(&mut self, id: Option<&'a str>) -> HtmlElement {
        self.current_footnote_ids.push(id);
        note_number(self.current_footnote_ids.len())
    }

    /// Number an endnote reference by its place among the document's endnote references.
    pub(super) fn render_endnote_reference(&mut self, id: Option<&'a str>) -> HtmlElement {
        self.current_endnote_ids.push(id);
        note_number(self.current_endnote_ids.len())
    }
}

/// Select the referenced notes in reference order; a repeated identifier selects its last
/// note, and a reference without a note selects nothing.
pub(super) fn find_notes<'a>(
    note_ids: &[Option<&'a str>],
    notes: &'a [WmlBaseNote],
) -> Vec<&'a WmlBaseNote> {
    note_ids
        .iter()
        .filter_map(|&id| notes.iter().rev().find(|note| note.id.as_deref() == id))
        .collect()
}

/// Write a reference number as superscript text.
fn note_number(number: usize) -> HtmlElement {
    let mut element = HtmlElement::new("sup");
    element.push_text(&number.to_string());
    element
}
