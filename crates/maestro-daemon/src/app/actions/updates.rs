//! Retained application update availability and installation guidance.

use crate::{
    app::state::{DaemonState, ToastKind, ToastNotification},
    settings::model::ToastDelivery,
};

impl<C: FnMut() -> u128> DaemonState<C> {
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
