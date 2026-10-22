//! Paragraphs, runs, links and the inline content they hold.
use super::{Children, HtmlRenderer, render_style_values};
use crate::docx_preview::html_element::HtmlElement;
use maestro_app::browser::docx_preview::{
    CommonProperties, OpenXmlElementTree, ParagraphProperties, Relationship, RunProperties,
    VerticalAlign, WmlHyperlink,
};
use maestro_app::browser::executable;

impl<'a> HtmlRenderer<'a> {
    /// Render a paragraph with its class, declarations, common properties and numbering class.
    pub(super) fn render_paragraph(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        properties: &'a ParagraphProperties,
        children: Children<'_>,
    ) -> HtmlElement {
        let mut element = HtmlElement::new("p");
        element.append(self.render_children(tree, index, children));
        let style_name = properties.style_name.as_ref().and_then(Option::as_deref);
        let style = self
            .find_style(style_name)
            .and_then(|style| style.paragraph_props.as_ref());
        self.render_class(properties.class_name.as_deref(), style_name, &mut element);
        render_style_values(properties.css_style.as_ref(), &mut element);
        render_common_properties(&properties.common, &mut element);
        if let Some(numbering) = properties
            .numbering
            .as_ref()
            .or_else(|| style.and_then(|style| style.numbering.as_ref()))
        {
            element.add_class(&self.numbering_class(numbering));
        }
        element
    }

    /// Render a run as a span, wrapping its content when it is raised or lowered.
    pub(super) fn render_run(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        properties: &'a RunProperties,
        children: Children<'_>,
    ) -> HtmlElement {
        let mut element = HtmlElement::new("span");
        let style_name = properties.style_name.as_ref().and_then(Option::as_deref);
        self.render_class(None, style_name, &mut element);
        render_style_values(properties.css_style.as_ref(), &mut element);
        let content = self.render_children(tree, index, children);
        match properties.vertical_align.as_ref().and_then(Option::as_ref) {
            Some(alignment) => {
                let mut wrapper = HtmlElement::new(match alignment {
                    VerticalAlign::Sub => "sub",
                    VerticalAlign::Sup => "sup",
                });
                wrapper.append(content);
                element.push_element(wrapper);
            }
            None => element.append(content),
        }
        element
    }

    /// Render a link around its runs; a refused target leaves the link without an `href`.
    pub(super) fn render_hyperlink(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        link: &WmlHyperlink,
    ) -> HtmlElement {
        let mut element = self.render_container(tree, index, "a");
        let relationships = self
            .preview
            .document
            .document_part()
            .and_then(|part| part.rels.as_deref());
        if let Some(href) = hyperlink_href(link, relationships) {
            element.set_attribute("href", &href);
        }
        element
    }
}

/// Produce a link's target, or nothing when a browser would run it as script or an active
/// document: the first external relationship of the link's identifier, then its anchor.
///
/// A matched relationship without a target gives an empty target, and the composed value
/// keeps its authored spelling.
pub(crate) fn hyperlink_href(
    link: &WmlHyperlink,
    relationships: Option<&[Relationship]>,
) -> Option<String> {
    let mut href = String::new();
    if let Some(id) = link.id.as_deref().filter(|id| !id.is_empty())
        && let Some(target) = relationships
            .unwrap_or_default()
            .iter()
            .find(|rel| {
                rel.id.as_deref() == Some(id) && rel.target_mode.as_deref() == Some("External")
            })
            .and_then(|rel| rel.target.as_deref())
    {
        href.push_str(target);
    }
    if let Some(anchor) = link.anchor.as_deref().filter(|anchor| !anchor.is_empty()) {
        href.push('#');
        href.push_str(anchor);
    }
    (!executable(&href)).then_some(href)
}

/// Apply a nonempty color and font size after the converted declarations.
fn render_common_properties(props: &CommonProperties, output: &mut HtmlElement) {
    for (property, value) in [("color", &props.color), ("font-size", &props.font_size)] {
        if let Some(value) = value
            .as_ref()
            .and_then(Option::as_deref)
            .filter(|v| !v.is_empty())
        {
            output.set_style(property, Some(value));
        }
    }
}

/// Render a text-wrapping break; page and column breaks render nothing.
pub(super) fn render_break(kind: &str) -> Option<HtmlElement> {
    (kind == "textWrapping").then(|| HtmlElement::new("br"))
}

/// Render a tab as an em space.
pub(super) fn render_tab() -> HtmlElement {
    let mut element = HtmlElement::new("span");
    element.push_text("\u{2003}");
    element
}

/// Render a bookmark as an empty span carrying its name, spelling a missing name `null`.
pub(super) fn render_bookmark_start(name: Option<&str>) -> HtmlElement {
    let mut element = HtmlElement::new("span");
    element.set_attribute("id", name.unwrap_or("null"));
    element
}

/// Render a symbol character in its font.
pub(super) fn render_symbol(font: Option<&str>, char: Option<&str>) -> HtmlElement {
    let mut element = HtmlElement::new("span");
    element.set_style("font-family", font);
    element.push_text(&symbol_text(char.unwrap_or("null")));
    element
}

/// Read a symbol's character as the numeric reference `&#x{char};` reads: its leading
/// hexadecimal digits name one character, and anything after them stays literal text.
fn symbol_text(char: &str) -> String {
    let end = char.bytes().take_while(u8::is_ascii_hexdigit).count();
    if end == 0 {
        return format!("&#x{char};");
    }
    let (digits, suffix) = char.split_at(end);
    let character = u32::from_str_radix(digits, 16)
        .ok()
        .and_then(char::from_u32)
        .filter(|character| *character != '\0')
        .unwrap_or('\u{fffd}');
    let suffix = suffix.strip_prefix(';').unwrap_or(suffix);
    format!(
        "{character}{suffix}{}",
        if end < char.len() { ";" } else { "" }
    )
}
