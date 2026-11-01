//! A floating overlay for checking how the terminal composes one over the conversation.
//!
//! `/overlay-test` opens a 70-cell overlay with rows of wide characters, themed colors and
//! emoji, above a menu whose first two entries hold an inline text field. Choosing an entry
//! reports its name, with the field's text when there is some; Escape closes the overlay
//! without a report.
// maestro-component: MAESTRO_OVERLAY_TEST_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    CURSOR_MARKER, CallbackEmitter, CommandOptions, Component, ComponentView, CustomDone,
    CustomFactoryOutcome, CustomUIOptions, ExtensionAPI, ExtensionCommandContext,
    ExtensionComponent, ExtensionFuture, ExtensionResult, Focusable, NotificationType, Theme,
    keys::matches_key, utils::slice_by_column, visible_width,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The overlay's width in cells, borders included.
const WIDTH: u16 = 70;
/// The cells between the borders.
const INNER: u16 = WIDTH - 2;
/// Wide characters filling all but one cell of their row.
const WIDE: &str = "幅広い文字で枠線の位置を確かめる한글도함께점검中文繁體简体字";
/// A joined sequence, a flag and single emoji, each two cells wide.
const EMOJI: &str = "🧑\u{200d}🎤 🇨🇦 🎵 🎶 🎹 🎻 🎺 🥁 🎼 🎷 🪕 🎸 🎤 🎧 🎨 🎭 🎬 🎯 🎲 🌟";
/// Words shown in the theme color before each.
const COLORS: [(&str, &str); 7] = [
    ("error", "ERROR"),
    ("success", "SUCCESS"),
    ("warning", "WARNING"),
    ("accent", "ACCENT"),
    ("dim", "DIM"),
    ("error", "and"),
    ("success", "again"),
];

/// One menu entry and its text field, if it has one.
struct Item {
    /// The name reported when the entry is chosen.
    label: &'static str,
    /// Whether the entry holds a text field.
    has_input: bool,
    /// The field's characters.
    text: Vec<char>,
    /// The field's cursor, as a character index.
    cursor: usize,
}

impl Item {
    /// An entry with an empty field, or with none.
    fn new(label: &'static str, has_input: bool) -> Self {
        Self {
            label,
            has_input,
            text: Vec::new(),
            cursor: 0,
        }
    }

    /// Applies one editing key to the field.
    fn edit(&mut self, api: &ExtensionAPI, data: &str) -> ExtensionResult<()> {
        if matches_key(api, data, "backspace")? {
            if self.cursor > 0 {
                self.cursor -= 1;
                self.text.remove(self.cursor);
            }
        } else if matches_key(api, data, "left")? {
            self.cursor = self.cursor.saturating_sub(1);
        } else if matches_key(api, data, "right")? {
            self.cursor = (self.cursor + 1).min(self.text.len());
        } else if let Some(character) = data
            .chars()
            .next()
            .filter(|character| *character >= ' ' && character.len_utf8() == data.len())
        {
            self.text.insert(self.cursor, character);
            self.cursor += 1;
        }
        Ok(())
    }

    /// The report for choosing this entry: its name, and the field's text when there is some.
    fn report(&self) -> String {
        if self.text.is_empty() {
            self.label.to_owned()
        } else {
            format!(
                "{}: \"{}\"",
                self.label,
                self.text.iter().collect::<String>()
            )
        }
    }
}

/// The overlay's menu, selection and focus.
struct Overlay {
    /// Host helpers for key matching and cell widths.
    api: ExtensionAPI,
    /// The live theme lent to the factory.
    theme: Theme,
    /// Closes the overlay with the chosen entry's report, or none.
    done: CustomDone<Option<String>>,
    /// Set by the host while the overlay has focus.
    focused: Cell<bool>,
    /// The selected entry's index.
    selected: Cell<usize>,
    /// The menu entries.
    items: RefCell<[Item; 4]>,
}

impl Overlay {
    /// The rows between the borders, before padding.
    fn rows(&self) -> ExtensionResult<Vec<String>> {
        let theme = &self.theme;
        let mut colors = " Colors:".to_owned();
        for (color, word) in COLORS {
            colors = format!("{colors} {}", theme.fg(color, word)?);
        }
        let mut rows = vec![
            format!(" {}", theme.fg("accent", "🎼 Overlay demo")?),
            String::new(),
            format!(
                " {}",
                theme.fg("dim", "─── Width checks: borders stay aligned ───")?
            ),
            format!(" Wide: {}", theme.fg("warning", WIDE)?),
            colors,
            format!(" Emoji: {EMOJI}"),
            String::new(),
            format!(" {}", theme.fg("dim", "─── Menu ───")?),
        ];
        for (index, item) in self.items.borrow().iter().enumerate() {
            rows.push(self.entry(item, index == self.selected.get())?);
        }
        rows.push(String::new());
        let help = "↑↓ move • type to edit • Enter choose • Esc close";
        rows.push(format!(" {}", theme.fg("dim", help)?));
        Ok(rows)
    }

