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
        step => children(step).map(|()| json!(true)),
    }
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
