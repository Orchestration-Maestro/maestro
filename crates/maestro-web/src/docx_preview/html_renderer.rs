//! Rendering state, the composition of one rendering, style lookup and the element dispatch.
use super::html_element::{HtmlContent, HtmlElement, Namespace};
use super::utils::escape_class_name;
use super::{Options, Rendering, Sections, WordPreview};
use indexmap::IndexMap;
use maestro_app::browser::docx_preview::{
    DomType, OpenXmlElementData as Data, OpenXmlElementTree, Part,
};

mod drawing;
mod header_footer;
pub(super) mod inline;
mod math;
mod notes;
mod numbering;
mod pages;
mod resources;
mod styles;
mod stylesheet;
mod tables;
mod vml;

use notes::NoteMap;
use pages::Slice;
use styles::ParagraphStyle;
pub(crate) use styles::Styles;
#[cfg(test)]
pub(crate) use styles::steps_taken;

/// Ordered declarations as the document parser produces them.
pub(crate) type CssStyle = IndexMap<String, Option<String>>;

/// Cell borders and margins a table hands to the cells that declare none.
const CELL_STYLE_KEYS: [&str; 8] = [
    "border-left",
    "border-right",
    "border-top",
    "border-bottom",
    "padding-left",
    "padding-right",
    "padding-top",
    "padding-bottom",
];

/// State of one rendering over a prepared document.
pub(crate) struct HtmlRenderer<'a> {
    /// Prepared package and parts.
    preview: &'a WordPreview,
    /// Rendering options.
    options: &'a Options,
    /// Part whose relationships the links and images being rendered resolve against.
    current_part: Option<&'a Part>,
    /// Footnotes by identifier.
    footnote_map: NoteMap<'a>,
    /// Endnotes by identifier.
    endnote_map: NoteMap<'a>,
    /// Footnote references placed on the current page.
    current_footnote_ids: Vec<Option<&'a str>>,
    /// Endnote references placed so far in the document.
    current_endnote_ids: Vec<Option<&'a str>>,
    /// Picture-bullet images and font faces, in load order.
    resource_styles: Vec<HtmlElement>,
    /// Object URLs the rendered elements name.
    #[cfg(target_arch = "wasm32")]
    object_urls: Vec<crate::utils::download::ObjectUrl>,
}

/// Children a paragraph or run renders: its own, or the ones a page split kept.
#[derive(Clone, Copy)]
pub(crate) enum Children<'s> {
    /// Every child the element owns.
    Own,
    /// The children kept on this side of a page split.
    Kept(&'s [Slice]),
}

impl<'a> HtmlRenderer<'a> {
    /// Start a rendering in the document part with empty note lists.
    pub(crate) fn new(preview: &'a WordPreview, options: &'a Options) -> Self {
        Self {
            preview,
            options,
            current_part: preview.document.document_part(),
            footnote_map: NoteMap::new(preview, preview.parts.footnotes.as_ref()),
            endnote_map: NoteMap::new(preview, preview.parts.endnotes.as_ref()),
            current_footnote_ids: Vec::new(),
            current_endnote_ids: Vec::new(),
            resource_styles: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            object_urls: Vec::new(),
        }
    }

    /// Render the library styles, then the theme, document and list styles the package has,
    /// the font faces unless fonts are ignored, and the pages, wrapped when requested.
    pub(crate) fn render(mut self) -> Rendering {
        let (preview, options) = (self.preview, self.options);
        let class_name = options.class_name.as_str();
        let parts = &preview.parts;
        let mut styles = vec![stylesheet::render_default_style(
            class_name,
            options.wrapper.hide_wrapper_on_print,
        )];
        styles.extend(
            parts
                .theme
                .as_ref()
                .map(|theme| stylesheet::render_theme(theme, class_name)),
        );
        styles.extend(
            parts
                .styles
                .as_ref()
                .map(|document_styles| document_styles.render_styles(class_name)),
        );
        if let Some(numbering) = &parts.numbering {
            styles.push(self.render_numbering(numbering));
        }
        if !options.resources.ignore_fonts
            && let Some(fonts) = &parts.font_table
        {
            self.render_font_table(fonts);
        }
        let pages = self.render_sections();
        let body = if options.wrapper.in_wrapper {
            vec![stylesheet::render_wrapper(class_name, pages)]
        } else {
            pages
        };
        Rendering {
            styles,
            body,
            #[cfg(any(target_arch = "wasm32", test))]
            resource_styles: self.resource_styles,
            #[cfg(target_arch = "wasm32")]
            object_urls: self.object_urls,
        }
    }

