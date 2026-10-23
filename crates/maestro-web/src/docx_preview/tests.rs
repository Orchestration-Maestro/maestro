//! Page trees and preview renderings recorded for controlled Word packages.
mod alt_chunk;
mod composition;
mod corrections;
mod drawing;
mod elements;
mod entry_points;
mod fonts;
mod header_footer;
mod inline;
mod links;
mod math;
mod notes;
mod numbering;
mod pages;
mod stylesheet;
mod tables;
mod theme;
mod vml;

use super::html_element::{HtmlContent, HtmlElement, Namespace};
use super::{Options, parse_async};
use maestro_app::browser::strip_decimal;
use serde::Deserialize;
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::Path;

/// One recorded package rendering.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Fixture name, also the package file stem.
    name: String,
    /// Options passed beside the defaults.
    options: RecordedOptions,
    /// Recorded result kind.
    outcome: String,
    /// Recorded page trees.
    expected: Vec<Value>,
}

/// Recorded option overrides, by their recorded spelling.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RecordedOptions {
    /// Page-break detection override.
    break_pages: Option<bool>,
    /// Page-width override.
    ignore_width: Option<bool>,
    /// Page-height override.
    ignore_height: Option<bool>,
    /// Last-rendered page-break override.
    ignore_last_rendered_page_break: Option<bool>,
    /// Header rendering override.
    render_headers: Option<bool>,
    /// Footer rendering override.
    render_footers: Option<bool>,
    /// Footnote rendering override.
    render_footnotes: Option<bool>,
    /// Endnote rendering override.
    render_endnotes: Option<bool>,
}

impl RecordedOptions {
    /// Apply the recorded overrides to the default options.
    fn options(&self) -> Options {
        let mut options = Options::default();
        let pick = |value: Option<bool>, target: &mut bool| *target = value.unwrap_or(*target);
        pick(self.break_pages, &mut options.break_pages);
        pick(self.ignore_width, &mut options.page_size.ignore_width);
        pick(self.ignore_height, &mut options.page_size.ignore_height);
        pick(
            self.ignore_last_rendered_page_break,
            &mut options.ignore_last_rendered_page_break,
        );
        pick(
            self.render_headers,
            &mut options.headers_footers.render_headers,
        );
        pick(
            self.render_footers,
            &mut options.headers_footers.render_footers,
        );
        pick(self.render_footnotes, &mut options.notes.render_footnotes);
        pick(self.render_endnotes, &mut options.notes.render_endnotes);
        options
    }
}

/// Read one recorded case by fixture name.
fn case(name: &str) -> Case {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/docx/render-cases.json")).unwrap();
    let case = cases.into_iter().find(|case| case.name == name).unwrap();
    assert_eq!(case.outcome, "success");
    case
}

/// Read one controlled package by fixture name.
fn package(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/docx_preview/fixtures/docx")
        .join(format!("{name}.docx"));
    std::fs::read(path).unwrap()
}

/// Render one fixture with its recorded options.
fn render(name: &str) -> Vec<HtmlElement> {
    parse_async(&package(name), case(name).options.options())
        .unwrap()
        .render_sections()
        .pages
}

