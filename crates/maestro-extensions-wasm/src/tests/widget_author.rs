//! The author's host widget scripts: each step builds, changes or renders a named widget the
//! author keeps, and reports its own outcome.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};

use maestro_extensions_wasm::{
    Box, CallbackEmitter, Component, ComponentView, Container, DefaultTextStyle, DynamicBorder,
    ExtensionResult, Highlight, Markdown, MarkdownTheme, Spacer, Style, Text, TextDecoration,
    get_markdown_theme,
};

/// A widget or authored component the author keeps under a name.
#[derive(Clone)]
enum Kept {
    /// Wrapped text.
    Text(Text),
    /// A padded box.
    Box(Box),
    /// A container.
    Container(Container),
    /// Blank rows.
    Spacer(Spacer),
    /// Markdown.
    Markdown(Markdown),
    /// A rule.
    Border(DynamicBorder),
    /// An authored component that renders kept widgets.
    View(Component),
}

impl Kept {
    /// The kept value as a component; a widget's view forwards to the host widget.
    fn component(&self) -> Component {
        match self {
            Self::Text(widget) => widget.into(),
            Self::Box(widget) => widget.into(),
            Self::Container(widget) => widget.into(),
            Self::Spacer(widget) => widget.into(),
            Self::Markdown(widget) => widget.into(),
            Self::Border(widget) => widget.into(),
            Self::View(component) => component.clone(),
        }
    }

    /// Renders a kept widget.
    fn render(&self, width: u32) -> ExtensionResult<Vec<String>> {
        match self {
            Self::Text(widget) => widget.render(width),
            Self::Box(widget) => widget.render(width),
            Self::Container(widget) => widget.render(width),
            Self::Spacer(widget) => widget.render(width),
            Self::Markdown(widget) => widget.render(width),
            Self::Border(widget) => widget.render(width),
            Self::View(_) => Err("an authored view renders through its host".to_owned()),
        }
    }

    /// Invalidates a kept widget.
    fn invalidate(&self) -> ExtensionResult<()> {
        match self {
            Self::Text(widget) => widget.invalidate(),
            Self::Box(widget) => widget.invalidate(),
            Self::Container(widget) => widget.invalidate(),
            Self::Spacer(widget) => widget.invalidate(),
            Self::Markdown(widget) => widget.invalidate(),
            Self::Border(widget) => widget.invalidate(),
            Self::View(_) => Err("an authored view invalidates through its host".to_owned()),
        }
    }
}

thread_local! {
    /// Kept widgets by name.
    static KEPT: RefCell<HashMap<String, Kept>> = RefCell::default();
    /// Authored view renders and invalidations, in order.
    static VIEWS: RefCell<Vec<String>> = RefCell::default();
}

/// An authored component that renders the kept widgets it names, synchronously.
struct View {
    /// Its name.
    name: String,
    /// The widgets it renders, in order.
    widgets: Vec<Kept>,
}

impl ComponentView for View {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        VIEWS.with_borrow_mut(|views| views.push(format!("{} render {width}", self.name)));
        let mut rows = vec![format!("view {}", self.name)];
        for widget in &self.widgets {
            rows.extend(widget.render(width)?);
        }
        Ok(rows)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        VIEWS.with_borrow_mut(|views| views.push(format!("{} invalidate", self.name)));
        self.widgets.iter().try_for_each(Kept::invalidate)
    }
}

/// A styling choice: `host:<role>` of the live Markdown theme, `fail:<label>` that fails, or
/// an authored label that wraps the text.
fn style(brush: &str) -> ExtensionResult<Style> {
    if let Some(role) = brush.strip_prefix("host:") {
        let theme = get_markdown_theme()?;
        return match role {
            "heading" => Ok(theme.heading),
            "bold" => Ok(theme.bold),
            _ => Err(format!("unknown role {role}")),
        };
    }
    if let Some(label) = brush.strip_prefix("fail:") {
        let label = label.to_owned();
        return Ok(Style::new(move |text| {
            Err(format!("{label} failed on {text}"))
        }));
    }
    let label = brush.to_owned();
    Ok(Style::new(move |text| Ok(format!("{label}({text})"))))
}

/// An optional styling choice.
fn styles(brush: Option<&str>) -> ExtensionResult<Option<Style>> {
    brush.map(style).transpose()
}

