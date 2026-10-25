//! The full changelog command and the rows it appends to the transcript.

use super::InteractiveMode;
use crate::components::dynamic_border::DynamicBorder;
use maestro_app::configuration::config::get_package_dir;
use maestro_app::{ChangelogEntry, get_changelog_path, get_new_entries, parse_changelog};
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
    join_changelog(entries.iter().rev())
}

impl InteractiveMode {
    /// Appends the selected startup history once, without requesting a separate frame.
    ///
    /// # Errors
    /// Returns a theme or heading matcher failure.
    pub(super) fn show_startup_notices_if_needed(&self) -> Result<(), Box<dyn Error>> {
        if self.startup_notices_shown.replace(true) {
            return Ok(());
        }
        let markdown = self.changelog_markdown.borrow();
        let Some(markdown) = markdown.as_deref().filter(|text| !text.is_empty()) else {
            return Ok(());
        };
        let live = self.themes.theme();
        let theme = live.get()?;
        if !self.chat_container.children().borrow().is_empty() {
            self.chat_container.add_child(Rc::new(Spacer::new(1)));
        }
        self.chat_container
            .add_child(Rc::new(DynamicBorder::new(live.clone(), None)));
        let session = self.runtime_host.session();
        let collapse = session.settings_manager().borrow().get_collapse_changelog();
        if collapse {
            let matcher = regress::Regex::new(r"##\s+\[?(\d+\.\d+\.\d+)\]?")?;
            let range = matcher.find(markdown).and_then(|found| found.group(1));
            let version = range.map_or(maestro_app::configuration::config::VERSION, |range| {
                &markdown[range]
            });
            let text = format!(
                "Updated to v{version}. Use {} to view full changelog.",
                Theme::bold("/changelog", true)
            );
            self.chat_container
                .add_child(Rc::new(Text::new(text, 1, 0, None)));
        } else {
            let title = Theme::bold(&theme.fg(&ThemeColor::Accent, "What's New")?, true);
            self.chat_container
                .add_child(Rc::new(Text::new(title, 1, 0, None)));
            self.chat_container.add_child(Rc::new(Spacer::new(1)));
            self.chat_container.add_child(Rc::new(Markdown::new(
                markdown
                    .trim_matches(maestro_app::is_whitespace_scalar)
                    .to_owned(),
                MarkdownOptions {
                    clean_content: false,
                    padding_x: 1,
                    padding_y: 0,
                    default_text_style: None,
                },
                self.get_markdown_theme_with_settings(),
                self.tool_environment.terminal_image.clone(),
            )));
            self.chat_container.add_child(Rc::new(Spacer::new(1)));
        }
        self.chat_container
            .add_child(Rc::new(DynamicBorder::new(live, None)));
        Ok(())
    }

    /// Reads the fresh session's unseen sections and records the installed version.
    ///
    /// # Errors
    /// Returns a package directory or changelog path lookup failure.
    pub(super) fn get_changelog_for_display(&self) -> Result<Option<String>, Box<dyn Error>> {
        let session = self.runtime_host.session();
        if !session
            .messages()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
        {
            return Ok(None);
        }
        let settings = session.settings_manager();
        let last = settings.borrow().get_last_changelog_version();
        let path = get_changelog_path(&get_package_dir()?)?;
        let entries = parse_changelog(&path);
        let selected = last
            .as_deref()
            .filter(|last| !last.is_empty())
            .map(|last| get_new_entries(&entries, last));
        if selected.as_ref().is_none_or(|entries| !entries.is_empty()) {
            settings
                .borrow_mut()
                .set_last_changelog_version(maestro_app::configuration::config::VERSION.to_owned());
            self.report_install_telemetry(maestro_app::configuration::config::VERSION);
        }
        Ok(selected
            .filter(|entries| !entries.is_empty())
            .map(|entries| join_changelog(entries.into_iter())))
    }

    /// Captures reporting policy before handing owned work to the terminal host.
    pub(super) fn report_install_telemetry(&self, version: &str) {
        let session = self.runtime_host.session();
        let work = maestro_app::configuration::telemetry::report_install_telemetry(
            version,
            &session.settings_manager().borrow(),
            self.options
                .service_origin
                .as_deref()
                .unwrap_or(maestro_app::configuration::config::SERVICE_ORIGIN),
            self.options.fetch.clone(),
            self.runtime_host.services().offline,
        );
        self.start_notice_work(work);
    }

    /// Starts immediate effects now and transfers pending work to the existing runtime.
    fn start_notice_work(&self, work: maestro_app::utils::task::LocalTask) {
        maestro_app::utils::task::start_now(work, |work| {
            self.tool_environment
                .runtime
                .spawn_local(Box::pin(async move {
                    work.await;
                    Ok(())
                }));
        });
    }
}

/// Joins borrowed sections in the caller's order with one blank line between them.
fn join_changelog<'a>(entries: impl Iterator<Item = &'a ChangelogEntry>) -> String {
    entries
        .map(|entry| entry.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}
