//! The full changelog command and the rows it appends to the transcript.

use super::InteractiveMode;
use crate::components::dynamic_border::DynamicBorder;
use maestro_app::configuration::config::get_package_dir;
use maestro_app::{ChangelogEntry, get_changelog_path, parse_changelog};
use maestro_theme::{Theme, ThemeColor};
use maestro_tui::{Markdown, MarkdownOptions, Spacer, Text};
use std::{error::Error, rc::Rc};

impl InteractiveMode {
    /// Appends every changelog section, in reverse file order, as one bordered transcript card, or
    /// the row that says no section was found, then requests a frame.
    ///
    /// # Errors
    /// Returns the failure of the package-directory or changelog-path lookup, or of the
    /// published theme's colours.
    pub(super) fn handle_changelog_command(&self) -> Result<(), Box<dyn Error>> {
        let path = get_changelog_path(&get_package_dir()?)?;
        let markdown = join_contents(&parse_changelog(&path));
        let live = self.themes.theme();
        let theme = live.get()?;
        let title = Theme::bold(&theme.fg(&ThemeColor::Accent, "What's New")?, true);
        self.chat_container.add_child(Rc::new(Spacer::new(1)));
        self.chat_container
            .add_child(Rc::new(DynamicBorder::new(live.clone(), None)));
        self.chat_container
            .add_child(Rc::new(Text::new(title, 1, 0, None)));
        self.chat_container.add_child(Rc::new(Spacer::new(1)));
        self.chat_container.add_child(Rc::new(Markdown::new(
            markdown,
            MarkdownOptions {
                clean_content: false,
                padding_x: 1,
                padding_y: 1,
                default_text_style: None,
            },
            self.get_markdown_theme_with_settings(),
            self.tool_environment.terminal_image.clone(),
        )));
        self.chat_container
            .add_child(Rc::new(DynamicBorder::new(live, None)));
        self.ui.request_render(false);
        Ok(())
    }
}

/// The section contents of `entries` in reverse file order, separated by a blank line, or the row
/// that says no section was found. The contents are borrowed, never copied.
fn join_contents(entries: &[ChangelogEntry]) -> String {
    if entries.is_empty() {
        return "No changelog entries found.".to_owned();
    }
    let contents: Vec<&str> = entries
        .iter()
        .rev()
        .map(|entry| entry.content.as_str())
        .collect();
    contents.join("\n\n")
}
