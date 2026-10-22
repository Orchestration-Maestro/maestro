//! Note references and the note lists that close a page.
use super::HtmlRenderer;
use crate::docx_preview::html_element::HtmlElement;
use crate::docx_preview::{Notes, WordPreview};
use maestro_app::browser::docx_preview::{Part, WmlBaseNote};
use std::collections::HashMap;

/// One kind of notes by identifier, a repeated identifier keeping its last note, and the part
/// holding them.
pub(super) struct NoteMap<'a> {
    /// Notes part, whose relationships the notes' links resolve against.
    pub(super) part: Option<&'a Part>,
    /// Notes by identifier.
    notes: HashMap<Option<&'a str>, &'a WmlBaseNote>,
}

impl<'a> NoteMap<'a> {
    /// Index the prepared notes of one kind.
    pub(super) fn new(preview: &'a WordPreview, notes: Option<&'a Notes>) -> Self {
        Self {
            part: notes.and_then(|notes| preview.document.parts().get(notes.part)),
            notes: notes
                .into_iter()
                .flat_map(|notes| &notes.notes)
                .map(|note| (note.id.as_deref(), note))
                .collect(),
        }
    }

    /// Select the referenced notes in reference order; a reference without a note selects
    /// nothing.
    pub(super) fn find_notes(&self, note_ids: &[Option<&str>]) -> Vec<&'a WmlBaseNote> {
        note_ids
            .iter()
            .filter_map(|id| self.notes.get(id).copied())
            .collect()
    }
}

impl<'a> HtmlRenderer<'a> {
    /// Render a list of the given notes against their part's relationships, or nothing when
    /// none were found.
    pub(super) fn render_notes(
        &mut self,
        part: Option<&'a Part>,
        notes: Vec<&'a WmlBaseNote>,
    ) -> Option<HtmlElement> {
        if notes.is_empty() {
            return None;
        }
        let mut list = HtmlElement::new("ol");
        for note in notes {
            let mut item = HtmlElement::new("li");
            item.append(self.render_part_elements(part, &note.children, &note.children.roots));
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

/// Write a reference number as superscript text.
fn note_number(number: usize) -> HtmlElement {
    let mut element = HtmlElement::new("sup");
    element.push_text(number.to_string());
    element
}
