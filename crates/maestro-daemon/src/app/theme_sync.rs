//! Host appearance precedence and active theme-name selection.

use super::App;
use crate::terminal_theme::HostAppearance;

impl<C: FnMut() -> u128> App<C> {
    /// Updates an observed default colour, inferring appearance only before an explicit report.
    pub fn update_host_terminal_theme(
        &mut self,
        kind: crate::terminal_theme::DefaultColorKind,
        color: crate::terminal_theme::RgbColor,
    ) -> bool {
        let mut changed = false;
        if kind == crate::terminal_theme::DefaultColorKind::Background
            && !self.state.host_terminal_appearance_explicit.0
        {
            changed |= self.set_host_terminal_appearance(
                crate::terminal_theme::inferred_appearance(color),
                false,
            );
        }
        changed
            | self.set_host_terminal_theme(self.state.host_terminal_theme.with_color(kind, color))
    }

    /// Propagates changed host colours without replacing child-owned overrides.
    pub fn set_host_terminal_theme(&mut self, theme: crate::terminal_theme::TerminalTheme) -> bool {
        if theme == self.state.host_terminal_theme {
            return false;
        }
        self.state.host_terminal_theme = theme;
        self.launch.theme = theme;
        for runtime in self.terminal_runtimes.values() {
            runtime.apply_host_terminal_theme(theme);
        }
        self.launch.render_dirty.request_generic();
        self.launch.render_notify.notify_one();
        true
    }

    /// Retains an appearance report unless an explicit report supersedes inference.
    /// Returns whether the effective theme name changed.
    pub fn set_host_terminal_appearance(
        &mut self,
        appearance: HostAppearance,
        explicit: bool,
    ) -> bool {
        if (self.state.host_terminal_appearance == Some(appearance)
            && self.state.host_terminal_appearance_explicit.0 == explicit)
            || (self.state.host_terminal_appearance_explicit.0 && !explicit)
        {
            return false;
        }
        self.state.host_terminal_appearance = Some(appearance);
        self.state.host_terminal_appearance_explicit.0 = explicit;
        self.launch.appearance = Some(appearance);
        for runtime in self.terminal_runtimes.values() {
            runtime.apply_host_terminal_appearance(Some(appearance));
        }
        self.refresh_effective_app_theme()
    }

    /// Refreshes the active name while palette construction remains renderer-owned.
    pub fn refresh_effective_app_theme(&mut self) -> bool {
        let name = self
            .state
            .theme_runtime
            .effective_name(self.state.host_terminal_appearance);
        if name == self.state.theme_name {
            return false;
        }
        self.state.theme_name = name.into();
        self.launch.render_dirty.request_generic();
        self.launch.render_notify.notify_one();
        true
    }
}
