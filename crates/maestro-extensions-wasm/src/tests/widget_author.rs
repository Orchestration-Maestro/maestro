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
    BorderedLoader, BorderedLoaderOptions, Box, CallbackEmitter, Component, ComponentFactory,
    ComponentView, Container, DefaultTextStyle, DynamicBorder, ExtensionComponent, ExtensionResult,
    ExtensionUIContext, Highlight, Markdown, MarkdownTheme, RowStyle, SelectItem, SelectList,
    SelectListLayoutOptions, SelectListTheme, SettingItem, SettingsList, SettingsListOptions,
    SettingsListTheme, Spacer, Style, SubmenuDone, Text, TextDecoration, get_markdown_theme,
    get_select_list_theme, get_settings_list_theme,
};
use maestro_extensions_wasm::{
    CustomEditor, Editor, EditorOptions, EditorTheme, KeybindingDefinition, KeybindingKeys,
    KeybindingsConfig, KeybindingsManager, get_editor_theme, get_keybindings, key_hint, key_text,
    raw_key_hint,
};
use std::rc::Rc;

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
    /// A selection list.
    Select(SelectList),
    /// A settings list.
    Settings(SettingsList),
    /// A bordered loader.
    Loader(BorderedLoader),
    /// A keybindings manager.
    Keys(KeybindingsManager),
    /// The host editor.
    Editor(Editor),
    /// The application's custom editor.
    Custom(CustomEditor),
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
            Self::Select(widget) => widget.into(),
            Self::Settings(widget) => widget.into(),
            Self::Loader(widget) => widget.into(),
            Self::Keys(_) => Component::new(View {
                name: "keys".to_owned(),
                widgets: Vec::new(),
            }),
            Self::Editor(editor) => editor.into(),
            Self::Custom(editor) => editor.into(),
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
            Self::Select(widget) => widget.render(width),
            Self::Settings(widget) => widget.render(width),
            Self::Loader(widget) => widget.render(width),
            Self::Keys(_) => Err("a keybindings manager renders nothing".to_owned()),
            Self::Editor(editor) => editor.render(width),
            Self::Custom(editor) => editor.render(width),
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
            Self::Select(widget) => widget.invalidate(),
            Self::Settings(widget) => widget.invalidate(),
            Self::Loader(widget) => widget.invalidate(),
            Self::Keys(_) => Ok(()),
            Self::Editor(editor) => editor.invalidate(),
            Self::Custom(editor) => editor.invalidate(),
        }
    }
}

thread_local! {
    /// Kept widgets by name.
    static KEPT: RefCell<HashMap<String, Kept>> = RefCell::default();
    /// Authored view renders and invalidations, in order.
    static VIEWS: RefCell<Vec<String>> = RefCell::default();
    /// The UI a script may show widgets through.
    static UI: RefCell<Option<ExtensionUIContext>> = RefCell::default();
}

