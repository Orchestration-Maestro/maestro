//! Page composition: section groups, page breaks, and page and section elements.
use super::notes::find_notes;
use super::{Children, HtmlRenderer, render_style_values};
use crate::docx_preview::html_element::HtmlElement;
use maestro_app::browser::docx_preview::{
    OpenXmlElement, OpenXmlElementData as Data, OpenXmlElementTree, ParagraphProperties,
    SectionProperties,
};

/// A body element as a section holds it: whole, or the part of a split paragraph or run kept
/// on one side of a page break.
#[derive(Debug)]
pub(crate) struct Slice {
    /// Arena position of the element.
    pub(crate) index: usize,
    /// Children kept on this side of the break; absent when the element keeps all of its own.
    kept: Option<Vec<Slice>>,
}

impl Slice {
    /// Hold an element with all of its children.
    fn whole(index: usize) -> Self {
        Self { index, kept: None }
    }

    /// Hold the given children of an element.
    fn kept(index: usize, children: &[usize]) -> Self {
        Self {
            index,
            kept: Some(children.iter().map(|&child| Self::whole(child)).collect()),
        }
    }

    /// The children this slice renders.
    pub(crate) fn children(&self) -> Children<'_> {
        self.kept.as_deref().map_or(Children::Own, Children::Kept)
    }
}

/// Body elements between two section or page boundaries.
struct Section<'a> {
    /// Section group the elements use.
    sect_props: &'a SectionProperties,
    /// Elements in body order.
    elements: Vec<Slice>,
    /// Whether a page break ended the section.
    page_break: bool,
}

/// A section still collecting elements; the backward fill gives an absent group.
#[derive(Default)]
struct OpenSection<'a> {
    /// Section group that ended the section, if one did.
    sect_props: Option<&'a SectionProperties>,
    /// Elements in body order.
    elements: Vec<Slice>,
    /// Whether a page break ended the section.
    page_break: bool,
}

impl<'a> HtmlRenderer<'a> {
    /// Render the body as page elements in document order.
    pub(crate) fn render_sections(&mut self) -> Vec<HtmlElement> {
        let body = &self.preview.body;
        let pages = self.group_by_page_breaks(self.split_by_section(&body.children, &body.props));
        let mut result = Vec::with_capacity(pages.len());
        let mut prev_props = None;
        for (number, page) in pages.iter().enumerate() {
            let last = number + 1 == pages.len();
            let (element, props) = self.render_page(page, number, prev_props, last);
            result.push(element);
            prev_props = Some(props);
        }
        result
    }

    /// Render one page: header, section contents, note lists and footer.
    fn render_page(
        &mut self,
        page: &[Section<'a>],
        number: usize,
        prev_props: Option<&'a SectionProperties>,
        last: bool,
    ) -> (HtmlElement, &'a SectionProperties) {
        let body = &self.preview.body;
        self.current_footnote_ids.clear();
        let mut props = page
            .first()
            .map_or(&body.props, |section| section.sect_props);
        let mut element = self.create_page_element(props);
        render_style_values(Some(&body.css_style), &mut element);
        if self.options().headers_footers.render_headers {
            let first_of_section = !prev_props.is_some_and(|prev| std::ptr::eq(prev, props));
            let refs = props.header_refs.as_deref();
            element.extend(self.render_header_footer(refs, props, number, first_of_section));
        }
        for section in page {
            let mut content = create_section_content(section.sect_props);
            content.append(self.render_slices(&body.children, &section.elements));
            element.push_element(content);
            props = section.sect_props;
        }
        if self.options().notes.render_footnotes {
            let notes = find_notes(&self.current_footnote_ids, &self.preview.parts.footnotes);
            element.extend(self.render_notes(notes));
        }
        if self.options().notes.render_endnotes && last {
            let notes = find_notes(&self.current_endnote_ids, &self.preview.parts.endnotes);
            element.extend(self.render_notes(notes));
        }
        if self.options().headers_footers.render_footers {
            let first_of_section = !prev_props.is_some_and(|prev| std::ptr::eq(prev, props));
            let refs = props.footer_refs.as_deref();
            element.extend(self.render_header_footer(refs, props, number, first_of_section));
        }
        (element, props)
    }

    /// Start a page with the section group's margins and size.
    fn create_page_element(&self, props: &SectionProperties) -> HtmlElement {
        let mut element = HtmlElement::new("section");
        element.set_attribute("class", &self.options().class_name);
        if let Some(margins) = &props.page_margins {
            element.set_style("padding-left", margins.left.as_deref());
            element.set_style("padding-right", margins.right.as_deref());
            element.set_style("padding-top", margins.top.as_deref());
            element.set_style("padding-bottom", margins.bottom.as_deref());
        }
        if let Some(size) = &props.page_size {
            if !self.options().page_size.ignore_width {
                element.set_style("width", size.width.as_deref());
            }
            if !self.options().page_size.ignore_height {
                element.set_style("min-height", size.height.as_deref());
            }
        }
        element
    }

    /// Split body elements into sections at section groups and page breaks, then give each
    /// section without a group the next group, or the body's.
    fn split_by_section(
        &self,
        tree: &'a OpenXmlElementTree,
        default_props: &'a SectionProperties,
    ) -> Vec<Section<'a>> {
        let mut result = vec![OpenSection::default()];
        for &index in &tree.roots {
            let paragraph = match &tree.nodes[index].data {
                Data::Paragraph { properties } => Some(properties.as_ref()),
                _ => None,
            };
            if paragraph.is_some_and(|paragraph| self.page_break_before(paragraph)) {
                close(&mut result, None, true);
            }
            if let Some(current) = result.last_mut() {
                current.elements.push(Slice::whole(index));
            }
            let Some(paragraph) = paragraph else { continue };
            let found = if self.options().break_pages {
                self.find_page_break(tree, index)
            } else {
                None
            };
            if paragraph.section_props.is_some() || found.is_some() {
                close(
                    &mut result,
                    paragraph.section_props.as_ref(),
                    found.is_some(),
                );
            }
            if let Some(found) = found {
                split_paragraph(tree, index, found, &mut result);
            }
        }
        let mut current = None;
        let mut sections: Vec<_> = result
            .into_iter()
            .rev()
            .map(|section| {
                current = section.sect_props.or(current);
                Section {
                    sect_props: current.unwrap_or(default_props),
                    elements: section.elements,
                    page_break: section.page_break,
                }
            })
            .collect();
        sections.reverse();
        sections
    }

