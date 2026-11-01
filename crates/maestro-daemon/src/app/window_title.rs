//! Outer terminal title rendering from daemon-owned names and pane metadata.

use super::App;
use crate::settings::window_title::{WindowTitlePart, WindowTitleTemplate, WindowTitleToken};

impl<C: FnMut() -> u128> App<C> {
    /// Parses the configured template, resolving the server hostname once when used.
    pub fn configure_window_title(&mut self, template: &str) {
        self.window_title_template =
            WindowTitleTemplate::parse(template)
                .ok()
                .flatten()
                .map(|template| {
                    let hostname = if template.uses(WindowTitleToken::Hostname) {
                        crate::platform::hostname().unwrap_or_default()
                    } else {
                        String::new()
                    };
                    (template, hostname)
                });
    }

    /// Whether the daemon owns the outer terminal title.
    #[must_use]
    pub fn window_title_configured(&self) -> bool {
        self.window_title_template.is_some()
    }

    /// Whether PTY title changes can affect the outer title.
    #[must_use]
    pub fn window_title_uses_terminal_title(&self) -> bool {
        self.window_title_template
            .as_ref()
            .is_some_and(|(template, _)| template.uses(WindowTitleToken::TerminalTitle))
    }

    /// Renders the configured title, retaining literals when token values are absent.
    #[must_use]
    pub fn window_title(&self) -> Option<String> {
        let (template, hostname) = self.window_title_template.as_ref()?;
        let workspace = self
            .state
            .active
            .and_then(|index| self.state.topology.workspaces.get(index));
        let terminal = workspace.and_then(|ws| {
            self.state
                .terminals
                .get(ws.terminal_id(ws.focused_pane_id()?)?)
        });
        let home = crate::paths::home_directory();
        let mut title = String::new();
        for part in template.parts() {
            let value = match part {
                WindowTitlePart::Literal(literal) => {
                    title.push_str(literal);
                    continue;
                }
                WindowTitlePart::Token(WindowTitleToken::Hostname) => {
                    title.push_str(hostname);
                    continue;
                }
                WindowTitlePart::Token(WindowTitleToken::Workspace) => workspace.map(|ws| {
                    crate::workspace::display_name_from_terminals(
                        ws,
                        &self.state.terminals,
                        home.as_deref(),
                    )
                }),
                WindowTitlePart::Token(WindowTitleToken::Tab) => workspace
                    .and_then(maestro_workspace::workspace::Workspace::active_tab_display_name),
                WindowTitlePart::Token(WindowTitleToken::Pane) => {
                    terminal.and_then(|terminal| terminal.manual_label.clone())
                }
                WindowTitlePart::Token(WindowTitleToken::TerminalTitle) => terminal
                    .and_then(crate::terminal::state::TerminalState::terminal_title_stripped),
            };
            if let Some(value) = value {
                title.push_str(&value);
            }
        }
        Some(title)
    }
}