/// Records one authored callback call.
fn record(line: String) {
    VIEWS.with_borrow_mut(|views| views.push(line));
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

/// A setting in script form: id, label, value, values and submenu label.
type SettingRow = (String, String, String, Option<Vec<String>>, Option<String>);

/// One script step.
#[derive(Deserialize)]
#[serde(
    tag = "do",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
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
    /// Builds a selection list of `[value, label, description]` choices.
    Select {
        /// Name to keep it under.
        name: String,
        /// Choices.
        items: Vec<(String, String, Option<String>)>,
        /// Visible rows.
        max_visible: u32,
        /// `host` or `authored` styles.
        theme: String,
        /// Narrowest and widest primary column.
        #[serde(default)]
        widths: (Option<i64>, Option<i64>),
        /// Whether an authored truncation replaces the rows.
        #[serde(default)]
        truncate: bool,
    },
    /// Replaces or removes a handler of a list or loader: `select`, `cancel`, `change` or `abort`.
    On {
        /// Kept name.
        name: String,
        /// Handler slot.
        slot: String,
        /// Label the handler records with; absent removes the handler.
        #[serde(default)]
        label: Option<String>,
    },
    /// Delivers input to a list or loader.
    Input {
        /// Kept name.
        name: String,
        /// Input.
        data: String,
    },
    /// Filters a selection list.
    Filter {
        /// Kept name.
        name: String,
        /// Filter.
        filter: String,
    },
    /// Selects a selection list row.
    SelectIndex {
        /// Kept name.
        name: String,
        /// Row.
        index: i64,
    },
    /// Reads a selection list's selected choice.
    Selected {
        /// Kept name.
        name: String,
    },
    /// Builds a settings list of `[id, label, value, values, submenu]` settings.
    Settings {
        /// Name to keep it under.
        name: String,
        /// Settings; a submenu label opens a submenu that renders the label.
        items: Vec<SettingRow>,
        /// Visible rows.
        max_visible: u32,
        /// `host` or `authored` styles.
        theme: String,
        /// Whether a search row filters by label.
        #[serde(default)]
        search: bool,
    },
    /// Replaces a setting's value.
    UpdateValue {
        /// Kept name.
        name: String,
        /// Setting id.
        id: String,
        /// Value.
        value: String,
    },
    /// Shows a bordered loader as a widget built from the lent terminal and theme.
    Loader {
        /// Name to keep it under.
        name: String,
        /// Message.
        message: String,
        /// Whether input can cancel it; absent means it can.
        #[serde(default)]
        cancellable: Option<bool>,
    },
    /// Reads whether a loader's signal is cancelled.
    Aborted {
        /// Kept name.
        name: String,
    },
    /// Stops a loader's animation.
    Dispose {
        /// Kept name.
        name: String,
    },
    /// Applies a host selection list style of a role.
    ApplyList {
        /// Role.
        role: String,
        /// Text.
        text: String,
    },
    /// Keeps a keybindings manager: `new`, `create`, `definitions` or `global`.
    Keybindings {
        /// Name to keep it under.
        name: String,
        /// Where it comes from.
        source: String,
        /// User bindings of a new manager.
        #[serde(default)]
        user: Option<Value>,
        /// File of a new manager.
        #[serde(default)]
        path: Option<String>,
        /// Agent directory of a created manager.
        #[serde(default)]
        dir: Option<String>,
        /// `[id, keys, description]` definitions.
        #[serde(default)]
        definitions: Option<Vec<(String, Value, Option<String>)>>,
    },
    /// One request on a kept keybindings manager.
    Keys {
        /// Kept name.
        name: String,
        /// The request.
        op: String,
        /// Input of a match.
        #[serde(default)]
        data: Option<String>,
        /// Keybinding of a match, keys or definition.
        #[serde(default)]
        id: Option<String>,
        /// User bindings to set.
        #[serde(default)]
        user: Option<Value>,
    },
    /// A keybinding's keys, or a keybinding or literal key hint.
    Hint {
        /// The keybinding.
        #[serde(default)]
        id: Option<String>,
        /// The literal key.
        #[serde(default)]
        key: Option<String>,
        /// The description.
        #[serde(default)]
        description: Option<String>,
    },
    /// Applies the live editor border and selected-text styles to `text`.
    EditorTheme {
        /// Text.
        text: String,
    },
    /// Shows a widget whose factory builds an editor on the lent terminal, a custom editor
    /// when it names a kept keybindings manager.
    Editor {
        /// Name to keep it under.
        name: String,
        /// The kept keybindings manager of a custom editor.
        #[serde(default)]
        keys: Option<String>,
        /// `host` or `authored` styles.
        theme: String,
        /// Side padding and completion rows.
        #[serde(default)]
        options: (Option<f64>, Option<f64>),
    },
    /// One request on a kept editor.
    Edit {
        /// Kept name.
        name: String,
        /// The request.
        op: String,
        /// Text of a change or history entry.
        #[serde(default)]
        text: Option<String>,
        /// Number of a padding or completion rows change.
        #[serde(default)]
        number: Option<f64>,
        /// Flag of a submission or focus change.
        #[serde(default)]
        flag: Option<bool>,
    },
    /// Replaces the handler of an application action of a kept custom editor.
    Action {
        /// Kept name.
        name: String,
        /// The action.
        action: String,
        /// Label the handler records.
        label: String,
    },
    /// Applies a host settings row style of a role.
    ApplyRow {
        /// Role.
        role: String,
        /// Text.
        text: String,
        /// Whether the row is selected.
        selected: bool,
    },
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
        step => lists(step),
    }
}