/// Write one element's declarations as the recording observed them: sorted, numbers in their
/// shortest form, `calc(...)` operands collapsed.
fn declarations(element: &HtmlElement) -> String {
    element
        .declarations()
        .map(|(property, value)| {
            let value = if value.starts_with("calc(") {
                "calc(...)".into()
            } else {
                shortest_numbers(value)
            };
            format!("{property}:{value}")
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Project an element the way the recording observed its pages.
///
/// The recording read declarations back from the browser, which writes numbers in their
/// shortest form, and then collapsed `calc(...)` operands and sorted attributes by name; the
/// projection does the same.
fn observe(element: &HtmlElement) -> Value {
    let mut attributes: Vec<(String, String)> = element.attributes().map(pair).collect();
    let declarations = declarations(element);
    if !declarations.is_empty() {
        attributes.push(("style".into(), declarations));
    }
    attributes.sort();
    let children: Vec<_> = element
        .children()
        .iter()
        .map(|child| match child {
            HtmlContent::Element(element) => observe(element),
            HtmlContent::Text(text) => json!({ "text": text }),
        })
        .collect();
    json!({ "tag": element.tag(), "attributes": attributes, "children": children })
}

/// Own both halves of an attribute pair.
fn pair((name, value): (&str, &str)) -> (String, String) {
    (name.to_owned(), value.to_owned())
}

/// Drop trailing fractional zeros from each decimal number in a declaration value.
fn shortest_numbers(value: &str) -> String {
    let mut out = String::new();
    let mut number = String::new();
    for character in value.chars().chain(['\0']) {
        if character.is_ascii_digit() || character == '.' {
            number.push(character);
            continue;
        }
        out.push_str(strip_decimal(&number));
        number.clear();
        if character != '\0' {
            out.push(character);
        }
    }
    out
}

/// Compare every rendered page with the recorded pages, as Maestro corrects them.
fn check(name: &str) {
    let mut expected = case(name).expected;
    corrections::correct(name, &mut expected);
    let observed: Vec<_> = render(name).iter().map(observe).collect();
    assert_eq!(observed, expected, "{name}");
}

/// One recorded whole-preview rendering: the mounted containers with every style the
/// rendering writes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviewCase {
    /// Fixture name, also the package file stem.
    name: String,
    /// Options the recording rendered with.
    options: PreviewOptions,
    /// Recorded result kind.
    outcome: String,
    /// Recorded body container, or the style container when the recording had a separate one.
    #[serde(default)]
    expected: Vec<Value>,
    /// Recorded style container, present only when the recording mounted styles apart.
    #[serde(default)]
    styles: Option<Vec<Value>>,
}

/// Options the preview recording rendered with, by their recorded spelling.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PreviewOptions {
    /// Class namespace override.
    class_name: Option<String>,
    /// XML declaration trimming override.
    trim_xml_declaration: Option<bool>,
    /// Page-break detection override.
    break_pages: Option<bool>,
    /// Page-width override.
    ignore_width: Option<bool>,
    /// Page wrapper override.
    in_wrapper: Option<bool>,
    /// Print rule override.
    hide_wrapper_on_print: Option<bool>,
    /// Font loading override.
    ignore_fonts: Option<bool>,
    /// Resource URL override.
    #[serde(rename = "useBase64URL")]
    use_base64_url: Option<bool>,
    /// Alternate chunk override.
    render_alt_chunks: Option<bool>,
}

impl PreviewOptions {
    /// Apply the recorded values to the default options.
    fn options(&self) -> Options {
        let mut options = Options::default();
        let pick = |value: Option<bool>, target: &mut bool| *target = value.unwrap_or(*target);
        if let Some(class_name) = &self.class_name {
            options.class_name.clone_from(class_name);
        }
        pick(self.trim_xml_declaration, &mut options.trim_xml_declaration);
        pick(self.break_pages, &mut options.break_pages);
        pick(self.ignore_width, &mut options.page_size.ignore_width);
        pick(self.in_wrapper, &mut options.wrapper.in_wrapper);
        pick(
            self.hide_wrapper_on_print,
            &mut options.wrapper.hide_wrapper_on_print,
        );
        pick(self.ignore_fonts, &mut options.resources.ignore_fonts);
        pick(self.use_base64_url, &mut options.resources.use_base64_url);
        pick(
            self.render_alt_chunks,
            &mut options.resources.render_alt_chunks,
        );
        options
    }
}

/// Read one recorded preview case by fixture name.
fn preview_case(name: &str) -> PreviewCase {
    let cases: Vec<PreviewCase> =
        serde_json::from_str(include_str!("fixtures/docx/preview-cases.json")).unwrap();
    cases
        .into_iter()
        .find(|case| case.name == name)
        .unwrap_or_else(|| panic!("no recorded preview case {name}"))
}

/// Render one fixture and fill its containers the way a mount does: styles, then the body,
/// then the resource styles.
fn render_preview(name: &str) -> (Vec<Value>, Vec<Value>) {
    let case = preview_case(name);
    assert_eq!(case.outcome, "success");
    let options = case.options.options();
    let preview = parse_async(&package(name), options.clone()).unwrap();
    let rendering = preview.render(&options);
    let styles: Vec<Value> = rendering.styles.iter().map(observe_preview).collect();
    let resources: Vec<Value> = rendering
        .resource_styles
        .iter()
        .map(observe_preview)
        .collect();
    let body: Vec<Value> = rendering.body.iter().map(observe_preview).collect();
    if case.styles.is_some() {
        let mut style_container = styles;
        style_container.extend(resources);
        return (body, style_container);
    }
    let mut container = styles;
    container.extend(body);
    container.extend(resources);
    (container, Vec::new())
}

/// Compare every container of a recorded preview rendering with the recording, as Maestro
/// corrects it.
fn check_preview(name: &str) {
    let case = preview_case(name);
    let separate = case.styles.is_some();
    let (mut expected, mut styles) = (case.expected, case.styles.unwrap_or_default());
    corrections::correct_preview(name, &mut expected, &mut styles);
    strip_measured_sizes(&mut expected);
    strip_measured_sizes(&mut styles);
    let (observed, observed_styles) = render_preview(name);
    assert_eq!(observed, expected, "{name} body");
    if separate {
        assert_eq!(observed_styles, styles, "{name} styles");
    }
}

/// Remove the shape sizes the browser measures one frame after mounting, which no native
/// rendering writes; the recording must hold them.
fn strip_measured_sizes(nodes: &mut [Value]) {
    for node in nodes {
        if node.get("tag").and_then(Value::as_str) == Some("svg") {
            let attributes = node
                .get_mut("attributes")
                .and_then(Value::as_array_mut)
                .expect("svg attributes");
            for name in ["width", "height"] {
                let index = attributes
                    .iter()
                    .position(|attribute| attribute[0] == name)
                    .unwrap_or_else(|| panic!("recorded svg without {name}"));
                attributes.remove(index);
            }
        }
        if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
            strip_measured_sizes(children);
        }
    }
}

