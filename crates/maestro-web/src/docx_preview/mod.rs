//! Word documents rendered as element trees for a browser to mount.
//!
//! [`parse_async`] reads a package with the application's Word readers, and
//! [`WordPreview::render`] composes one rendering: the library stylesheet, theme variables,
//! document and list styles, embedded font faces and the pages, wrapped or not. Section groups
//! and page breaks split the pages, each page carries its margins, size, header, footer and
//! note lists, tables keep their columns and merged cells, and pages hold images, drawings,
//! shapes as `SVG`, equations as `MathML` and alternate format chunks. In a browser,
//! `render_document` and `render_async` mount a rendering as nodes.
#[cfg(target_arch = "wasm32")]
mod browser;
mod html_element;
mod html_renderer;
#[cfg(test)]
mod tests;
mod utils;

#[cfg(target_arch = "wasm32")]
pub use browser::{RenderedDocument, render_async, render_document};
pub use html_element::{HtmlContent, HtmlElement, Namespace};
use html_renderer::{HtmlRenderer, Styles, process_element};
use maestro_app::browser::docx_preview::font_table::{FontDeclaration, FontTablePart};
use maestro_app::browser::docx_preview::numbering::NumberingPart;
use maestro_app::browser::docx_preview::{
    DmlTheme, DocxError, EndnotesPart, FooterPart, FootnotesPart, HeaderPart, IDomNumbering,
    OpenXmlElementTree, Options as PackageOptions, OwnedXmlElement, Part, PartKind, StylesPart,
    ThemePart, WmlBaseNote, WordBody, WordDocument, XmlParser, parse_document_file,
    parse_xml_string,
};
use std::collections::HashMap;

/// Rendering options. The defaults use the `docx` class namespace, trim XML declarations,
/// detect page breaks, ignore last-rendered page breaks, keep page sizes, render every header,
/// footer and note list, wrap the pages and load fonts, images and chunks.
#[derive(Clone, Debug)]
pub struct Options {
    /// Class namespace of pages and style classes.
    pub class_name: String,
    /// Remove a leading XML declaration before parsing each part.
    pub trim_xml_declaration: bool,
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
    /// Page wrapper rendering.
    pub wrapper: WrapperOptions,
    /// Package resources a rendering loads.
    pub resources: ResourceOptions,
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

/// Page wrapper rendering; defaults: wrapped, wrapper rules kept when printing.
#[derive(Clone, Copy, Debug)]
pub struct WrapperOptions {
    /// Enclose the pages in a gray page frame.
    pub in_wrapper: bool,
    /// Apply the page frame's rules only when not printing.
    pub hide_wrapper_on_print: bool,
}

/// Package resources; defaults: fonts loaded, object URLs, alternate chunks rendered.
#[derive(Clone, Copy, Debug)]
pub struct ResourceOptions {
    /// Leave embedded fonts unloaded.
    pub ignore_fonts: bool,
    /// Name images and fonts by data URL instead of object URL.
    pub use_base64_url: bool,
    /// Render alternate format chunks in frames.
    pub render_alt_chunks: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            class_name: "docx".into(),
            trim_xml_declaration: true,
            break_pages: true,
            ignore_last_rendered_page_break: true,
            page_size: PageSizeOptions::default(),
            headers_footers: HeaderFooterOptions::default(),
            notes: NoteOptions::default(),
            wrapper: WrapperOptions::default(),
            resources: ResourceOptions::default(),
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

impl Default for WrapperOptions {
    fn default() -> Self {
        Self {
            in_wrapper: true,
            hide_wrapper_on_print: false,
        }
    }
}

impl Default for ResourceOptions {
    fn default() -> Self {
        Self {
            ignore_fonts: false,
            use_base64_url: false,
            render_alt_chunks: true,
        }
    }
}

/// A Word package prepared for rendering.
pub struct WordPreview {
    /// Options the package was read with.
    options: Options,
    /// Opened package, for relationship and resource lookups.
    document: WordDocument,
    /// Body content, the body's own section group and the background.
    body: WordBody,
    /// Styles, theme, numbering, fonts, notes, headers and footers.
    parts: Parts,
}

/// Style and body content of one rendering, in mount order.
pub struct Rendering {
    /// Library defaults, theme variables, document styles and list styles.
    pub styles: Vec<HtmlElement>,
    /// The page wrapper, or the pages when no wrapper is requested.
    pub body: Vec<HtmlElement>,
    /// Picture-bullet images and font faces; in a shared container they follow the body.
    pub resource_styles: Vec<HtmlElement>,
    /// Object URLs the elements name, revoked when dropped.
    #[cfg(target_arch = "wasm32")]
    object_urls: Vec<crate::utils::download::ObjectUrl>,
}

/// Parts the rendering uses besides the body.
#[derive(Default)]
struct Parts {
    /// Resolved styles, when the package has a styles part.
    styles: Option<Styles>,
    /// Theme colors and fonts.
    theme: Option<DmlTheme>,
    /// Concrete numbering levels.
    numbering: Option<PartRecords<IDomNumbering>>,
    /// Font declarations with their embedded faces.
    font_table: Option<PartRecords<FontDeclaration>>,
    /// Footnotes.
    footnotes: Option<PartRecords<WmlBaseNote>>,
    /// Endnotes.
    endnotes: Option<PartRecords<WmlBaseNote>>,
    /// Header and footer content by part path.
    headers_footers: HashMap<String, OpenXmlElementTree>,
}

/// Records of one part in source order and the package position of the part holding them,
/// whose relationships their resources resolve against.
struct PartRecords<T> {
    /// Position of the part among the package parts.
    part: usize,
    /// Records in source order.
    records: Vec<T>,
}

/// Read a Word package and prepare its body, styles, theme, numbering, fonts, notes, headers
/// and footers.
///
/// # Errors
/// Returns package or XML failures from the Word readers, or an XML failure for a package
/// without a main document.
pub fn parse_async(data: &[u8], options: Options) -> Result<WordPreview, DocxError> {
    let package = PackageOptions {
        trim_xml_declaration: options.trim_xml_declaration,
    };
    let document = WordDocument::load(data, package)?;
    let ignore_width = options.page_size.ignore_width;
    let part = document.document_part().ok_or_else(|| DocxError::Xml {
        path: None,
        message: "Word package has no main document".into(),
    })?;
    let mut body = parse_part(part, &options, |xml| {
        parse_document_file(&OwnedXmlElement::new(xml.root()), ignore_width)
    })?;
    process_element(&mut body.children);
    let mut parts = Parts::default();
    for (index, part) in document.parts().iter().enumerate() {
        parts.prepare(index, part, &options)?;
    }
    if let (Some(styles), Some(numbering)) = (&mut parts.styles, &parts.numbering) {
        styles.prodess_numberings(&numbering.records);
    }
    Ok(WordPreview {
        options,
        document,
        body,
        parts,
    })
}

impl WordPreview {
    /// Render the whole preview with the supplied options.
    #[must_use]
    pub fn render(&self, options: &Options) -> Rendering {
        HtmlRenderer::new(self, options).render()
    }