/// Selection list styles: the host's, or authored ones that wrap text in their role.
fn select_theme(choice: &str) -> ExtensionResult<SelectListTheme> {
    if choice == "host" {
        return get_select_list_theme();
    }
    let role = |name: &str| style(name);
    Ok(SelectListTheme {
        selected_prefix: role("selectedPrefix")?,
        selected_text: role("selectedText")?,
        description: role("description")?,
        scroll_info: role("scrollInfo")?,
        no_match: role("noMatch")?,
    })
}

/// Settings list styles: the host's, or authored ones that wrap text in their role.
fn settings_theme(choice: &str) -> ExtensionResult<SettingsListTheme> {
    if choice == "host" {
        return get_settings_list_theme();
    }
    let row = |name: &'static str| {
        RowStyle::new(move |text, selected| Ok(format!("{name}({text},{selected})")))
    };
    Ok(SettingsListTheme {
        label: row("label"),
        value: row("value"),
        description: style("description")?,
        cursor: "> ".to_owned(),
        hint: style("hint")?,
    })
}

/// A submenu that renders its label and value, and closes on Enter with `picked` or on
/// Escape without a value.
struct Submenu {
    /// Label.
    label: String,
    /// The value it opened with.
    value: String,
    /// Closes it.
    done: SubmenuDone,
}

impl ComponentView for Submenu {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(vec![format!(
            "submenu {} {} {width}",
            self.label, self.value
        )])
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        record(format!("{} input {}", self.label, json!(data)));
        match data {
            "\r" => self.done.done(Some("picked")),
            "\x1b" => self.done.done(None),
            _ => Ok(()),
        }
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// A setting from its script form.
fn setting((id, label, current_value, values, submenu): SettingRow) -> SettingItem {
    SettingItem {
        id,
        label,
        description: None,
        current_value,
        values,
        submenu: submenu.map(|label| {
            Rc::new(move |value: &str, done, _: CallbackEmitter<'_>| {
                record(format!("{label} opened {value}"));
                Ok(Component::new(Submenu {
                    label: label.clone(),
                    value: value.to_owned(),
                    done,
                }))
            }) as maestro_extensions_wasm::Submenu
        }),
    }
}

/// Builds a selection list, settings list or loader.
fn lists(step: Step) -> ExtensionResult<Value> {
    match step {
        step @ Step::Select { .. } => build_select(step),
        step @ Step::Settings { .. } => build_settings(step),
        Step::Loader {
            name,
            message,
            cancellable,
        } => loader(name, message, cancellable),
        step => list_steps(step),
    }
}

/// Builds and keeps a selection list.
fn build_select(step: Step) -> ExtensionResult<Value> {
    let Step::Select {
        name,
        items,
        max_visible,
        theme,
        widths,
        truncate,
    } = step
    else {
        return Err("expected a selection list".to_owned());
    };
    let items: Vec<_> = items
        .into_iter()
        .map(|(value, label, description)| SelectItem {
            value,
            label,
            description,
        })
        .collect();
    let truncate_primary = truncate.then(|| {
        Rc::new(
            |context: &maestro_extensions_wasm::SelectListTruncatePrimaryContext,
             _: CallbackEmitter<'_>| {
                Ok(format!(
                    "cut {} {} {} {}",
                    context.text, context.max_width, context.column_width, context.is_selected
                ))
            },
        ) as maestro_extensions_wasm::TruncatePrimary
    });
    let layout = SelectListLayoutOptions {
        min_primary_column_width: widths.0,
        max_primary_column_width: widths.1,
        truncate_primary,
    };
    let list = SelectList::new(&items, max_visible, &select_theme(&theme)?, &layout)?;
    Ok(keep(name, Kept::Select(list)))
}

