//! Workspace preference saves through the settings owner's captured write queue.

use super::App;
use crate::settings::model::{
    AgentPanelSortConfig, ConfigReloadReport, ConfigReloadStatus, StatusIndicatorStyle,
    ToastDelivery,
};
use maestro_settings::{SettingsError, SettingsErrorKind, SettingsManager, SettingsScope};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};

impl<C: FnMut() -> u128> App<C> {
    /// Saves notification delivery and removes its superseded enabled key in one write.
    pub async fn save_toast_delivery(
        &mut self,
        manager: &mut SettingsManager,
        agent_dir: &Path,
        delivery: ToastDelivery,
    ) -> (ConfigReloadReport, Vec<SettingsError>) {
        self.update_config_file(
            manager,
            agent_dir,
            "toast setting",
            &[
                (&["toast", "delivery"], Some(json!(delivery))),
                (&["toast", "enabled"], None),
            ],
        )
        .await
    }

    /// Saves status indicator style before applying accepted live preferences.
    pub async fn save_status_indicators(
        &mut self,
        manager: &mut SettingsManager,
        agent_dir: &Path,
        style: StatusIndicatorStyle,
    ) -> (ConfigReloadReport, Vec<SettingsError>) {
        self.update_config_file(
            manager,
            agent_dir,
            "status indicators",
            &[(&["statusIndicators"], Some(json!(style.as_str())))],
        )
        .await
    }

    /// Saves agent-panel ordering before applying accepted live preferences.
    pub async fn save_agent_panel_sort(
        &mut self,
        manager: &mut SettingsManager,
        agent_dir: &Path,
        sort: AgentPanelSortConfig,
    ) -> (ConfigReloadReport, Vec<SettingsError>) {
        self.update_config_file(
            manager,
            agent_dir,
            "agent panel sort",
            &[(&["agentPanelSort"], Some(json!(sort.as_str())))],
        )
        .await
    }

    /// Saves a manual theme and disables automatic switching in the same write.
    pub async fn save_theme(
        &mut self,
        manager: &mut SettingsManager,
        agent_dir: &Path,
        name: &str,
    ) -> (ConfigReloadReport, Vec<SettingsError>) {
        self.update_config_file(
            manager,
            agent_dir,
            "theme",
            &[
                (&["theme", "name"], Some(json!(name))),
                (&["theme", "autoSwitch"], Some(json!(false))),
            ],
        )
        .await
    }

    /// Saves sound admission before applying accepted live preferences.
    pub async fn save_sound(
        &mut self,
        manager: &mut SettingsManager,
        agent_dir: &Path,
        enabled: bool,
    ) -> (ConfigReloadReport, Vec<SettingsError>) {
        self.update_config_file(
            manager,
            agent_dir,
            "sound setting",
            &[(&["sound", "enabled"], Some(json!(enabled)))],
        )
        .await
    }

    /// Saves pane-border agent label visibility without replacing other interface preferences.
    pub async fn save_agent_border_labels(
        &mut self,
        manager: &mut SettingsManager,
        agent_dir: &Path,
        enabled: bool,
    ) -> (ConfigReloadReport, Vec<SettingsError>) {
        self.update_config_file(
            manager,
            agent_dir,
            "agent border labels",
            &[(&["showAgentLabelsOnPaneBorders"], Some(json!(enabled)))],
        )
        .await
    }

    /// Records completion of initial workspace setup in global preferences.
    pub async fn mark_onboarding_complete(
        &mut self,
        manager: &mut SettingsManager,
        agent_dir: &Path,
    ) -> (ConfigReloadReport, Vec<SettingsError>) {
        self.update_config_file(
            manager,
            agent_dir,
            "onboarding setting",
            &[(&["onboarding"], Some(json!(false)))],
        )
        .await
    }

    /// Flushes one atomic leaf batch, applying it live only after a successful global write.
    async fn update_config_file(
        &mut self,
        manager: &mut SettingsManager,
        agent_dir: &Path,
        context: &str,
        edits: &[(&[&str], Option<Value>)],
    ) -> (ConfigReloadReport, Vec<SettingsError>) {
        manager.set_global_key_paths("multiplexer", edits);
        manager.flush().await;
        let mut errors = manager.drain_errors();
        if let Some(error) = errors.iter().find(|error| {
            error.scope == SettingsScope::Global && error.kind == SettingsErrorKind::Failure
        }) {
            tracing::warn!(%error.error, context, "failed to save workspace preferences");
            let diagnostic = format!("failed to save {context}: {}", error.error);
            self.state.config_diagnostic = Some(diagnostic.clone());
            self.config_diagnostic_deadline = Some(Instant::now() + Duration::from_secs(5));
            return (
                ConfigReloadReport {
                    status: ConfigReloadStatus::Failed,
                    diagnostics: vec![diagnostic],
                },
                errors,
            );
        }
        let (report, reload_errors) = self.apply_config_from_disk(manager, agent_dir, false).await;
        errors.extend(reload_errors);
        (report, errors)
    }
}
