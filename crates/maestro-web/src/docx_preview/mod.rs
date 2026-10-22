//! Word document pages rendered as HTML text.
//!
//! [`WordPreview::load`] reads a package with the application's Word readers, and
//! [`WordPreview::render_sections`] lays the body out as page elements: section groups and
//! page breaks split the pages, each page carries its margins, size, header, footer and
//! note lists, and tables keep their columns and merged cells. [`HtmlElement::to_html`]
//! writes a page as escaped HTML text for a browser to mount.
mod html_element;
mod html_renderer;
#[cfg(test)]
mod tests;
mod utils;

pub use html_element::HtmlElement;
use html_renderer::{HtmlRenderer, Styles, process_element};
use maestro_app::browser::docx_preview::{
    DocxError, EndnotesPart, FooterPart, FootnotesPart, HeaderPart, OpenXmlElementTree,
    Options as PackageOptions, OwnedXmlElement, Part, PartKind, StylesPart, WmlBaseNote, WordBody,
    WordDocument, XmlParser, parse_document_file, parse_xml_string,
};
use std::collections::HashMap;

/// Page rendering options. The defaults use the `docx` class namespace, detect page breaks,
/// ignore last-rendered page breaks, keep page sizes and render every header, footer and
/// note list.
#[derive(Clone, Debug)]
pub struct Options {
    /// Class namespace of pages and style classes.
    pub class_name: String,
    /// Split pages at page breaks inside paragraphs.
    pub break_pages: bool,
    /// Ignore last-rendered page breaks; each section then starts a new page.
    pub ignore_last_rendered_page_break: bool,
    /// Page and cell dimensions left unset.
    pub page_size: PageSizeOptions,
    /// Header and footer rendering.
    pub headers_footers: HeaderFooterOptions,
    /// Note list rendering.
    pub notes: NoteOptions,
}

/// Page and cell dimensions left unset; both are kept by default.
#[derive(Clone, Copy, Debug, Default)]
pub struct PageSizeOptions {
    /// Leave page widths and cell widths unset.
    pub ignore_width: bool,
    /// Leave page heights unset.
    pub ignore_height: bool,
}

/// Header and footer rendering; both are rendered by default.
#[derive(Clone, Copy, Debug)]
pub struct HeaderFooterOptions {
    /// Render page headers.
    pub render_headers: bool,
    /// Render page footers.
    pub render_footers: bool,
}

/// Note list rendering; both lists are rendered by default.
#[derive(Clone, Copy, Debug)]
pub struct NoteOptions {
    /// Render footnote lists at the end of their pages.
    pub render_footnotes: bool,
    /// Render the endnote list at the end of the last page.
    pub render_endnotes: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            class_name: "docx".into(),
            break_pages: true,
            ignore_last_rendered_page_break: true,
            page_size: PageSizeOptions::default(),
            headers_footers: HeaderFooterOptions::default(),
            notes: NoteOptions::default(),
        }
    }
}

impl Default for HeaderFooterOptions {
    fn default() -> Self {
        Self {
            render_headers: true,
            render_footers: true,
        }
    }
}

impl Default for NoteOptions {
    fn default() -> Self {
        Self {
            render_footnotes: true,
            render_endnotes: true,
        }
    }
}

/// A Word package prepared for page rendering.
pub struct WordPreview {
    /// Rendering options.
    options: Options,
    /// Opened package, for relationship lookups.
    document: WordDocument,
    /// Body content, the body's own section group and the background.
    body: WordBody,
    /// Styles, notes, headers and footers.
    parts: Parts,
}

/// Parts the pages use besides the body.
#[derive(Default)]
struct Parts {
    /// Resolved styles.
    styles: Styles,
    /// Footnotes and the part holding them.
    footnotes: Option<Notes>,
    /// Endnotes and the part holding them.
    endnotes: Option<Notes>,
    /// Header and footer content by part path.
    headers_footers: HashMap<String, OpenXmlElementTree>,
}