/// A Markdown theme: the host's, or one whose roles wrap text in their names.
fn theme(choice: &str) -> ExtensionResult<MarkdownTheme> {
    if choice == "host" {
        return get_markdown_theme();
    }
    let role = |name: &str| style(name);
    Ok(MarkdownTheme {
        heading: role("heading")?,
        link: role("link")?,
        link_url: role("linkUrl")?,
        code: role("code")?,
        code_block: role("codeBlock")?,
        code_block_border: role("codeBlockBorder")?,
        quote: role("quote")?,
        quote_border: role("quoteBorder")?,
        hr: role("hr")?,
        list_bullet: role("listBullet")?,
        bold: role("bold")?,
        italic: role("italic")?,
        strikethrough: role("strikethrough")?,
        underline: role("underline")?,
        highlight_code: Some(Highlight::new(|code, lang| {
            Ok(code
                .lines()
                .map(|line| format!("hl {}({line})", lang.unwrap_or_default()))
                .collect())
        })),
        code_block_indent: Some("> ".to_owned()),
    })
}

/// One script step.
#[derive(Deserialize)]
#[serde(tag = "do", rename_all = "camelCase", deny_unknown_fields)]
pub enum Step {
    /// Builds wrapped text.
    Text {
        /// Name to keep it under.
        name: String,
        /// Text.
        text: String,
        /// Horizontal and vertical padding.
        padding: (u32, u32),
        /// Row background.
        #[serde(default)]
        bg: Option<String>,
    },
    /// Builds a box.
    Box {
        /// Name to keep it under.
        name: String,
        /// Horizontal and vertical padding.
        padding: (u32, u32),
        /// Row background.
        #[serde(default)]
        bg: Option<String>,
    },
    /// Builds a container.
    Container {
        /// Name to keep it under.
        name: String,
    },
    /// Builds a spacer.
    Spacer {
        /// Name to keep it under.
        name: String,
        /// Rows.
        lines: u32,
    },
    /// Builds Markdown.
    Markdown {
        /// Name to keep it under.
        name: String,
        /// Text.
        text: String,
        /// Horizontal and vertical padding.
        padding: (u32, u32),
        /// `host` or `authored`.
        theme: String,
        /// Base foreground and background styles and decorations.
        #[serde(default)]
        style: Option<(Option<String>, Option<String>, Vec<String>)>,
    },
    /// Builds a rule.
    Border {
        /// Name to keep it under.
        name: String,
        /// Color.
        #[serde(default)]
        color: Option<String>,
    },
    /// Builds an authored component rendering kept widgets.
    View {
        /// Name to keep it under.
        name: String,
        /// Kept widgets it renders.
        widgets: Vec<String>,
    },
    /// Replaces the text of text or Markdown.
    SetText {
        /// Kept name.
        name: String,
        /// Text.
        text: String,
    },
    /// Replaces or removes the background of text or a box.
    SetBg {
        /// Kept name.
        name: String,
        /// Background.
        #[serde(default)]
        bg: Option<String>,
    },
    /// Replaces the rows of a spacer.
    SetLines {
        /// Kept name.
        name: String,
        /// Rows.
        lines: u32,
    },
    /// Appends a kept child to a container or box.
    Add {
        /// Kept name.
        name: String,
        /// Kept child name.
        child: String,
    },
    /// Removes a kept child from a container or box.
    Remove {
        /// Kept name.
        name: String,
        /// Kept child name.
        child: String,
    },
    /// Removes every child of a container or box.
    Clear {
        /// Kept name.
        name: String,
    },
    /// Renders a kept widget.
    Render {
        /// Kept name.
        name: String,
        /// Width.
        width: u32,
    },
    /// Invalidates a kept widget.
    Invalidate {
        /// Kept name.
        name: String,
    },
    /// Applies a host Markdown style of a role.
    Apply {
        /// Role.
        role: String,
        /// Text.
        text: String,
    },
    /// Reports and forgets the authored view renders and invalidations so far.
    Views,
    /// Forgets every kept widget.
    Forget,
}

/// The widget kept under `name`.
fn kept(name: &str) -> ExtensionResult<Kept> {
    KEPT.with_borrow(|kept| kept.get(name).cloned())
        .ok_or_else(|| format!("nothing kept as {name}"))
}

/// Keeps `widget` under `name`.
fn keep(name: String, widget: Kept) -> Value {
    KEPT.with_borrow_mut(|kept| kept.insert(name, widget));
    json!(true)
}