/// Builds and keeps a settings list whose callbacks record their calls.
fn build_settings(step: Step) -> ExtensionResult<Value> {
    let Step::Settings {
        name,
        items,
        max_visible,
        theme,
        search,
    } = step
    else {
        return Err("expected a settings list".to_owned());
    };
    let items = items.into_iter().map(setting).collect();
    let change = Rc::new(|id: &str, value: &str, _: CallbackEmitter<'_>| {
        record(format!("change {id} {value}"));
        Ok(())
    });
    let cancel = Rc::new(|_: CallbackEmitter<'_>| {
        record("cancel settings".to_owned());
        Ok(())
    });
    let options = SettingsListOptions {
        enable_search: search,
    };
    let theme = settings_theme(&theme)?;
    let list = SettingsList::new(items, max_visible, &theme, (change, cancel), options)?;
    Ok(keep(name, Kept::Settings(list)))
}

/// Shows a widget whose factory builds a bordered loader and keeps it under `name`.
fn loader(name: String, message: String, cancellable: Option<bool>) -> ExtensionResult<Value> {
    let ui = UI.with_borrow(Clone::clone).ok_or("the script has no UI")?;
    let factory: ComponentFactory = Rc::new(move |tui, theme, _| {
        let options = cancellable.map(|cancellable| BorderedLoaderOptions {
            cancellable: Some(cancellable),
        });
        let loader = BorderedLoader::new(&tui, &theme, &message, options)?;
        let component = Component::from(&loader);
        KEPT.with_borrow_mut(|kept| kept.insert(name.clone(), Kept::Loader(loader)));
        Ok(ExtensionComponent {
            component,
            dispose: None,
            set_expanded: None,
            width: None,
        })
    });
    ui.set_widget_factory("loader", factory, None)?;
    Ok(json!(true))
}

/// Handles, filters, selects and reads lists and loaders.
fn list_steps(step: Step) -> ExtensionResult<Value> {
    match step {
        Step::On { name, slot, label } => on(&name, &slot, label).map(|()| json!(true)),
        Step::Input { name, data } => {
            match kept(&name)? {
                Kept::Select(list) => list.handle_input(&data)?,
                Kept::Settings(list) => list.handle_input(&data)?,
                Kept::Loader(loader) => loader.handle_input(&data)?,
                Kept::Editor(editor) => editor.handle_input(&data)?,
                Kept::Custom(editor) => editor.handle_input(&data)?,
                _ => return Err(format!("{name} takes no input")),
            }
            Ok(json!(true))
        }
        Step::Filter { name, filter } => select(&name)?.set_filter(&filter).map(|()| json!(true)),
        Step::SelectIndex { name, index } => select(&name)?
            .set_selected_index(index)
            .map(|()| json!(true)),
        Step::Selected { name } => {
            let item = select(&name)?.get_selected_item()?;
            Ok(json!(item.map(|item| [item.value, item.label])))
        }
        Step::UpdateValue { name, id, value } => match kept(&name)? {
            Kept::Settings(list) => list.update_value(&id, &value).map(|()| json!(true)),
            _ => Err(format!("{name} has no settings")),
        },
        Step::Aborted { name } => Ok(json!(bordered(&name)?.signal()?.aborted())),
        Step::Dispose { name } => bordered(&name)?.dispose().map(|()| json!(true)),
        Step::ApplyList { role, text } => {
            let theme = get_select_list_theme()?;
            let style = match role.as_str() {
                "selectedText" => theme.selected_text,
                _ => theme.no_match,
            };
            Ok(json!(style.apply(&text)?))
        }
        Step::ApplyRow {
            role,
            text,
            selected,
        } => {
            let theme = get_settings_list_theme()?;
            let style = if role == "label" {
                theme.label
            } else {
                theme.value
            };
            Ok(json!(style.apply(&text, selected)?))
        }
        step => keys_steps(step),
    }
}

/// Keybinding keys from their script form: one key, or a list.
fn keys_of(value: &Value) -> ExtensionResult<KeybindingKeys> {
    match value {
        Value::String(key) => Ok(KeybindingKeys::Single(key.clone())),
        value => serde_json::from_value(value.clone())
            .map(KeybindingKeys::Multiple)
            .map_err(|error| error.to_string()),
    }
}