    /// Render the pages, keeping the object URLs they name with them.
    pub(crate) fn into_sections(mut self) -> Sections {
        let pages = self.render_sections();
        Sections {
            pages,
            #[cfg(target_arch = "wasm32")]
            _urls: self.object_urls,
        }
    }

    /// Borrow the rendering options.
    fn options(&self) -> &'a Options {
        self.options
    }

    /// Borrow the resolved paragraph values for a style name.
    fn find_style(&self, style_name: Option<&str>) -> Option<&'a ParagraphStyle> {
        self.preview.parts.styles.as_ref()?.find_style(style_name)
    }

    /// Replace the class with authored classes, then add the style's class token.
    fn render_class(
        &self,
        class_name: Option<&str>,
        style_name: Option<&str>,
        output: &mut HtmlElement,
    ) {
        if let Some(class_name) = class_name.filter(|name| !name.is_empty()) {
            output.set_attribute("class", class_name);
        }
        if let Some(style_name) = style_name.filter(|name| !name.is_empty()) {
            output.add_class(&process_style_name(
                &self.options().class_name,
                Some(style_name),
            ));
        }
    }

    /// Render elements in order, dropping the kinds that render nothing.
    fn render_elements(
        &mut self,
        tree: &'a OpenXmlElementTree,
        indices: &[usize],
    ) -> Vec<HtmlContent> {
        let mut output = Vec::new();
        self.render_elements_into(tree, indices, &mut output);
        output
    }

    /// Append rendered elements in order.
    fn render_elements_into(
        &mut self,
        tree: &'a OpenXmlElementTree,
        indices: &[usize],
        output: &mut Vec<HtmlContent>,
    ) {
        for &index in indices {
            self.render_element(tree, index, Children::Own, output);
        }
    }

    /// Render elements whose links and images resolve against a part's relationships, then
    /// return to the enclosing part.
    fn render_part_elements(
        &mut self,
        part: Option<&'a Part>,
        tree: &'a OpenXmlElementTree,
        indices: &[usize],
    ) -> Vec<HtmlContent> {
        let enclosing = std::mem::replace(&mut self.current_part, part);
        let output = self.render_elements(tree, indices);
        self.current_part = enclosing;
        output
    }

    /// Render page slices in order.
    fn render_slices(
        &mut self,
        tree: &'a OpenXmlElementTree,
        slices: &[Slice],
    ) -> Vec<HtmlContent> {
        let mut output = Vec::new();
        for slice in slices {
            self.render_element(tree, slice.index, slice.children(), &mut output);
        }
        output
    }

    /// Render an element's own children or the ones a page split kept.
    fn render_children(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        children: Children<'_>,
    ) -> Vec<HtmlContent> {
        match children {
            Children::Own => self.render_elements(tree, &tree.nodes[index].base.children),
            Children::Kept(slices) => self.render_slices(tree, slices),
        }
    }

    /// Render an element's own children into a new HTML element.
    fn render_container(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        tag_name: &'static str,
    ) -> HtmlElement {
        self.render_container_ns(tree, index, Namespace::Html, tag_name)
    }

    /// Render an element's own children into a new element of a namespace.
    fn render_container_ns(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        namespace: Namespace,
        tag_name: &'static str,
    ) -> HtmlElement {
        let mut element = HtmlElement::new_ns(namespace, tag_name);
        element.append(self.render_elements(tree, &tree.nodes[index].base.children));
        element
    }

    /// Append one element by kind: text as text, inserted content as its children, any other
    /// kind as its element.
    fn render_element(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        children: Children<'_>,
        output: &mut Vec<HtmlContent>,
    ) {
        let node = &tree.nodes[index];
        match &node.data {
            Data::Text { text } => output.push(HtmlContent::Text(text.clone())),
            Data::NoBreakHyphen => output.push(HtmlContent::Text("\u{2011}".into())),
            Data::Inserted => self.render_elements_into(tree, &node.base.children, output),
            _ => output.extend(
                self.render_node(tree, index, children)
                    .map(HtmlContent::Element),
            ),
        }
    }

    /// Render one element by kind; kinds without output here render nothing.
    fn render_node(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        children: Children<'_>,
    ) -> Option<HtmlElement> {
        let element = match &tree.nodes[index].data {
            Data::Paragraph { properties } => {
                self.render_paragraph(tree, index, properties, children)
            }
            Data::BookmarkStart { name, .. } => inline::render_bookmark_start(name.as_deref()),
            Data::Run {
                r#type: DomType::Run,
                field_run: false,
                properties,
            } => self.render_run(tree, index, properties, children),
            Data::Run {
                r#type: DomType::MmlRun,
                properties,
                ..
            } => self.render_mml_run(tree, index, properties),
            Data::Table(table) => self.render_table(tree, index, table),
            Data::Hyperlink(link) => self.render_hyperlink(tree, index, link),
            Data::SmartTag(_) => self.render_container(tree, index, "span"),
            Data::Tab => inline::render_tab(),
            Data::Symbol { font, char: code } => {
                inline::render_symbol(font.as_deref(), code.as_deref())
            }
            Data::Break { r#break } => return inline::render_break(r#break),
            Data::FootnoteReference { id } => self.render_footnote_reference(id.as_deref()),
            Data::EndnoteReference { id } => self.render_endnote_reference(id.as_deref()),
            Data::Drawing { css_style } => self.render_drawing(tree, index, css_style),
            Data::Image {
                src,
                css_style,
                src_rect,
                rotation,
            } => self.render_image(src.as_deref(), css_style, *src_rect, *rotation),
            Data::VmlPicture => self.render_container(tree, index, "div"),
            Data::VmlElement(shape) => self.render_vml_element(tree, index, shape),
            Data::Math { r#type, props } => {
                return self.render_math(tree, index, *r#type, props.as_ref());
            }
            Data::AltChunk { id } => return self.render_alt_chunk(id.as_deref()),
            _ => return None,
        };
        Some(element)
    }
}

/// Name a style's class: the class namespace alone for an absent or empty identifier,
/// otherwise the namespace and the escaped identifier.
fn process_style_name(class_name: &str, style_name: Option<&str>) -> String {
    match style_name.filter(|name| !name.is_empty()) {
        Some(name) => format!("{class_name}_{}", escape_class_name(name)),
        None => class_name.to_owned(),
    }
}

/// Write declarations in order; a `$` key names an attribute instead.
fn render_style_values(values: Option<&CssStyle>, output: &mut HtmlElement) {
    for (key, value) in values.into_iter().flatten() {
        match key.strip_prefix('$') {
            Some(attribute) => output.set_attribute(attribute, value.as_deref().unwrap_or("null")),
            None => output.set_style(key, value.as_deref()),
        }
    }
}

/// Fill the requested declarations the destination lacks from the source.
fn copy_style_properties(input: &CssStyle, output: &mut CssStyle, attrs: &[&str]) {
    for &key in attrs {
        if let Some(value) = input.get(key)
            && !output.contains_key(key)
        {
            output.insert(key.to_owned(), value.clone());
        }
    }
}

/// Hand each table's cell borders and margins to the cells of its rows.
///
/// Every table in the arena belongs to the tree, so visiting the arena reaches each table once.
pub(crate) fn process_element(tree: &mut OpenXmlElementTree) {
    for table in 0..tree.nodes.len() {
        process_table(tree, table);
    }
}

/// Copy one table's cell declarations into the cells that lack them.
fn process_table(tree: &mut OpenXmlElementTree, table: usize) {
    let Data::Table(record) = &mut tree.nodes[table].data else {
        return;
    };
    let Some(cell_style) = record.cell_style.take() else {
        return;
    };
    let cells: Vec<usize> = tree.nodes[table]
        .base
        .children
        .iter()
        .flat_map(|&row| tree.nodes[row].base.children.iter().copied())
        .collect();
    for cell in cells {
        if let Data::Cell(cell) = &mut tree.nodes[cell].data {
            copy_style_properties(
                &cell_style,
                cell.css_style.get_or_insert_default(),
                &CELL_STYLE_KEYS,
            );
        }
    }
    if let Data::Table(record) = &mut tree.nodes[table].data {
        record.cell_style = Some(cell_style);
    }
}
