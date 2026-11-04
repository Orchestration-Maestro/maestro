//! The full changelog command and the rows it appends to the transcript.

use super::InteractiveMode;
use crate::components::dynamic_border::DynamicBorder;
use maestro_app::configuration::config::get_package_dir;
use maestro_app::{
    ChangelogEntry, ReadonlySessionManager, get_changelog_path, get_new_entries, parse_changelog,
};
use maestro_theme::{Theme, ThemeColor};
use maestro_tui::{Markdown, MarkdownOptions, Spacer, Text, hyperlink};
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
        self.append_markdown_panel("What's New", markdown)
    }

    /// Appends a bordered Markdown card with a captured heading and requests a frame.
    ///
    /// # Errors
    /// Returns a failure reading the heading's theme.
    pub(super) fn append_markdown_panel(
        &self,
        heading: &str,
        markdown: String,
    ) -> Result<(), Box<dyn Error>> {
        let live = self.themes.theme();
        let theme = live.get()?;
        let title = Theme::bold(&theme.fg(&ThemeColor::Accent, heading)?, true);
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
        start_notice_work(self.tool_environment.runtime.as_ref(), work);
    }
}

/// Joins borrowed sections in the caller's order with one blank line between them.
pub(super) fn join_changelog<'a>(entries: impl Iterator<Item = &'a ChangelogEntry>) -> String {
    entries
        .map(|entry| entry.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

impl InteractiveMode {
    /// Appends the available-release card and requests a frame.
    ///
    /// When the terminal supports links, the changelog row is a link labelled
    /// `open changelog`; otherwise it prints the changelog address. Link support is
    /// read once, when the card is built.
    ///
    /// # Errors
    /// Returns a published-theme colour failure.
    pub fn show_new_version_notification(&self, new_version: &str) -> Result<(), Box<dyn Error>> {
        let theme = self.themes.theme().get()?;
        let action = theme.fg(
            &ThemeColor::Accent,
            &format!("{} update", maestro_app::configuration::config::APP_NAME),
        )?;
        let instruction = theme.fg(
            &ThemeColor::Muted,
            &format!("New version {new_version} is available. Run "),
        )?;
        let label = theme.fg(&ThemeColor::Muted, "Changelog: ")?;
        let url = "https://github.com/Orchestration-Maestro/maestro/blob/main/CHANGELOG.md";
        let link = if self
            .tool_environment
            .terminal_image
            .get_capabilities()
            .hyperlinks
        {
            hyperlink(&theme.fg(&ThemeColor::Accent, "open changelog")?, url)
        } else {
            theme.fg(&ThemeColor::Accent, url)?
        };
        self.show_update_notification(
            "Update Available",
            &format!("{instruction}{action}\n{label}{link}"),
        )
    }

    /// Appends the package-update card in supplied order and requests a frame.
    ///
    /// # Errors
    /// Returns a published-theme colour failure.
    pub fn show_package_update_notification(
        &self,
        packages: &[String],
    ) -> Result<(), Box<dyn Error>> {
        let theme = self.themes.theme().get()?;
        let action = theme.fg(
            &ThemeColor::Accent,
            &format!("{} update", maestro_app::configuration::config::APP_NAME),
        )?;
        let instruction = theme.fg(&ThemeColor::Muted, "Package updates are available. Run ")?;
        let label = theme.fg(&ThemeColor::Muted, "Packages:")?;
        let lines = packages
            .iter()
            .map(|name| format!("- {name}"))
            .collect::<Vec<_>>()
            .join("\n");
        self.show_update_notification(
            "Package Updates Available",
            &format!("{instruction}{action}\n{label}\n{lines}"),
        )
    }

    /// Frames prepared update text with warning rules that read the live theme.
    fn show_update_notification(&self, title: &str, body: &str) -> Result<(), Box<dyn Error>> {
        let live = self.themes.theme();
        let theme = live.get()?;
        let title = Theme::bold(&theme.fg(&ThemeColor::Warning, title)?, true);
        let color: maestro_tui::editor_component::BorderColor = Rc::new(move |text| {
            maestro_theme::paint(live.get().ok().as_deref(), &ThemeColor::Warning, text)
        });
        self.chat_container.add_child(Rc::new(Spacer::new(1)));
        self.chat_container.add_child(Rc::new(DynamicBorder::new(
            self.themes.theme(),
            Some(color.clone()),
        )));
        self.chat_container
            .add_child(Rc::new(Text::new(format!("{title}\n{body}"), 1, 0, None)));
        self.chat_container.add_child(Rc::new(DynamicBorder::new(
            self.themes.theme(),
            Some(color),
        )));
        self.ui.request_render(false);
        Ok(())
    }
}

/// Starts immediate effects now and transfers pending work to the existing runtime.
fn start_notice_work(
    runtime: &dyn maestro_tui::tui::TuiRuntime,
    work: maestro_app::utils::task::LocalTask,
) {
    maestro_app::utils::task::start_now(work, |work| {
        runtime.spawn_local(Box::pin(async move {
            work.await;
            Ok(())
        }));
    });
}

impl InteractiveMode {
    /// Creates fresh native metadata effects with weak runtime admission.
    pub(super) fn native_package_operations(&self) -> maestro_app::NativePackageOperations {
        let runtime = Rc::downgrade(&self.tool_environment.runtime);
        maestro_app::NativePackageOperations::with_spawn(Rc::new(|| false), move |work| {
            let runtime = runtime
                .upgrade()
                .ok_or_else(|| std::io::Error::other("package runtime is no longer available"))?;
            start_notice_work(runtime.as_ref(), work);
            Ok(())
        })
    }

    /// Reads current package context only when startup observations are enabled.
    pub(super) async fn check_for_package_updates(&self) -> Vec<String> {
        let offline = self.runtime_host.services().offline;
        if offline.unwrap_or_else(|| {
            maestro_app::configuration::flag_is_set(
                self.tool_environment
                    .runtime
                    .environment("MAESTRO_OFFLINE")
                    .as_deref(),
            )
        }) {
            return Vec::new();
        }
        let check = async {
            let session = self.runtime_host.session();
            let cwd = session.session_manager().borrow().get_cwd().to_owned();
            let manager = Rc::new(
                maestro_app::DefaultPackageManager::new(
                    maestro_app::PackageManagerOptions {
                        cwd,
                        agent_dir: maestro_app::configuration::config::get_agent_dir()?,
                        settings_manager: Rc::clone(session.settings_manager()),
                    },
                    self.native_package_operations(),
                )
                .with_offline(offline),
            );
            manager.check_for_available_updates().await
        };
        check
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|update| update.display_name)
            .collect()
    }
}

impl InteractiveMode {
    /// Queries both options before choosing the first applicable keyboard warning.
    pub(super) async fn check_tmux_keyboard_setup(&self) -> Option<String> {
        if !maestro_app::configuration::flag_is_set(
            self.tool_environment.runtime.environment("TMUX").as_deref(),
        ) {
            return None;
        }
        let keys = self.run_tmux_show("extended-keys");
        let format = self.run_tmux_show("extended-keys-format");
        let (keys, format) = (keys.await, format.await);
        let keys = keys?;
        if keys != "on" && keys != "always" {
            return Some("tmux extended-keys is off. Modified Enter keys may not work. Add `set -g extended-keys on` to ~/.tmux.conf and restart tmux.".into());
        }
        (format.as_deref() == Some("xterm")).then(|| "tmux extended-keys-format is xterm. Maestro works best with csi-u. Add `set -g extended-keys-format csi-u` to ~/.tmux.conf and restart tmux.".into())
    }

    /// Starts native capture now, independently of the owned notice deadline.
    pub(super) fn run_tmux_show(
        &self,
        option: &'static str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<String>>>> {
        use maestro_app::PackageOperations;
        let (send, receive) = tokio::sync::oneshot::channel();
        let send = Rc::new(std::cell::RefCell::new(Some(send)));
        let expire = send.clone();
        let operations = self.native_package_operations();
        start_notice_work(
            self.tool_environment.runtime.as_ref(),
            Box::pin(async move {
                let result = operations
                    .run_command_capture(
                        "tmux",
                        &["show".into(), "-gv".into(), option.into()],
                        maestro_app::CommandCaptureOptions {
                            cwd: None,
                            timeout: Some(std::time::Duration::from_millis(2_000)),
                            env: &[],
                        },
                    )
                    .await
                    .ok();
                let sender = send.borrow_mut().take();
                if let Some(send) = sender {
                    let _ = send.send(result);
                }
            }),
        );
        let mut timer = self.tool_environment.runtime.schedule(
            std::time::Duration::from_millis(2_000),
            Box::new(move || {
                let sender = expire.borrow_mut().take();
                if let Some(send) = sender {
                    let _ = send.send(None);
                }
                Ok(())
            }),
        );
        Box::pin(async move {
            let result = receive.await.ok().flatten();
            timer.cancel();
            result
        })
    }
}

impl InteractiveMode {
    /// Starts producer prefixes before prompts, deferring their notifications to the host.
    pub(super) fn start_startup_checks(self: &Rc<Self>) {
        let producer = self.clone();
        let mode = self.clone();
        start_notice_check(
            self.tool_environment.runtime.as_ref(),
            async move {
                let options = maestro_app::configuration::version_check::VersionCheckOptions {
                    fetch: producer.options.fetch.clone(),
                    offline: producer.runtime_host.services().offline,
                    ..Default::default()
                };
                maestro_app::configuration::version_check::check_for_new_maestro_version(
                    maestro_app::configuration::config::VERSION,
                    producer
                        .options
                        .service_origin
                        .as_deref()
                        .unwrap_or(maestro_app::configuration::config::SERVICE_ORIGIN),
                    &options,
                )
                .await
            },
            move |version| {
                if let Some(version) = version {
                    let _ = mode.show_new_version_notification(&version);
                }
            },
        );
        let mode = self.clone();
        let notify = self.clone();
        start_notice_check(
            self.tool_environment.runtime.as_ref(),
            async move { mode.check_for_package_updates().await },
            move |packages| {
                if !packages.is_empty() {
                    let _ = notify.show_package_update_notification(&packages);
                }
            },
        );
        let mode = self.clone();
        let notify = self.clone();
        start_notice_check(
            self.tool_environment.runtime.as_ref(),
            async move { mode.check_tmux_keyboard_setup().await },
            move |warning| {
                if let Some(warning) = warning {
                    let _ = notify.show_warning(&warning);
                }
            },
        );
    }
}

/// Eagerly admits a producer but always delivers its result through the existing task host.
fn start_notice_check<T: 'static>(
    runtime: &dyn maestro_tui::tui::TuiRuntime,
    work: impl std::future::Future<Output = T> + 'static,
    notify: impl FnOnce(T) + 'static,
) {
    let (send, receive) = tokio::sync::oneshot::channel();
    start_notice_work(
        runtime,
        Box::pin(async move {
            let _ = send.send(work.await);
        }),
    );
    runtime.spawn_local(Box::pin(async move {
        if let Ok(value) = receive.await {
            notify(value);
        }
        Ok(())
    }));
}
