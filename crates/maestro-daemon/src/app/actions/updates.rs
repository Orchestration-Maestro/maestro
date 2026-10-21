//! Retained application update availability and installation guidance.

use crate::{
    app::state::{DaemonState, ToastKind, ToastNotification},
    settings::model::ToastDelivery,
};

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Refreshes detection diagnostics and summaries from the runtime's shared snapshot.
    pub(super) fn record_manifest_update(
        &mut self,
        updated: &[maestro_detection::manifest_update::ManifestUpdateCommit],
        status: maestro_detection::manifest_update::ManifestUpdateStatus,
    ) {
        self.agent_manifest_update_status = status;
        self.agent_manifest_summaries = self
            .manifests
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .manifest_summaries();
        if !updated.is_empty() && self.toast_config.delivery == ToastDelivery::Workspace {
            let context = updated
                .iter()
                .map(|item| {
                    format!(
                        "{} {}",
                        maestro_detection::agents::agent_label(item.agent),
                        item.version
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            self.toast = Some(ToastNotification {
                kind: ToastKind::UpdateInstalled,
                title: "Agent detection rules updated".into(),
                context,
                position: None,
                target: None,
            });
        }
    }

    /// Records update availability independently of workspace notification delivery.
    pub(super) fn record_update_ready(&mut self, version: String, install_command: String) {
        if self.toast_config.delivery == ToastDelivery::Workspace {
            self.toast = Some(ToastNotification {
                kind: ToastKind::UpdateInstalled,
                title: format!("v{version} available"),
                context: update_install_instruction(&install_command),
                position: None,
                target: None,
            });
        }
        self.update_available = Some(version);
        self.update_install_command = install_command;
        self.latest_release_notes_available = true;
        self.update_dismissed = true;
    }
}

/// Formats the install command without conflating installation with restart.
fn update_install_instruction(command: &str) -> String {
    match command {
        "maestro update" => "detach, run `maestro update`, then follow its restart guidance".into(),
        "update through Nix" => {
            "detach, update through Nix, then restart this Maestro session when ready".into()
        }
        command => format!("detach, run `{command}`, then restart this Maestro session when ready"),
    }
}