    /// Whether a paragraph's style starts it on a new page.
    fn page_break_before(&self, paragraph: &ParagraphProperties) -> bool {
        self.find_style(paragraph.style_name.as_ref().and_then(Option::as_deref))
            .and_then(|style| style.paragraph_props.as_ref())
            .is_some_and(|props| props.page_break_before == Some(true))
    }

    /// Find the first page break in a paragraph's children: the child and its position.
    fn find_page_break(
        &self,
        tree: &OpenXmlElementTree,
        paragraph: usize,
    ) -> Option<(usize, usize)> {
        tree.nodes[paragraph]
            .base
            .children
            .iter()
            .enumerate()
            .find_map(|(child, &run)| {
                tree.nodes[run]
                    .base
                    .children
                    .iter()
                    .position(|&element| self.is_page_break_element(&tree.nodes[element]))
                    .map(|position| (child, position))
            })
    }

    /// Whether an element breaks the page: a page break, or a last-rendered break when kept.
    fn is_page_break_element(&self, element: &OpenXmlElement) -> bool {
        match &element.data {
            Data::Break { r#break } if r#break == "lastRenderedPageBreak" => {
                !self.options().ignore_last_rendered_page_break
            }
            Data::Break { r#break } => r#break == "page",
            _ => false,
        }
    }

    /// Group sections into pages, ending a page after a page break or a page size change.
    fn group_by_page_breaks(&self, sections: Vec<Section<'a>>) -> Vec<Vec<Section<'a>>> {
        let mut result = vec![Vec::new()];
        let mut prev = None;
        for section in sections {
            let ends_page = self.options().ignore_last_rendered_page_break
                || section.page_break
                || is_page_break_section(prev, section.sect_props);
            prev = Some(section.sect_props);
            if let Some(current) = result.last_mut() {
                current.push(section);
            }
            if ends_page {
                result.push(Vec::new());
            }
        }
        result.retain(|page| !page.is_empty());
        result
    }
}

/// Close the current section with its group and break, and open the next one.
fn close<'a>(
    result: &mut Vec<OpenSection<'a>>,
    props: Option<&'a SectionProperties>,
    page_break: bool,
) {
    if let Some(current) = result.last_mut() {
        current.sect_props = props;
        current.page_break = page_break;
    }
    result.push(OpenSection::default());
}

/// Split a paragraph at its page break: the part before the break stays in the closed section
/// and the rest leads the next one; a break inside a run splits the run the same way.
fn split_paragraph(
    tree: &OpenXmlElementTree,
    paragraph: usize,
    (child, position): (usize, usize),
    result: &mut [OpenSection<'_>],
) {
    let children = &tree.nodes[paragraph].base.children;
    let (before, after) = children.split_at(child);
    let Some(&run) = after.first() else { return };
    let run_children = &tree.nodes[run].base.children;
    let split_run = position + 1 < run_children.len();
    if after.len() == 1 && !split_run {
        return;
    }
    let mut head: Vec<_> = before.iter().map(|&child| Slice::whole(child)).collect();
    let mut tail: Vec<_> = after.iter().map(|&child| Slice::whole(child)).collect();
    if split_run {
        let (run_before, run_after) = run_children.split_at(position);
        head.push(Slice::kept(run, run_before));
        if let Some(first) = tail.first_mut() {
            *first = Slice::kept(run, run_after);
        }
    }
    if let [.., closed, current] = result
        && let Some(last) = closed.elements.last_mut()
    {
        *last = Slice {
            index: paragraph,
            kept: Some(head),
        };
        current.elements.push(Slice {
            index: paragraph,
            kept: Some(tail),
        });
    }
}

/// Whether consecutive section groups differ in orientation or page size.
fn is_page_break_section(prev: Option<&SectionProperties>, next: &SectionProperties) -> bool {
    prev.is_some_and(|prev| page_size(prev) != page_size(next))
}

/// A section group's orientation, width and height, each absent when not declared.
fn page_size(props: &SectionProperties) -> (Option<&str>, Option<&str>, Option<&str>) {
    props.page_size.as_ref().map_or((None, None, None), |size| {
        (
            size.orientation.as_deref(),
            size.width.as_deref(),
            size.height.as_deref(),
        )
    })
}

/// Start a section's content element with its column layout.
fn create_section_content(props: &SectionProperties) -> HtmlElement {
    let mut element = HtmlElement::new("article");
    if let Some(columns) = &props.columns
        && let Some(count) = columns
            .number_of_columns
            .filter(|count| *count != 0.0 && !count.is_nan())
    {
        element.set_style("column-count", Some(ryu_js::Buffer::new().format(count)));
        element.set_style("column-gap", columns.space.as_deref());
        if columns.separator == Some(true) {
            element.set_style("column-rule", Some("1px solid black"));
        }
    }
    element
}
