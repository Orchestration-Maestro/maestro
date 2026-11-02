//! Live settings application through the existing global document owner.

use super::{
    App,
    state::{ThemeRuntimeConfig, ToastKind, ToastNotification},
};
use crate::settings::{
    io::{apply_loaded, config_diagnostic_summary, load_live_config},
    model::{ConfigReloadReport, ConfigReloadStatus, LoadedConfig},
};
use maestro_settings::{SettingsError, SettingsManager};
use std::{path::Path, time::Instant};

impl<C: FnMut() -> u128> App<C> {
    /// Reloads settings, retaining rejected areas and reporting unrelated storage errors separately.
    pub async fn reload_config(
        &mut self,
        manager: &mut SettingsManager,
        agent_dir: &Path,
    ) -> (ConfigReloadReport, Vec<SettingsError>) {
        self.apply_config_from_disk(manager, agent_dir, true).await
    }

    /// Loads live preferences with optional success feedback for explicit reloads.
    pub async fn apply_config_from_disk(
        &mut self,
        manager: &mut SettingsManager,
        agent_dir: &Path,
        notify_success: bool,
    ) -> (ConfigReloadReport, Vec<SettingsError>) {
        self.config_reloaded_from_disk = true;
        let previous_toast = self.state.toast.clone();
        let (loaded, errors) =
            load_live_config(manager, &self.accepted_config_areas, agent_dir).await;
        let report = match loaded {
            Ok(loaded) => self.apply_live_config(loaded, notify_success),
            Err(diagnostics) => {
                self.state.toast = None;
                ConfigReloadReport {
                    status: ConfigReloadStatus::Failed,
                    diagnostics,
                }
            }
        };
        self.state.config_diagnostic = config_diagnostic_summary(&report.diagnostics);
        self.config_diagnostic_deadline = None;
        self.sync_toast_deadline(previous_toast.as_ref());
        (report, errors)
    }

    /// Consumes the disk-reload observation once for the loop's persistence policy.
    pub fn take_config_reloaded_from_disk(&mut self) -> bool {
        std::mem::take(&mut self.config_reloaded_from_disk)
    }

    /// Applies accepted areas without disturbing interaction state owned by other callers.
    fn apply_live_config(
        &mut self,
        mut loaded: LoadedConfig,
        notify_success: bool,
    ) -> ConfigReloadReport {
        if loaded.config.invalid_sidebar_bounds_diagnostic().is_some() {
            loaded.invalid_sections.push("ui".into());
        }
        if loaded
            .config
            .server
            .invalid_headless_size_diagnostic()
            .is_some()
        {
            loaded.invalid_sections.push("server".into());
        }
        let valid_ui = !loaded.invalid_sections.iter().any(|area| area == "ui");
        let valid_theme = !loaded.invalid_sections.iter().any(|area| area == "theme");
        let valid_experimental = !loaded
            .invalid_sections
            .iter()
            .any(|area| area == "experimental");
        let previous = self.config.clone();
        self.accepted_config_areas.accept(&loaded);
        let report = apply_loaded(&mut self.config, loaded);
        self.clear_history_after_reload |=
            valid_experimental && !self.config.experimental.pane_history.0;
        if valid_ui {
            if self.state.sidebar_width_source == super::state::SidebarWidthSource::ConfigDefault {
                self.state.sidebar_width = self.config.ui.sidebar_width;
            }
            self.state.sidebar_width = self.state.sidebar_width.clamp(
                self.config.ui.sidebar_min_width,
                self.config.ui.sidebar_max_width,
            );
            self.state.agent_panel_scroll = 0;
            self.state.request_client_config_reload.0 |= previous.ui.redraw_on_focus_gained
                != self.config.ui.redraw_on_focus_gained
                || previous.ui.host_cursor != self.config.ui.host_cursor
                || previous.ui.sound != self.config.ui.sound;
            self.state.redraw_on_focus_gained = self.config.ui.redraw_on_focus_gained;
            self.state.prompt_new_workspace_name = self.config.ui.prompt_new_workspace_name;
            self.state.confirm_close = self.config.ui.confirm_close;
            self.state.agent_panel_sort = self.config.ui.agent_panel_sort;
            self.state.sound = self.config.ui.sound.clone();
            self.state.toast_config = self.config.ui.toast.clone();
            let entries = self.config.ui.tab_bar_right.clone();
            let separator = self.config.ui.tab_bar_right_separator.clone();
            self.configure_tab_bar_status(&entries, &separator);
            self.configure_window_title(&self.config.ui.window_title.clone());
        }
        self.state.headless_size = self.config.headless_size();
        self.configure_update_checks(previous.update);
        if valid_theme {
            self.state.theme_runtime = ThemeRuntimeConfig::new(&self.config, valid_ui);
            self.refresh_effective_app_theme();
        }
        if notify_success {
            self.show_config_reloaded_toast(report.diagnostics.is_empty());
        }
        report
    }

    /// Shows explicit reload feedback independently of settings application.
    fn show_config_reloaded_toast(&mut self, without_diagnostics: bool) {
        self.state.toast = Some(ToastNotification {
            kind: ToastKind::UpdateInstalled,
            title: "reloaded config".into(),
            context: if without_diagnostics {
                "using settings.json"
            } else {
                "with warnings"
            }
            .into(),
            position: None,
            target: None,
        });
    }

    /// Cancels disabled checks and admits newly enabled background producers.
    fn configure_update_checks(&mut self, previous: crate::settings::model::UpdateConfig) {
        let now = Instant::now();
        let background = !self.launch.no_session && !cfg!(debug_assertions);
        if !self.config.update.version_check {
            self.next_auto_update_check = None;
        } else if !previous.version_check && background && self.state.update_available.is_none() {
            self.next_auto_update_check = Some(now);
        }
        if !self.config.update.manifest_check {
            self.next_agent_manifest_update_check = None;
        } else if !previous.manifest_check && background {
            self.next_agent_manifest_update_check = Some(now);
        }
    }
}
