//! Host widgets both test hosts keep: their literal requests, and the rows and nested calls
//! a rendering makes.
//!
//! A host style paints `<role>text`; host highlighting prefixes each row with `<hl lang>`. A
//! text row is `text <text> <padding>|<width>`, a box first draws `box <padding>|<width>`, a
//! Markdown row is its heading style over `md <text>|<width>`, a rule is `─` across the width.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::collections::HashMap;

use serde_json::{Value, json};

use super::observed::Observed;

/// A style the host applies: one of its own by role, or an authored one by identity.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    /// A host style of this role.
    Host(String),
    /// The authored style under this identity.
    Authored(u32),
}

impl Paint {
    /// How the transcript names the style.
    fn label(&self) -> Value {
        match self {
            Self::Host(role) => json!(role),
            Self::Authored(id) => json!(format!("authored {id}")),
        }
    }
}

/// A child of a container or box.
#[derive(Clone, Debug, PartialEq)]
pub enum Child {
    /// A host widget by key.
    Widget(u32),
    /// An authored component by identity.
    Component(u32),
}

/// What one widget holds.
#[derive(Default)]
pub struct Fake {
    /// `text`, `box`, `container`, `spacer`, `markdown` or `border`.
    kind: String,
    /// Text of a text or Markdown widget.
    text: String,
    /// Padding of a text, box or Markdown widget.
    padding: (u32, u32),
    /// Rows of a spacer.
    lines: u32,
    /// Background of a text or box, heading style of Markdown, color of a rule.
    paint: Option<Paint>,
    /// Markdown's code highlighting.
    highlight: Option<Paint>,
    /// Children of a container or box.
    children: Vec<Child>,
}

/// One piece of a rendering.
#[derive(Debug)]
pub enum Part {
    /// A finished row.
    Row(String),
    /// A row the authored style under this identity paints.
    Style(u32, String),
    /// The rows of the authored component under this identity.
    Component(u32),
    /// The rows the authored highlighting under this identity makes of the sample code.
    Highlight(u32),
    /// The text an authored widget callback under this identity answers.
    Widget(u32, super::widget_lists::Event),
}

/// Code an authored highlighting receives, with its language.
pub const CODE: (&str, &str) = ("fn main() {}\nlet x = 1;", "rust");

/// Host widgets by key.
#[derive(Default)]
pub struct Bank {
    /// Widgets by key.
    widgets: HashMap<u32, Fake>,
    /// Last key handed out.
    next: u32,
}

/// The fourteen Markdown roles in field order.
pub const ROLES: [&str; 14] = [
    "heading",
    "link",
    "linkUrl",
    "code",
    "codeBlock",
    "codeBlockBorder",
    "quote",
    "quoteBorder",
    "hr",
    "listBullet",
    "bold",
    "italic",
    "strikethrough",
    "underline",
];

/// What a new widget is built from.
#[derive(Default)]
pub struct Build {
    /// Widget kind.
    pub kind: &'static str,
    /// Text.
    pub text: String,
    /// Padding.
    pub padding: (u32, u32),
    /// Rows of a spacer.
    pub lines: u32,
    /// Background, heading style or rule color.
    pub paint: Option<Paint>,
    /// Markdown's code highlighting.
    pub highlight: Option<Paint>,
    /// Literal remaining operands, recorded as they are.
    pub operands: Value,
}

/// Records and keeps a new widget, returning its key.
pub fn build(observed: &mut Observed, build: Build) -> u32 {
    observed.ui.hosted.next += 1;
    let key = observed.ui.hosted.next;
    let paint = build.paint.as_ref().map(Paint::label);
    observed.log(format!(
        "widget {key} new {} {}",
        build.kind,
        json!([
            build.text,
            build.padding,
            build.lines,
            paint,
            build.operands
        ])
    ));
    observed.ui.hosted.widgets.insert(
        key,
        Fake {
            kind: build.kind.to_owned(),
            text: build.text,
            padding: build.padding,
            lines: build.lines,
            paint: build.paint,
            highlight: build.highlight,
            children: Vec::new(),
        },
    );
    key
}

/// The widget under `key`.
fn widget(observed: &mut Observed, key: u32) -> Result<&mut Fake, String> {
    observed
        .ui
        .hosted
        .widgets
        .get_mut(&key)
        .ok_or_else(|| format!("no widget {key}"))
}

/// One mutation of the widget under `key`.
pub enum Change {
    /// Replaces the text.
    Text(String),
    /// Replaces or removes the background.
    Background(Option<Paint>),
    /// Replaces the row count.
    Lines(u32),
    /// Appends a child.
    Add(Child),
    /// Removes the first occurrence of a child.
    Remove(Child),
    /// Removes every child.
    Clear,
}

