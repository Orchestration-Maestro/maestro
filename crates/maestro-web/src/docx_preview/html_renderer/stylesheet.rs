//! Style elements: the library defaults, the page wrapper, theme variables, document styles and
//! font faces.
use super::styles::Styles;
use super::{HtmlRenderer, process_style_name};
use crate::docx_preview::PartRecords;
use crate::docx_preview::html_element::{HtmlElement, contained};
use maestro_app::browser::docx_preview::font_table::{EmbedFontType, FontDeclaration};
use maestro_app::browser::docx_preview::{DmlTheme, enclose_font_family};
use maestro_app::normalize_to_lf;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Write one rule: its selectors, each declaration whose key does not start with `$` (a
/// missing value written `null`), then a literal suffix.
pub(super) fn style_to_string<'v>(
    selectors: &str,
    values: impl IntoIterator<Item = (&'v str, Option<&'v str>)>,
    css_text: Option<&str>,
) -> String {
    let mut result = format!("{selectors} {{\r\n");
    for (key, value) in values {
        if !key.starts_with('$') {
            let _ = write!(result, "  {key}: {};\r\n", value.unwrap_or("null"));
        }
    }
    result.push_str(css_text.unwrap_or_default());
    result.push_str("}\r\n");
    result
}

/// Keep only the declarations whose values stay inside their own declaration.
pub(super) fn contained_values<'v>(
    values: impl IntoIterator<Item = (&'v str, Option<&'v str>)>,
) -> impl Iterator<Item = (&'v str, Option<&'v str>)> {
    values
        .into_iter()
        .filter(|(_, value)| value.is_none_or(contained))
}

/// Whether a class, counter or variable token is a plain identifier: ASCII letters, digits,
/// `-`, `_` and non-ASCII characters only.
pub(super) fn identifier(token: &str) -> bool {
    token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_') || !c.is_ascii())
}

/// Create a style element holding CSS text, its line breaks as a parsed style element keeps
/// them.
pub(super) fn create_style_element(css_text: &str) -> HtmlElement {
    let mut element = HtmlElement::new("style");
    element.push_text(normalize_to_lf(css_text));
    element
}

/// Render the library rules under the class namespace, the two wrapper rules inside
/// `@media not print` when they are hidden on print.
pub(super) fn render_default_style(class_name: &str, hide_wrapper_on_print: bool) -> HtmlElement {
    let c = class_name;
    let mut wrapper = format!(
        "
.{c}-wrapper {{ background: gray; padding: 30px; padding-bottom: 0px; display: flex; flex-flow: column; align-items: center; }}\x20
.{c}-wrapper>section.{c} {{ background: white; box-shadow: 0 0 10px rgba(0, 0, 0, 0.5); margin-bottom: 30px; }}"
    );
    if hide_wrapper_on_print {
        wrapper = format!("@media not print {{ {wrapper} }}");
    }
    create_style_element(&format!(
        "{wrapper}
.{c} {{ color: black; hyphens: auto; text-underline-position: from-font; }}
section.{c} {{ box-sizing: border-box; display: flex; flex-flow: column nowrap; position: relative; overflow: hidden; }}
section.{c}>article {{ margin-bottom: auto; z-index: 1; }}
section.{c}>footer {{ z-index: 1; }}
.{c} table {{ border-collapse: collapse; }}
.{c} table td, .{c} table th {{ vertical-align: top; }}
.{c} p {{ margin: 0pt; min-height: 1em; }}
.{c} span {{ white-space: pre-wrap; overflow-wrap: break-word; }}
.{c} a {{ color: inherit; text-decoration: inherit; }}
.{c} svg {{ fill: transparent; }}
"
    ))
}

/// Enclose the pages in the page frame.
pub(super) fn render_wrapper(class_name: &str, pages: Vec<HtmlElement>) -> HtmlElement {
    let mut wrapper = HtmlElement::new("div");
    wrapper.set_attribute("class", format!("{class_name}-wrapper"));
    wrapper.extend(pages);
    wrapper
}