/// Builds and keeps one widget.
fn build(step: Step) -> ExtensionResult<Value> {
    Ok(match step {
        Step::Text {
            name,
            text,
            padding,
            bg,
        } => keep(
            name,
            Kept::Text(Text::new(
                &text,
                padding.0,
                padding.1,
                styles(bg.as_deref())?.as_ref(),
            )?),
        ),
        Step::Box { name, padding, bg } => keep(
            name,
            Kept::Box(Box::new(
                padding.0,
                padding.1,
                styles(bg.as_deref())?.as_ref(),
            )?),
        ),
        Step::Container { name } => keep(name, Kept::Container(Container::new()?)),
        Step::Spacer { name, lines } => keep(name, Kept::Spacer(Spacer::new(lines)?)),
        Step::Markdown {
            name,
            text,
            padding,
            theme: choice,
            style,
        } => {
            let style = style.map(base).transpose()?;
            let markdown = Markdown::new(
                &text,
                padding.0,
                padding.1,
                &theme(&choice)?,
                style.as_ref(),
            )?;
            keep(name, Kept::Markdown(markdown))
        }
        Step::Border { name, color } => keep(
            name,
            Kept::Border(DynamicBorder::new(styles(color.as_deref())?.as_ref())?),
        ),
        Step::View { name, widgets } => {
            let widgets = widgets
                .iter()
                .map(|name| kept(name))
                .collect::<ExtensionResult<_>>()?;
            let view = View {
                name: name.clone(),
                widgets,
            };
            keep(name, Kept::View(Component::new(view)))
        }
        step => change(step)?,
    })
}

/// A base text style from its foreground, background and decoration names.
fn base(
    (color, bg, decorations): (Option<String>, Option<String>, Vec<String>),
) -> ExtensionResult<DefaultTextStyle> {
    let decorations = decorations
        .iter()
        .map(|name| match name.as_str() {
            "bold" => Ok(TextDecoration::Bold),
            "italic" => Ok(TextDecoration::Italic),
            "strikethrough" => Ok(TextDecoration::Strikethrough),
            "underline" => Ok(TextDecoration::Underline),
            _ => Err(format!("unknown decoration {name}")),
        })
        .collect::<ExtensionResult<_>>()?;
    Ok(DefaultTextStyle {
        color: styles(color.as_deref())?,
        bg_color: styles(bg.as_deref())?,
        decorations,
    })
}

/// Changes or renders one kept widget.
fn change(step: Step) -> ExtensionResult<Value> {
    match step {
        Step::SetText { name, text } => match kept(&name)? {
            Kept::Text(widget) => widget.set_text(&text)?,
            Kept::Markdown(widget) => widget.set_text(&text)?,
            _ => return Err(format!("{name} has no text")),
        },
        Step::SetBg { name, bg } => match kept(&name)? {
            Kept::Text(widget) => widget.set_custom_bg_fn(styles(bg.as_deref())?.as_ref())?,
            Kept::Box(widget) => widget.set_bg_fn(styles(bg.as_deref())?.as_ref())?,
            _ => return Err(format!("{name} has no background")),
        },
        Step::SetLines { name, lines } => match kept(&name)? {
            Kept::Spacer(widget) => widget.set_lines(lines)?,
            _ => return Err(format!("{name} has no rows")),
        },
        step => return observe(step),
    }
    Ok(json!(true))
}

/// Renders, invalidates or styles through kept widgets, or reports the view calls so far.
fn observe(step: Step) -> ExtensionResult<Value> {
    match step {
        Step::Render { name, width } => Ok(json!(kept(&name)?.render(width)?)),
        Step::Invalidate { name } => kept(&name)?.invalidate().map(|()| json!(true)),
        Step::Apply { role, text } => Ok(json!(style(&format!("host:{role}"))?.apply(&text)?)),
        Step::Views => Ok(json!(VIEWS.take())),
        Step::Forget => {
            drop(KEPT.take());
            Ok(json!(true))
        }
        step => children(step).map(|()| json!(true)),
    }
}

/// Adds, removes or clears the children of a kept container or box.
fn children(step: Step) -> ExtensionResult<()> {
    let (name, child) = match &step {
        Step::Add { name, child } | Step::Remove { name, child } => {
            (name.clone(), Some(kept(child)?.component()))
        }
        Step::Clear { name } => (name.clone(), None),
        _ => return Err("expected a widget step".to_owned()),
    };
    let parent = kept(&name)?;
    match (step, parent, child) {
        (Step::Add { .. }, Kept::Container(parent), Some(child)) => parent.add_child(&child),
        (Step::Add { .. }, Kept::Box(parent), Some(child)) => parent.add_child(&child),
        (Step::Remove { .. }, Kept::Container(parent), Some(child)) => parent.remove_child(&child),
        (Step::Remove { .. }, Kept::Box(parent), Some(child)) => parent.remove_child(&child),
        (Step::Clear { .. }, Kept::Container(parent), None) => parent.clear(),
        (Step::Clear { .. }, Kept::Box(parent), None) => parent.clear(),
        _ => Err(format!("{name} has no children")),
    }
}

/// Runs each step, reporting each outcome in order; a failed step does not stop the script.
#[must_use]
pub fn run(steps: Vec<Step>) -> Value {
    let outcomes: Vec<_> = steps
        .into_iter()
        .map(|step| match build(step) {
            Ok(value) => json!({ "ok": value }),
            Err(error) => json!({ "error": error }),
        })
        .collect();
    json!(outcomes)
}
