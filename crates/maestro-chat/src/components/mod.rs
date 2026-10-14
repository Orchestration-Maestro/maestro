//! Interactive borders, countdowns, loaders and editor routing.

/// Gives message widgets their existing child-array owner's editing interface.
macro_rules! message_children {
    ($name:ty, $target:ty, $($field:tt)+) => {
        impl std::ops::Deref for $name {
            type Target = $target;
            fn deref(&self) -> &Self::Target { &self.$($field)+ }
        }
    };
}

pub mod dynamic_border;

pub mod countdown_timer;

pub mod bordered_loader;

pub mod custom_editor;

pub mod session_selector_search;
pub mod tree_selector;

pub mod user_message;

use maestro_theme::{LiveTheme, ThemeBg, ThemeColor, paint};
use maestro_tui::{
    Component, DefaultTextStyle, Markdown, MarkdownOptions, MarkdownTheme, TerminalImage,
    TextDecoration,
};
use std::rc::Rc;

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

pub mod compaction_summary_message;

/// Shared constructor inputs and Markdown composition for transcript widgets.
struct MessageRendering {
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
            },
            self.markdown.clone(),
            self.terminal.clone(),
        ))
    }
}