/// Bindings from their script form: `[id, keys or null]` pairs.
fn bindings(value: Option<Value>) -> ExtensionResult<KeybindingsConfig> {
    let pairs: Vec<(String, Value)> =
        serde_json::from_value(value.unwrap_or(json!([]))).map_err(|error| error.to_string())?;
    pairs
        .into_iter()
        .map(|(id, keys)| {
            Ok((
                id,
                if keys.is_null() {
                    None
                } else {
                    Some(keys_of(&keys)?)
                },
            ))
        })
        .collect()
}

/// The script form of keys.
fn keys_json(keys: &KeybindingKeys) -> Value {
    match keys {
        KeybindingKeys::Single(key) => json!(key),
        KeybindingKeys::Multiple(keys) => json!(keys),
    }
}

/// The script form of bindings.
pub(super) fn bindings_json(config: &KeybindingsConfig) -> Value {
    json!(
        config
            .iter()
            .map(|(id, keys)| json!([id, keys.as_ref().map(keys_json)]))
            .collect::<Vec<_>>()
    )
}

/// A kept keybindings manager from its source.
fn manager_of(step: Step) -> ExtensionResult<(String, KeybindingsManager)> {
    let Step::Keybindings {
        name,
        source,
        user,
        path,
        dir,
        definitions,
    } = step
    else {
        return Err("expected a keybindings manager".to_owned());
    };
    let manager = match source.as_str() {
        "new" => KeybindingsManager::new(bindings(user)?, path.as_deref())?,
        "create" => KeybindingsManager::create(dir.as_deref())?,
        "definitions" => {
            let definitions = definitions
                .unwrap_or_default()
                .into_iter()
                .map(|(id, keys, description)| {
                    let default_keys = keys_of(&keys)?;
                    Ok((
                        id,
                        KeybindingDefinition {
                            default_keys,
                            description,
                        },
                    ))
                })
                .collect::<ExtensionResult<_>>()?;
            KeybindingsManager::from_definitions(definitions, bindings(user)?)?
        }
        _ => get_keybindings()?,
    };
    Ok((name, manager))
}

/// Keeps keybindings managers, sends their requests and reads hints and editor styles.
fn keys_steps(step: Step) -> ExtensionResult<Value> {
    match step {
        step @ Step::Keybindings { .. } => {
            let (name, manager) = manager_of(step)?;
            Ok(keep(name, Kept::Keys(manager)))
        }
        Step::Keys {
            name,
            op,
            data,
            id,
            user,
        } => {
            let Kept::Keys(manager) = kept(&name)? else {
                return Err(format!("{name} is no keybindings manager"));
            };
            keys_request(&manager, &op, (data, id), user)
        }
        Step::Hint {
            id,
            key,
            description,
        } => match (id, key, description) {
            (Some(id), _, Some(description)) => Ok(json!(key_hint(&id, &description)?)),
            (_, Some(key), Some(description)) => Ok(json!(raw_key_hint(&key, &description)?)),
            (Some(id), _, None) => Ok(json!(key_text(&id)?)),
            _ => Err("expected a keybinding or a key".to_owned()),
        },
        Step::EditorTheme { text } => {
            let theme = get_editor_theme()?;
            let border = theme.border_color.apply(&text)?;
            Ok(json!([
                border,
                theme.select_list.selected_text.apply(&text)?
            ]))
        }
        step => editor_steps(step),
    }
}

/// Shows a widget whose factory builds an editor and keeps it under `name`.
fn editor(
    name: String,
    keys: Option<String>,
    theme: &str,
    (padding_x, autocomplete_max_visible): (Option<f64>, Option<f64>),
) -> ExtensionResult<Value> {
    let ui = UI.with_borrow(Clone::clone).ok_or("the script has no UI")?;
    let manager = match keys {
        Some(keys) => match kept(&keys)? {
            Kept::Keys(manager) => Some(manager),
            _ => return Err(format!("{keys} is no keybindings manager")),
        },
        None => None,
    };
    let theme = if theme == "host" {
        get_editor_theme()?
    } else {
        EditorTheme {
            border_color: style("border")?,
            select_list: select_theme(theme)?,
        }
    };
    let options = EditorOptions {
        padding_x,
        autocomplete_max_visible,
    };
    let factory: ComponentFactory = Rc::new(move |tui, _, _| {
        let editor = match &manager {
            Some(manager) => Kept::Custom(CustomEditor::new(&tui, &theme, manager, options)?),
            None => Kept::Editor(Editor::new(&tui, &theme, options)?),
        };
        let component = editor.component();
        KEPT.with_borrow_mut(|kept| kept.insert(name.clone(), editor));
        Ok(ExtensionComponent {
            component,
            dispose: None,
            set_expanded: None,
            width: None,
        })
    });
    ui.set_widget_factory("editor", factory, None)?;
    Ok(json!(true))
}