/// Declare the theme's heading and body fonts, then each color slot in source order, as
/// document variables; a missing font child is `undefined`, a missing typeface or color
/// value `null`.
pub(super) fn render_theme(theme: &DmlTheme, class_name: &str) -> HtmlElement {
    let mut variables = Vec::new();
    if let Some(fonts) = &theme.font_scheme {
        for (name, font) in [
            ("--docx-majorHAnsi-font", &fonts.major_font),
            ("--docx-minorHAnsi-font", &fonts.minor_font),
        ] {
            if let Some(font) = font {
                let typeface = match &font.latin_typeface {
                    None => "undefined",
                    Some(typeface) => typeface.as_deref().unwrap_or("null"),
                };
                variables.push((name.to_owned(), typeface.to_owned()));
            }
        }
    }
    for (slot, value) in theme.color_scheme.iter().flat_map(|scheme| &scheme.colors) {
        let value = value.as_deref().unwrap_or("null");
        variables.push((format!("--docx-{slot}-color"), format!("#{value}")));
    }
    let values = variables
        .iter()
        .map(|(key, value)| (key.as_str(), Some(value.as_str())));
    create_style_element(&style_to_string(
        &format!(".{class_name}"),
        contained_values(values),
        None,
    ))
}

impl Styles {
    /// Write every definition in document order: its own resolved substyles, then its linked
    /// style's. The last default of each target also matches its target anywhere in the
    /// document; a rule whose class is not a plain identifier is skipped.
    pub(super) fn render_styles(&self, class_name: &str) -> HtmlElement {
        let defaults: HashMap<Option<&str>, usize> = self
            .styles
            .iter()
            .enumerate()
            .filter(|(_, style)| style.is_default)
            .map(|(index, style)| (style.target.as_deref(), index))
            .collect();
        let mut css = String::new();
        for (index, style) in self.styles.iter().enumerate() {
            let css_name = process_style_name(class_name, style.id.as_deref());
            if !identifier(&css_name) {
                continue;
            }
            let target = style.target.as_deref();
            let linked = self.named(style.linked.as_deref());
            let prefixed = defaults.get(&target) == Some(&index);
            for substyle in style
                .substyles
                .iter()
                .chain(linked.into_iter().flat_map(|linked| &linked.substyles))
            {
                let selector =
                    rule_selector(target, &substyle.target, &css_name, class_name, prefixed);
                let values = substyle
                    .values
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_deref()));
                css.push_str(&style_to_string(&selector, contained_values(values), None));
            }
        }
        create_style_element(&css)
    }
}

impl HtmlRenderer<'_> {
    /// Add one font face per embedded face in table order, its family enclosed and its weight
    /// and style set by face kind; a face whose bytes do not load adds nothing.
    pub(super) fn render_font_table(&mut self, fonts: &PartRecords<FontDeclaration>) {
        for font in &fonts.records {
            let family = enclose_font_family(font.name.as_deref().unwrap_or("null"));
            let faces: Vec<(String, EmbedFontType)> = font
                .embed_font_refs
                .iter()
                .filter_map(|embedded| {
                    let url =
                        self.load_font(fonts, embedded.id.as_deref(), embedded.key.as_deref())?;
                    Some((url, embedded.r#type))
                })
                .collect();
            for (url, kind) in faces {
                let css = font_face_css(&family, &url, kind);
                self.resource_styles.push(create_style_element(&css));
            }
        }
    }
}

/// The selector of one rule: the style's class on its target, the substyle's own target when
/// they differ, and the whole-document form when the style is its target's last default.
fn rule_selector(
    target: Option<&str>,
    substyle_target: &str,
    css_name: &str,
    class_name: &str,
    prefixed: bool,
) -> String {
    let mut selector = format!("{}.{css_name}", target.unwrap_or_default());
    if target != Some(substyle_target) {
        selector.push(' ');
        selector.push_str(substyle_target);
    }
    if prefixed {
        let target = target.unwrap_or("null");
        selector = format!(".{class_name} {target}, {selector}");
    }
    selector
}

/// One font face rule: its family when enclosed, its source, and its weight and style by face
/// kind.
fn font_face_css(family: &str, url: &str, kind: EmbedFontType) -> String {
    let src = format!("url({url})");
    let mut values = vec![("src", Some(src.as_str()))];
    if contained(family) {
        values.insert(0, ("font-family", Some(family)));
    }
    if matches!(kind, EmbedFontType::Bold | EmbedFontType::BoldItalic) {
        values.push(("font-weight", Some("bold")));
    }
    if matches!(kind, EmbedFontType::Italic | EmbedFontType::BoldItalic) {
        values.push(("font-style", Some("italic")));
    }
    style_to_string("@font-face", values, None)
}
