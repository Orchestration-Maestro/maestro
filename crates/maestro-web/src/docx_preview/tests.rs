//! Page trees recorded for controlled Word packages.
mod corrections;
mod elements;
mod header_footer;
mod inline;
mod links;
mod notes;
mod pages;
mod tables;

use super::html_element::{HtmlContent, HtmlElement};
use super::{Options, WordPreview};
use maestro_app::browser::strip_decimal;
use serde::Deserialize;
use serde_json::{Value, json};
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
    WordPreview::load(&package(name), case(name).options.options())
        .unwrap()
        .render_sections()
}

/// Project an element the way the recording observed its pages.
///
/// The recording read declarations back from the browser, which writes numbers in their
/// shortest form, and then collapsed `calc(...)` operands and sorted attributes by name; the
/// projection does the same. Declarations are already sorted.
fn observe(element: &HtmlElement) -> Value {
    let mut attributes: Vec<(String, String)> = element.attributes.clone().into_iter().collect();
    if !element.style.is_empty() {
        let declarations: Vec<_> = element
            .style
            .iter()
            .map(|(property, value)| {
                let value = if value.starts_with("calc(") {
                    "calc(...)".into()
                } else {
                    shortest_numbers(value)
                };
                format!("{property}:{value}")
            })
            .collect();
        attributes.push(("style".into(), declarations.join(";")));
    }
    attributes.sort();
    let children: Vec<_> = element
        .children
        .iter()
        .map(|child| match child {
            HtmlContent::Element(element) => observe(element),
            HtmlContent::Text(text) => json!({ "text": text }),
        })
        .collect();
    json!({ "tag": element.tag, "attributes": attributes, "children": children })
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