/// The editor operations of the editor kept under `name`.
fn edited(name: &str) -> ExtensionResult<Editor> {
    match kept(name)? {
        Kept::Editor(editor) => Ok(editor),
        Kept::Custom(editor) => Ok((*editor).clone()),
        _ => Err(format!("{name} is no editor")),
    }
}

/// The custom editor kept under `name`.
fn custom(name: &str) -> ExtensionResult<CustomEditor> {
    match kept(name)? {
        Kept::Custom(editor) => Ok(editor),
        _ => Err(format!("{name} is no custom editor")),
    }
}

/// Sends one request to an editor.
fn edit(
    editor: &Editor,
    op: &str,
    (text, number, flag): (Option<String>, Option<f64>, Option<bool>),
) -> ExtensionResult<Value> {
    let text = text.unwrap_or_default();
    let number = number.unwrap_or_default();
    let flag = flag.unwrap_or_default();
    Ok(match op {
        "getText" => json!(editor.get_text()?),
        "setText" => json!(editor.set_text(&text)?),
        "getExpandedText" => json!(editor.get_expanded_text()?),
        "getLines" => json!(editor.get_lines()?),
        "getCursor" => json!(editor.get_cursor()?),
        "insertText" => json!(editor.insert_text_at_cursor(&text)?),
        "addToHistory" => json!(editor.add_to_history(&text)?),
        "getPaddingX" => json!(editor.get_padding_x()?),
        "setPaddingX" => json!(editor.set_padding_x(number)?),
        "getAutocompleteMaxVisible" => json!(editor.get_autocomplete_max_visible()?),
        "setAutocompleteMaxVisible" => json!(editor.set_autocomplete_max_visible(number)?),
        "disableSubmit" => json!(editor.disable_submit()?),
        "setDisableSubmit" => json!(editor.set_disable_submit(flag)?),
        "isShowingAutocomplete" => json!(editor.is_showing_autocomplete()?),
        "focused" => json!(editor.focused()?),
        "setFocused" => json!(editor.set_focused(flag)?),
        "setBorderColor" => json!(editor.set_border_color(&style(&text)?)?),
        _ => return Err(format!("unknown editor request {op}")),
    })
}

/// Builds, changes and reads editors.
fn editor_steps(step: Step) -> ExtensionResult<Value> {
    match step {
        Step::Editor {
            name,
            keys,
            theme,
            options,
        } => editor(name, keys, &theme, options),
        Step::Edit {
            name,
            op,
            text,
            number,
            flag,
        } => edit(&edited(&name)?, &op, (text, number, flag)),
        Step::Action {
            name,
            action,
            label,
        } => {
            let handler = Rc::new(move |_: CallbackEmitter<'_>| {
                record(label.clone());
                Ok(())
            });
            custom(&name)?.on_action(&action, handler)?;
            Ok(json!(true))
        }
        step => children(step).map(|()| json!(true)),
    }
}