/// Records and applies one mutation.
///
/// # Errors
/// Returns a message when the widget is unknown.
pub fn change(observed: &mut Observed, key: u32, change: Change) -> Result<(), String> {
    let line = match &change {
        Change::Text(text) => format!("setText {}", json!(text)),
        Change::Background(paint) => format!(
            "setBackground {}",
            paint.as_ref().map(Paint::label).unwrap_or_default()
        ),
        Change::Lines(lines) => format!("setLines {lines}"),
        Change::Add(child) => format!("addChild {child:?}"),
        Change::Remove(child) => format!("removeChild {child:?}"),
        Change::Clear => "clear".to_owned(),
    };
    observed.log(format!("widget {key} {line}"));
    let widget = widget(observed, key)?;
    match change {
        Change::Text(text) => widget.text = text,
        Change::Background(paint) => widget.paint = paint,
        Change::Lines(lines) => widget.lines = lines,
        Change::Add(child) => widget.children.push(child),
        Change::Remove(child) => {
            if let Some(index) = widget.children.iter().position(|added| *added == child) {
                widget.children.remove(index);
            }
        }
        Change::Clear => widget.children.clear(),
    }
    Ok(())
}

/// A row painted by `paint`, or left as it is.
fn painted(paint: Option<&Paint>, row: String) -> Part {
    match paint {
        None => Part::Row(row),
        Some(Paint::Host(role)) => Part::Row(format!("<{role}>{row}")),
        Some(Paint::Authored(id)) => Part::Style(*id, row),
    }
}

/// The pieces of rendering the widget under `key` at `width`, recorded as one rendering.
///
/// # Errors
/// Returns a message when a widget is unknown.
pub fn render(observed: &mut Observed, key: u32, width: u32) -> Result<Vec<Part>, String> {
    observed.log(format!("widget {key} render {width}"));
    let mut parts = Vec::new();
    plan(observed, key, width, &mut parts)?;
    Ok(parts)
}

/// Appends the pieces of rendering the widget under `key`.
fn plan(
    observed: &mut Observed,
    key: u32,
    width: u32,
    parts: &mut Vec<Part>,
) -> Result<(), String> {
    let special = super::widget_lists::render(observed, key, width)
        .or_else(|| super::widget_editors::render(observed, key, width));
    if let Some(special) = special {
        parts.extend(special);
        return Ok(());
    }
    let widget = widget(observed, key)?;
    let (px, py) = widget.padding;
    let paint = widget.paint.clone();
    let children = widget.children.clone();
    match widget.kind.as_str() {
        "text" => parts.push(painted(
            paint.as_ref(),
            format!("text {} {px} {py}|{width}", widget.text),
        )),
        "spacer" => parts.extend((0..widget.lines).map(|_| Part::Row(String::new()))),
        "border" => {
            let color = paint.unwrap_or_else(|| Paint::Host("border".to_owned()));
            let width = usize::try_from(width).map_err(|error| error.to_string())?;
            parts.push(painted(Some(&color), "─".repeat(width)));
        }
        "markdown" => {
            parts.push(painted(
                paint.as_ref(),
                format!("md {}|{width}", widget.text),
            ));
            match widget.highlight.clone() {
                Some(Paint::Authored(id)) => parts.push(Part::Highlight(id)),
                Some(Paint::Host(_)) => parts.extend(
                    CODE.0
                        .lines()
                        .map(|line| Part::Row(format!("<hl {}>{line}", CODE.1))),
                ),
                None => {}
            }
        }
        kind => {
            if kind == "box" {
                parts.push(painted(paint.as_ref(), format!("box {px} {py}|{width}")));
            }
            for child in children {
                match child {
                    Child::Widget(key) => plan(observed, key, width, parts)?,
                    Child::Component(id) => parts.push(Part::Component(id)),
                }
            }
        }
    }
    Ok(())
}

/// The authored components an invalidation of the widget under `key` reaches, in order,
/// recorded as one invalidation.
///
/// # Errors
/// Returns a message when a widget is unknown.
pub fn invalidate(observed: &mut Observed, key: u32) -> Result<Vec<u32>, String> {
    observed.log(format!("widget {key} invalidate"));
    let mut components = Vec::new();
    reach(observed, key, &mut components)?;
    Ok(components)
}

/// Appends the authored components below the widget under `key`.
fn reach(observed: &mut Observed, key: u32, components: &mut Vec<u32>) -> Result<(), String> {
    for child in widget(observed, key)?.children.clone() {
        match child {
            Child::Widget(key) => reach(observed, key, components)?,
            Child::Component(id) => components.push(id),
        }
    }
    Ok(())
}

/// Records one host style application and paints the text.
pub fn apply(observed: &mut Observed, role: &str, text: &str) -> String {
    observed.log(format!("style {role} {}", json!(text)));
    format!("<{role}>{text}")
}

/// Records one host highlighting and highlights the code.
pub fn highlight(observed: &mut Observed, code: &str, lang: Option<&str>) -> Vec<String> {
    observed.log(format!("highlight {}", json!([code, lang])));
    let lang = lang.unwrap_or_default();
    code.lines()
        .map(|line| format!("<hl {lang}>{line}"))
        .collect()
}