    /// Render the body as page elements in document order, with the options the package was
    /// read with. Object URLs the pages name are revoked when this returns; a [`Rendering`]
    /// keeps them.
    #[must_use]
    pub fn render_sections(&self) -> Vec<HtmlElement> {
        HtmlRenderer::new(self, &self.options).render_sections()
    }

    /// Borrow the header or footer part a document relationship names, with its content.
    fn header_footer(&self, id: &str) -> Option<(&Part, &OpenXmlElementTree)> {
        let part = self
            .document
            .find_part_by_rel_id(id, self.document.document_part())?;
        let tree = self.parts.headers_footers.get(&part.path)?;
        Some((part, tree))
    }

    /// Borrow the package part at a position.
    fn part<T>(&self, records: Option<&PartRecords<T>>) -> Option<&Part> {
        self.document.parts().get(records?.part)
    }
}

impl Parts {
    /// Prepare one part the rendering uses, given its package position; the last part of each
    /// kind wins.
    fn prepare(&mut self, index: usize, part: &Part, options: &Options) -> Result<(), DocxError> {
        let ignore_width = options.page_size.ignore_width;
        let root = |xml: &XmlParser<'_>| OwnedXmlElement::new(xml.root());
        match part.kind {
            PartKind::Styles => {
                let styles = parse_part(part, options, |xml| {
                    StylesPart::parse_xml(xml, xml.root(), ignore_width)
                })?;
                self.styles = Some(Styles::process_styles(&styles.styles));
            }
            PartKind::Theme => {
                let theme = parse_part(part, options, |xml| ThemePart::parse_xml(xml, xml.root()))?;
                self.theme = Some(theme.theme);
            }
            PartKind::Numbering => {
                let numbering = parse_part(part, options, |xml| {
                    Ok(NumberingPart::parse_xml(xml, xml.root(), ignore_width))
                })?;
                self.numbering = Some(PartRecords::new(index, numbering.dom_numberings));
            }
            PartKind::FontTable => {
                let fonts = parse_part(part, options, |xml| {
                    Ok(FontTablePart::parse_xml(xml, xml.root()))
                })?;
                self.font_table = Some(PartRecords::new(index, fonts.fonts));
            }
            PartKind::Footnotes => {
                let notes = parse_part(part, options, |xml| {
                    FootnotesPart::parse_xml(&root(xml), ignore_width)
                })?;
                let notes = notes.notes.into_iter().map(|note| note.base).collect();
                self.footnotes = Some(PartRecords::new(index, notes));
            }
            PartKind::Endnotes => {
                let notes = parse_part(part, options, |xml| {
                    EndnotesPart::parse_xml(&root(xml), ignore_width)
                })?;
                let notes = notes.notes.into_iter().map(|note| note.base).collect();
                self.endnotes = Some(PartRecords::new(index, notes));
            }
            PartKind::Header | PartKind::Footer => self.add_header_footer(part, options)?,
            _ => {}
        }
        Ok(())
    }

    /// Keep a header or footer's content, its tables' cell declarations handed down.
    fn add_header_footer(&mut self, part: &Part, options: &Options) -> Result<(), DocxError> {
        let ignore_width = options.page_size.ignore_width;
        let mut tree = parse_part(part, options, |xml| {
            let root = OwnedXmlElement::new(xml.root());
            Ok(if part.kind == PartKind::Header {
                HeaderPart::parse_xml(&root, ignore_width)?
                    .root_element
                    .children
            } else {
                FooterPart::parse_xml(&root, ignore_width)?
                    .root_element
                    .children
            })
        })?;
        process_element(&mut tree);
        self.headers_footers.insert(part.path.clone(), tree);
        Ok(())
    }
}

impl<T> PartRecords<T> {
    /// Keep a part's records with its package position.
    fn new(part: usize, records: Vec<T>) -> Self {
        Self { part, records }
    }
}

/// Parse one part's XML, naming the part in an XML failure.
fn parse_part<T>(
    part: &Part,
    options: &Options,
    parse: impl FnOnce(&XmlParser<'_>) -> Result<T, DocxError>,
) -> Result<T, DocxError> {
    let xml = parse_xml_string(&part.xml, options.trim_xml_declaration)?;
    parse(&xml).map_err(|mut error| {
        if let DocxError::Xml { path, .. } = &mut error {
            path.get_or_insert_with(|| part.path.clone());
        }
        error
    })
}