/// Project one element the way the preview recording observed it: a style element by its
/// text, every other element by its namespace, sorted attributes and children.
fn observe_preview(element: &HtmlElement) -> Value {
    let namespace = element.namespace();
    if namespace == Namespace::Html && element.tag() == "style" {
        let css = match element.children() {
            [HtmlContent::Text(text)] => text.clone(),
            _ => String::new(),
        };
        return json!({ "tag": "style", "css": css });
    }
    let mut attributes: Vec<(String, String)> = element.attributes().map(pair).collect();
    if namespace != Namespace::Svg {
        let declarations = declarations(element);
        if !declarations.is_empty() {
            attributes.push(("style".into(), declarations));
        }
    }
    attributes.sort();
    let children: Vec<_> = element
        .children()
        .iter()
        .map(|child| match child {
            HtmlContent::Element(element) => observe_preview(element),
            HtmlContent::Text(text) => json!({ "text": text }),
        })
        .collect();
    let mut observed = json!({ "tag": element.tag() });
    match namespace {
        Namespace::Html => {}
        Namespace::Svg => observed["ns"] = json!("svg"),
        Namespace::MathMl => observed["ns"] = json!("math"),
    }
    observed["attributes"] = json!(attributes);
    observed["children"] = Value::Array(children);
    observed
}

/// Write an element as markup, for the assertions that read its serialized form.
fn markup(element: &HtmlElement) -> String {
    let mut out = format!("<{}", element.tag());
    for (name, value) in element.attributes() {
        let _ = write!(out, " {name}=\"{}\"", escaped(value));
    }
    let declarations: Vec<String> = element
        .declarations()
        .map(|(property, value)| format!("{property}:{value}"))
        .collect();
    if !declarations.is_empty() {
        let _ = write!(out, " style=\"{}\"", escaped(&declarations.join(";")));
    }
    out.push('>');
    for child in element.children() {
        match child {
            HtmlContent::Element(element) => out.push_str(&markup(element)),
            HtmlContent::Text(text) => out.push_str(&escaped(text)),
        }
    }
    if !VOID_ELEMENTS.contains(&element.tag()) {
        let _ = write!(out, "</{}>", element.tag());
    }
    out
}

/// Elements markup writes without a closing tag.
const VOID_ELEMENTS: [&str; 4] = ["br", "img", "meta", "link"];

/// Escape markup-significant characters once.
fn escaped(text: &str) -> String {
    let mut out = String::new();
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            character => out.push(character),
        }
    }
    out
}
