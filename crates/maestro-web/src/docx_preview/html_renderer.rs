//! Page rendering state, style lookup and the element dispatch.
use super::html_element::{HtmlContent, HtmlElement};
use super::utils::escape_class_name;
use super::{Options, WordPreview};
use indexmap::IndexMap;
use maestro_app::browser::docx_preview::{
    DomType, Numbering, OpenXmlElementData as Data, OpenXmlElementTree, Part,
};

mod header_footer;
pub(super) mod inline;
mod notes;
mod pages;
mod styles;
mod tables;

use notes::NoteMap;
use pages::Slice;
use styles::ParagraphStyle;
pub(crate) use styles::Styles;

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

/// State of one page rendering over a prepared document.
pub(crate) struct HtmlRenderer<'a> {
    /// Prepared package, parts and options.
    preview: &'a WordPreview,
    /// Part whose relationships the links being rendered resolve against.
    current_part: Option<&'a Part>,
    /// Footnotes by identifier.
    footnote_map: NoteMap<'a>,
    /// Endnotes by identifier.
    endnote_map: NoteMap<'a>,
    /// Footnote references placed on the current page.
    current_footnote_ids: Vec<Option<&'a str>>,
    /// Endnote references placed so far in the document.
    current_endnote_ids: Vec<Option<&'a str>>,
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
    pub(crate) fn new(preview: &'a WordPreview) -> Self {
        Self {
            preview,
            current_part: preview.document.document_part(),
            footnote_map: NoteMap::new(preview, preview.parts.footnotes.as_ref()),
            endnote_map: NoteMap::new(preview, preview.parts.endnotes.as_ref()),
            current_footnote_ids: Vec::new(),
            current_endnote_ids: Vec::new(),
        }
    }

    /// Borrow the rendering options.
    fn options(&self) -> &'a Options {
        &self.preview.options
    }

    /// Borrow the resolved paragraph values for a style name.
    fn find_style(&self, style_name: Option<&str>) -> Option<&'a ParagraphStyle> {
        self.preview.parts.styles.find_style(style_name)
    }

    /// Prefix a nonempty style name with the class namespace.
    fn process_style_name(&self, class_name: &str) -> String {
        format!(
            "{}_{}",
            self.options().class_name,
            escape_class_name(class_name)
        )
    }

    /// Name the numbering class, spelling an absent part `undefined` and a null part `null`.
    fn numbering_class(&self, numbering: &Numbering) -> String {
        let id = match &numbering.id {
            None => "undefined",
            Some(None) => "null",
            Some(Some(id)) => id,
        };
        let level = match numbering.level {
            None => "undefined".into(),
            Some(None) => "null".into(),
            Some(Some(level)) => ryu_js::Buffer::new().format(level).to_owned(),
        };
        format!("{}-num-{id}-{level}", self.options().class_name)
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
            output.add_class(&self.process_style_name(style_name));
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

    /// Render elements whose links resolve against a part's relationships, then return to the
    /// enclosing part.
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

    /// Render an element's own children into a new element.
    fn render_container(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        tag_name: &'static str,
    ) -> HtmlElement {
        let mut element = HtmlElement::new(tag_name);
        element.append(self.render_elements(tree, &tree.nodes[index].base.children));
        element
    }

    /// Append one element by kind; kinds without output here render nothing.
    fn render_element(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        children: Children<'_>,
        output: &mut Vec<HtmlContent>,
    ) {
        let node = &tree.nodes[index];
        let element = match &node.data {
            Data::Paragraph { properties } => {
                Some(self.render_paragraph(tree, index, properties, children))
            }
            Data::BookmarkStart { name, .. } => {
                Some(inline::render_bookmark_start(name.as_deref()))
            }
            Data::Run {
                r#type: DomType::Run,
                field_run: false,
                properties,
            } => Some(self.render_run(tree, index, properties, children)),
            Data::Table(table) => Some(self.render_table(tree, index, table)),
            Data::Hyperlink(link) => Some(self.render_hyperlink(tree, index, link)),
            Data::SmartTag(_) => Some(self.render_container(tree, index, "span")),
            Data::Text { text } => {
                output.push(HtmlContent::Text(text.clone()));
                return;
            }
            Data::Tab => Some(inline::render_tab()),
            Data::Symbol { font, char: code } => {
                Some(inline::render_symbol(font.as_deref(), code.as_deref()))
            }
            Data::Break { r#break } => inline::render_break(r#break),
            Data::NoBreakHyphen => {
                output.push(HtmlContent::Text("\u{2011}".into()));
                return;
            }
            Data::FootnoteReference { id } => Some(self.render_footnote_reference(id.as_deref())),
            Data::EndnoteReference { id } => Some(self.render_endnote_reference(id.as_deref())),
            Data::Inserted => {
                self.render_elements_into(tree, &node.base.children, output);
                return;
            }
            _ => None,
        };
        output.extend(element.map(HtmlContent::Element));
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