/// Notes of one kind in source order and the package position of the part holding them.
struct Notes {
    /// Position of the notes part among the package parts.
    part: usize,
    /// Notes in source order.
    notes: Vec<WmlBaseNote>,
}

impl WordPreview {
    /// Read the package and prepare its body, styles, notes, headers and footers.
    ///
    /// # Errors
    /// Returns package or XML failures from the Word readers, or an XML failure for a package
    /// without a main document.
    pub fn load(data: &[u8], options: Options) -> Result<Self, DocxError> {
        let document = WordDocument::load(data, PackageOptions::default())?;
        let ignore_width = options.page_size.ignore_width;
        let part = document.document_part().ok_or_else(|| DocxError::Xml {
            path: None,
            message: "Word package has no main document".into(),
        })?;
        let mut body = parse_part(part, |xml| {
            parse_document_file(&OwnedXmlElement::new(xml.root()), ignore_width)
        })?;
        process_element(&mut body.children);
        let mut parts = Parts::default();
        for (index, part) in document.parts().iter().enumerate() {
            parts.prepare(index, part, ignore_width)?;
        }
        Ok(Self {
            options,
            document,
            body,
            parts,
        })
    }

    /// Render the body as page elements in document order.
    #[must_use]
    pub fn render_sections(&self) -> Vec<HtmlElement> {
        HtmlRenderer::new(self).render_sections()
    }

    /// Borrow the header or footer part a document relationship names, with its content.
    fn header_footer(&self, id: &str) -> Option<(&Part, &OpenXmlElementTree)> {
        let part = self
            .document
            .find_part_by_rel_id(id, self.document.document_part())?;
        let tree = self.parts.headers_footers.get(&part.path)?;
        Some((part, tree))
    }
}

impl Parts {
    /// Prepare one part the pages use, given its package position; the last styles and notes
    /// parts win.
    fn prepare(&mut self, index: usize, part: &Part, ignore_width: bool) -> Result<(), DocxError> {
        let root = |xml: &XmlParser<'_>| OwnedXmlElement::new(xml.root());
        match part.kind {
            PartKind::Styles => {
                let styles = parse_part(part, |xml| {
                    StylesPart::parse_xml(xml, xml.root(), ignore_width)
                })?;
                self.styles = Styles::process_styles(&styles.styles);
            }
            PartKind::Footnotes => {
                let notes = parse_part(part, |xml| {
                    FootnotesPart::parse_xml(&root(xml), ignore_width)
                })?;
                self.footnotes = Some(Notes {
                    part: index,
                    notes: notes.notes.into_iter().map(|note| note.base).collect(),
                });
            }
            PartKind::Endnotes => {
                let notes = parse_part(part, |xml| {
                    EndnotesPart::parse_xml(&root(xml), ignore_width)
                })?;
                self.endnotes = Some(Notes {
                    part: index,
                    notes: notes.notes.into_iter().map(|note| note.base).collect(),
                });
            }
            PartKind::Header => {
                let header =
                    parse_part(part, |xml| HeaderPart::parse_xml(&root(xml), ignore_width))?;
                self.add_header_footer(part, header.root_element.children);
            }
            PartKind::Footer => {
                let footer =
                    parse_part(part, |xml| FooterPart::parse_xml(&root(xml), ignore_width))?;
                self.add_header_footer(part, footer.root_element.children);
            }
            _ => {}
        }
        Ok(())
    }

    /// Keep a header or footer's content, its tables' cell declarations handed down.
    fn add_header_footer(&mut self, part: &Part, mut tree: OpenXmlElementTree) {
        process_element(&mut tree);
        self.headers_footers.insert(part.path.clone(), tree);
    }
}

/// Parse one part's XML, naming the part in an XML failure.
fn parse_part<T>(
    part: &Part,
    parse: impl FnOnce(&XmlParser<'_>) -> Result<T, DocxError>,
) -> Result<T, DocxError> {
    let xml = parse_xml_string(&part.xml, true)?;
    parse(&xml).map_err(|mut error| {
        if let DocxError::Xml { path, .. } = &mut error {
            path.get_or_insert_with(|| part.path.clone());
        }
        error
    })
}
