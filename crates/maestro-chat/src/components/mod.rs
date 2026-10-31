//! Interactive borders, countdowns, loaders and editor routing.

/// Gives composed widgets their existing child-array owner's editing interface.
macro_rules! component_children {
    ($name:ty, $target:ty, $($field:tt)+) => {
        impl std::ops::Deref for $name {
            type Target = $target;
            fn deref(&self) -> &Self::Target { &self.$($field)+ }
        }
    };
}

pub mod dynamic_border;

pub mod extension_widgets;

pub mod countdown_timer;

pub mod bordered_loader;

pub mod custom_editor;

pub mod session_selector_search;
pub mod tree_selector;

pub mod user_message;

#[cfg(not(target_arch = "wasm32"))]
pub mod footer;

use maestro_theme::{LiveTheme, ThemeBg, ThemeColor, paint};
use maestro_tui::{
    Component, DefaultTextStyle, Markdown, MarkdownOptions, MarkdownTheme, TerminalImage,
    TextDecoration,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Paints a row using the current background, leaving absent themes unstyled.
fn background(theme: LiveTheme, key: ThemeBg) -> Rc<dyn Fn(&str) -> String> {
    Rc::new(move |text| {
        theme
            .get()
            .ok()
            .and_then(|theme| theme.bg(&key, text).ok())
            .unwrap_or_else(|| text.to_owned())
    })
}

/// Prefixes prompt-zone markers without introducing another rendered row.
fn mark_zone(mut lines: Vec<String>) -> Vec<String> {
    if let Some(first) = lines.first_mut() {
        first.insert_str(0, "\x1b]133;A\x07");
    }
    if let Some(last) = lines.last_mut() {
        last.insert_str(0, "\x1b]133;B\x07\x1b]133;C\x07");
    }
    lines
}

pub mod assistant_message;

pub mod branch_summary_message;

pub mod custom_message;

#[cfg(not(target_arch = "wasm32"))]
pub mod skill_invocation_message;

pub mod compaction_summary_message;

/// Shared constructor inputs and Markdown composition for transcript widgets.
struct MessageRendering {
    /// Filters decoded authored Markdown before applying terminal styles.
    clean_content: bool,
    /// Styling slot sampled by foreground callbacks.
    theme: LiveTheme,
    /// Caller-supplied Markdown callbacks.
    markdown: Rc<MarkdownTheme>,
    /// Shared terminal capabilities.
    terminal: TerminalImage,
}
impl MessageRendering {
    /// Retains the caller's rendering inputs.
    fn new(theme: LiveTheme, markdown: Rc<MarkdownTheme>, terminal: TerminalImage) -> Self {
        Self {
            clean_content: false,
            theme,
            markdown,
            terminal,
        }
    }
    /// Paints text using the currently published foreground.
    fn paint(&self, key: &ThemeColor, text: &str) -> String {
        paint(self.theme.get().ok().as_deref(), key, text)
    }
    /// Builds Markdown with optional base foreground and thinking decoration.
    fn markdown(
        &self,
        text: String,
        padding: usize,
        color: Option<ThemeColor>,
        thinking: bool,
    ) -> Rc<dyn Component> {
        let style = color.map(|key| {
            let theme = self.theme.clone();
            DefaultTextStyle {
                color: Some(Box::new(move |text| {
                    paint(theme.get().ok().as_deref(), &key, text)
                })),
                decorations: if thinking {
                    vec![TextDecoration::Italic]
                } else {
                    vec![]
                },
                ..Default::default()
            }
        });
        Rc::new(Markdown::new(
            text,
            MarkdownOptions {
                padding_x: padding,
                padding_y: 0,
                default_text_style: style,
                clean_content: self.clean_content,
            },
            self.markdown.clone(),
            self.terminal.clone(),
        ))
    }
}

pub mod thinking_selector;

/// Composes the shared bounded-column list between live borders.
fn bordered_select_list(
    items: Vec<maestro_tui::SelectItem>,
    max_visible: usize,
    theme: LiveTheme,
) -> (maestro_tui::Container, Rc<maestro_tui::SelectList>) {
    use maestro_tui::Container;
    let container = Container::new();
    let list = bounded_select_list(items, max_visible, theme.clone());
    container.add_child(Rc::new(dynamic_border::DynamicBorder::new(
        theme.clone(),
        None,
    )));
    container.add_child(list.clone());
    container.add_child(Rc::new(dynamic_border::DynamicBorder::new(theme, None)));
    (container, list)
}

pub mod show_images_selector;

pub mod theme_selector;

pub mod user_message_selector;

pub mod model_selector;
pub mod oauth_selector;

pub mod extension_selector;

/// Builds padded dialog chrome and its live countdown title updater.
fn dialog_chrome(
    title: String,
    body: Rc<dyn Component>,
    theme: LiveTheme,
    hint: String,
    bold: bool,
) -> (maestro_tui::Container, Rc<dyn Fn(f64)>) {
    use maestro_tui::{Container, Spacer, Text};
    let title_theme = theme.clone();
    let color = move |text: &str| {
        let text = if bold {
            maestro_theme::Theme::bold(text, true)
        } else {
            text.to_owned()
        };
        paint(
            title_theme.get().ok().as_deref(),
            &ThemeColor::Accent,
            &text,
        )
    };
    let heading = Rc::new(Text::new(color(&title), 1, 0, None));
    let container = Container::new();
    container.add_child(Rc::new(dynamic_border::DynamicBorder::new(
        theme.clone(),
        None,
    )));
    container.add_child(Rc::new(Spacer::new(1)));
    container.add_child(heading.clone());
    container.add_child(Rc::new(Spacer::new(1)));
    container.add_child(body);
    container.add_child(Rc::new(Spacer::new(1)));
    container.add_child(Rc::new(Text::new(hint, 1, 0, None)));
    container.add_child(Rc::new(Spacer::new(1)));
    container.add_child(Rc::new(dynamic_border::DynamicBorder::new(theme, None)));
    let update = Rc::new(move |seconds: f64| {
        let seconds = if seconds.is_infinite() {
            "Infinity".to_owned()
        } else {
            format!("{seconds:.0}")
        };
        heading.set_text(color(&format!("{title} ({seconds}s)")));
    });
    (container, update)
}

/// Admits only positive supplied deadlines with a writer.
fn dialog_countdown(
    timeout: Option<f64>,
    tui: Option<&maestro_tui::TUI>,
    runtime: Rc<dyn maestro_tui::tui::TuiRuntime>,
    update: Rc<dyn Fn(f64)>,
    cancel: Rc<dyn Fn()>,
) -> Option<countdown_timer::CountdownTimer> {
    timeout
        .filter(|timeout| *timeout > 0.0)
        .zip(tui)
        .map(|(timeout, tui)| {
            countdown_timer::CountdownTimer::new(timeout, Some(tui), runtime, update, cancel)
        })
}

pub mod extension_input;

/// Builds the shared bounded primary-column list.
fn bounded_select_list(
    items: Vec<maestro_tui::SelectItem>,
    max_visible: usize,
    theme: LiveTheme,
) -> Rc<maestro_tui::SelectList> {
    use maestro_tui::{SelectList, SelectListLayoutOptions};
    Rc::new(SelectList::new(
        items,
        max_visible,
        maestro_theme::get_select_list_theme(theme),
        SelectListLayoutOptions {
            min_primary_column_width: Some(12),
            max_primary_column_width: Some(32),
            ..Default::default()
        },
    ))
}

pub mod settings_selector;

#[cfg(not(target_arch = "wasm32"))]
pub mod extension_editor;
pub mod login_dialog;

#[cfg(test)]
mod tests;

/// Centers a bounded window while keeping its last page full.
fn list_window(selected: usize, total: usize, capacity: usize) -> std::ops::Range<usize> {
    let start = selected
        .saturating_sub(capacity / 2)
        .min(total.saturating_sub(capacity));
    start..start.saturating_add(capacity).min(total)
}

/// Shows `path` below the first usable home variable as `~` for display.
fn shorten_path_with_home(path: &str, environment: &dyn Fn(&str) -> Option<String>) -> String {
    let home = environment("HOME")
        .filter(|home| !home.is_empty())
        .or_else(|| environment("USERPROFILE").filter(|home| !home.is_empty()));
    home.map_or_else(
        || path.to_owned(),
        |home| maestro_path::shorten_home(path, &home),
    )
}

pub mod bash_execution;
pub mod session_selector;
/// Navigates a nonempty captured view, releasing its borrow before redraw.
fn navigate_list<T>(
    selected: &Cell<usize>,
    filtered: &RefCell<Vec<T>>,
    up: bool,
    redraw: impl FnOnce(),
) {
    let count = filtered.borrow().len();
    if count == 0 {
        return;
    }
    selected.set(if up {
        (selected.get() + count - 1) % count
    } else {
        (selected.get() + 1) % count
    });
    redraw();
}

pub mod scoped_models_selector;
pub use scoped_models_selector::{ModelsCallbacks, ModelsConfig, ScopedModelsSelectorComponent};

pub mod tool_execution;

#[cfg(not(target_arch = "wasm32"))]
pub mod config_selector;