    /// One menu row. A selected field shows its cursor cell in reverse video; while
    /// focused, the marker before that cell lets the host place the hardware cursor there
    /// for input methods.
    fn entry(&self, item: &Item, selected: bool) -> ExtensionResult<String> {
        let (prefix, color) = if selected {
            (" ▶ ", "accent")
        } else {
            ("   ", "text")
        };
        if !item.has_input {
            return Ok(format!("{prefix}{}", self.theme.fg(color, item.label)?));
        }
        let label = format!(
            "{prefix}{} ",
            self.theme.fg(color, &format!("{}:", item.label))?
        );
        let cells = usize::from(INNER).saturating_sub(visible_width(&self.api, &label)?);
        let (before, under, after) = self.window(item, cells)?;
        if !selected {
            return Ok(format!("{label}{before}{under}{after}"));
        }
        let marker = if self.focused.get() {
            CURSOR_MARKER
        } else {
            ""
        };
        Ok(format!(
            "{label}{before}{marker}\x1b[7m{under}\x1b[27m{after}"
        ))
    }

    /// The field's text before its cursor, the character under it and the text after,
    /// scrolled to fit in `cells` with the cursor kept near the middle of a long field.
    ///
    /// The field is measured joined: characters on either side of the cursor can form one
    /// wider cell, such as a sign and the variation selector after it.
    fn window(&self, item: &Item, cells: usize) -> ExtensionResult<(String, char, String)> {
        let (before, rest) = item.text.split_at(item.cursor);
        let mut before: String = before.iter().collect();
        let under = rest.first().copied().unwrap_or(' ');
        let mut after: String = rest.iter().skip(1).collect();
        let total = visible_width(&self.api, &format!("{before}{under}{after}"))?;
        if total > cells {
            let column = visible_width(&self.api, &before)?;
            let start = column.saturating_sub(cells / 2).min(total - cells);
            before = slice_by_column(&self.api, &before, start, column - start, true)?;
            let joined = format!("{before}{under}{after}");
            let shown = slice_by_column(&self.api, &joined, 0, cells, true)?;
            let kept = shown.get(before.len() + under.len_utf8()..);
            kept.unwrap_or_default().clone_into(&mut after);
        }
        Ok((before, under, after))
    }
}

impl ComponentView for Overlay {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let inner = usize::from(INNER);
        let side = self.theme.fg("border", "│")?;
        let rule = "─".repeat(inner);
        let mut lines = vec![self.theme.fg("border", &format!("╭{rule}╮"))?];
        for row in self.rows()? {
            let padding = inner.saturating_sub(visible_width(&self.api, &row)?);
            lines.push(format!("{side}{row}{}{side}", " ".repeat(padding)));
        }
        lines.push(self.theme.fg("border", &format!("╰{rule}╯"))?);
        Ok(lines)
    }

    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if matches_key(&self.api, data, "escape")? {
            return (self.done)(None);
        }
        let selected = self.selected.get();
        if matches_key(&self.api, data, "return")? {
            let report = self.items.borrow()[selected].report();
            return (self.done)(Some(report));
        }
        let mut items = self.items.borrow_mut();
        if matches_key(&self.api, data, "up")? {
            self.selected.set(selected.saturating_sub(1));
        } else if matches_key(&self.api, data, "down")? {
            self.selected.set((selected + 1).min(items.len() - 1));
        } else if items[selected].has_input {
            items[selected].edit(&self.api, data)?;
        }
        Ok(())
    }

    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }

    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}

impl Focusable for Overlay {
    fn focused(&self, _: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        Ok(self.focused.get())
    }

    fn set_focused(&self, focused: bool, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.focused.set(focused);
        Ok(())
    }
}

/// Shows the overlay until it closes, then reports the chosen entry.
async fn show(api: ExtensionAPI, context: ExtensionCommandContext) -> ExtensionResult<()> {
    let ui = context.ui()?;
    let chosen = ui
        .custom(
            Rc::new(move |_, theme, _, done, _| {
                let overlay = Overlay {
                    api: api.clone(),
                    theme,
                    done,
                    focused: Cell::new(false),
                    selected: Cell::new(0),
                    items: RefCell::new([
                        Item::new("Find", true),
                        Item::new("Launch", true),
                        Item::new("Preferences", false),
                        Item::new("Dismiss", false),
                    ]),
                };
                Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                    component: Component::new(overlay),
                    dispose: None,
                    set_expanded: None,
                    width: Some(f64::from(WIDTH)),
                }))
            }),
            Some(CustomUIOptions {
                overlay: Some(true),
                ..CustomUIOptions::default()
            }),
        )
        .await?;
    if let Some(Some(report)) = chosen {
        ui.notify(&report, Some(NotificationType::Info))?;
    }
    Ok(())
}

/// Registers the `/overlay-test` command.
///
/// # Errors
/// Returns the host's failure when it rejects command registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let helpers = api.clone();
        api.register_command(
            "overlay-test",
            CommandOptions {
                description: Some(
                    "Show a floating overlay with inline fields, wide text and emoji".to_owned(),
                ),
                get_argument_completions: None,
                handler: Rc::new(move |_, context| Box::pin(show(helpers.clone(), context))),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