/// Replaces or removes the `slot` handler of the editor under `name`.
fn on_editor(name: &str, slot: &str, label: Option<String>) -> ExtensionResult<()> {
    let text = label.clone().map(|label| {
        Rc::new(move |text: &str, _: CallbackEmitter<'_>| {
            record(format!("{label} {text}"));
            Ok(())
        }) as maestro_extensions_wasm::TextHandler
    });
    let notice = label.clone().map(|label| {
        Rc::new(move |_: CallbackEmitter<'_>| {
            record(label.clone());
            Ok(())
        }) as maestro_extensions_wasm::WidgetNotice
    });
    let shortcut = label.map(|label| {
        Rc::new(move |data: &str, _: CallbackEmitter<'_>| {
            record(format!("{label} {data}"));
            Ok(data == "ctrl+k" || data == "\u{b}")
        }) as maestro_extensions_wasm::EditorShortcut
    });
    match slot {
        "submit" => edited(name)?.set_on_submit(text),
        "change" => edited(name)?.set_on_change(text),
        "escape" => custom(name)?.set_on_escape(notice),
        "ctrlD" => custom(name)?.set_on_ctrl_d(notice),
        "pasteImage" => custom(name)?.set_on_paste_image(notice),
        "shortcut" => custom(name)?.set_on_extension_shortcut(shortcut),
        _ => Err(format!("{name} has no {slot} handler")),
    }
}

/// Sends one request to a keybindings manager.
fn keys_request(
    manager: &KeybindingsManager,
    op: &str,
    (data, id): (Option<String>, Option<String>),
    user: Option<Value>,
) -> ExtensionResult<Value> {
    let id = id.unwrap_or_default();
    Ok(match op {
        "matches" => json!(manager.matches(&data.unwrap_or_default(), &id)?),
        "getKeys" => json!(manager.get_keys(&id)?),
        "getDefinition" => json!(manager.get_definition(&id)?.map(|definition| {
            json!([keys_json(&definition.default_keys), definition.description])
        })),
        "getConflicts" => json!(
            manager
                .get_conflicts()?
                .into_iter()
                .map(|conflict| json!([conflict.key, conflict.keybindings]))
                .collect::<Vec<_>>()
        ),
        "setUserBindings" => {
            manager.set_user_bindings(bindings(user)?)?;
            json!(true)
        }
        "getUserBindings" => bindings_json(&manager.get_user_bindings()?),
        "getResolvedBindings" => bindings_json(&manager.get_resolved_bindings()?),
        "reload" => {
            manager.reload()?;
            json!(true)
        }
        _ => bindings_json(&manager.get_effective_config()?),
    })
}

/// The selection list kept under `name`.
fn select(name: &str) -> ExtensionResult<SelectList> {
    match kept(name)? {
        Kept::Select(list) => Ok(list),
        _ => Err(format!("{name} is no selection list")),
    }
}

/// The loader kept under `name`.
fn bordered(name: &str) -> ExtensionResult<BorderedLoader> {
    match kept(name)? {
        Kept::Loader(loader) => Ok(loader),
        _ => Err(format!("{name} is no loader")),
    }
}

/// Replaces or removes the `slot` handler of the list or loader under `name`.
fn on(name: &str, slot: &str, label: Option<String>) -> ExtensionResult<()> {
    let label_of = label.clone();
    let item = label.clone().map(|label| {
        Rc::new(move |item: &SelectItem, _: CallbackEmitter<'_>| {
            record(format!("{label} {}", item.value));
            Ok(())
        }) as maestro_extensions_wasm::SelectHandler
    });
    let notice = label.map(|label| {
        Rc::new(move |_: CallbackEmitter<'_>| {
            record(label.clone());
            Ok(())
        }) as maestro_extensions_wasm::WidgetNotice
    });
    match (kept(name)?, slot) {
        (Kept::Select(list), "select") => list.set_on_select(item),
        (Kept::Select(list), "cancel") => list.set_on_cancel(notice),
        (Kept::Select(list), "change") => list.set_on_selection_change(item),
        (Kept::Loader(loader), "abort") => loader.set_on_abort(notice),
        (Kept::Editor(_) | Kept::Custom(_), _) => on_editor(name, slot, label_of),
        _ => Err(format!("{name} has no {slot} handler")),
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
pub fn run(steps: Vec<Step>, ui: Option<ExtensionUIContext>) -> Value {
    UI.set(ui);
    let outcomes: Vec<_> = steps
        .into_iter()
        .map(|step| match build(step) {
            Ok(value) => json!({ "ok": value }),
            Err(error) => json!({ "error": error }),
        })
        .collect();
    json!(outcomes)
}
