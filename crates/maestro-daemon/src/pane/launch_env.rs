//! Pane terminal identity and launch environment edits.

use maestro_process::pty::{PtyCommand, PtyEnvironment};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

/// Shell variable that tells fish to repaint its prompt after a resize.
const FISH_REFLOW: &str = "fish_handle_reflow";

/// Markers of the terminal or multiplexer around the daemon, which a pane child must not inherit.
const OUTER_TERMINAL_MARKERS: [&str; 12] = [
    "ITERM_SESSION_ID",
    "LC_TERMINAL",
    "LC_TERMINAL_VERSION",
    "WEZTERM_PANE",
    "KITTY_WINDOW_ID",
    "WT_SESSION",
    "TMUX",
    "TMUX_PANE",
    "STY",
    "ZELLIJ",
    "ZELLIJ_SESSION_NAME",
    "ZELLIJ_PANE_ID",
];

/// Extra user environment pairs and the pane identity policy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneLaunchEnv {
    /// Explicit user pairs applied after terminal defaults.
    extra: Vec<(String, String)>,
    /// Whether inherited identity is retained, replaced or removed.
    identity: PaneLaunchIdentity,
}

/// Identity edits after user environment pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum PaneLaunchIdentity {
    /// Retain the parent's identity values.
    #[default]
    Inherit,
    /// Supply this pane's managed identity.
    Managed {
        /// Owning workspace identifier.
        workspace_id: String,
        /// Owning tab identifier.
        tab_id: String,
        /// Pane identifier.
        pane_id: String,
    },
    /// Remove the pane identifier while retaining other inherited context.
    OmitPane,
}

impl PaneLaunchEnv {
    /// Supply user environment pairs while retaining inherited identity.
    #[must_use]
    pub fn from_extra(extra: Vec<(String, String)>) -> Self {
        Self {
            extra,
            identity: PaneLaunchIdentity::Inherit,
        }
    }

    /// Replace inherited workspace, tab and pane identifiers.
    #[must_use]
    pub fn with_identity(mut self, workspace_id: String, tab_id: String, pane_id: String) -> Self {
        self.identity = PaneLaunchIdentity::Managed {
            workspace_id,
            tab_id,
            pane_id,
        };
        self
    }

    /// Clear the inherited pane identifier after applying user pairs.
    #[must_use]
    pub fn without_pane_identity(mut self) -> Self {
        self.identity = PaneLaunchIdentity::OmitPane;
        self
    }

    /// Apply pane identity after terminal defaults and explicit user pairs.
    pub fn apply(&self, command: &mut PtyCommand, socket: &Path, executable: Option<&Path>) {
        self.apply_with(command, socket, executable, &process_variable);
    }

    /// Apply the pane policy, reading an inherited parent variable through `parent`.
    fn apply_with(
        &self,
        command: &mut PtyCommand,
        socket: &Path,
        executable: Option<&Path>,
        parent: &dyn Fn(&OsStr) -> Option<OsString>,
    ) {
        let mut edits = vec![
            ("TERM".into(), Some("xterm-256color".into())),
            ("COLORTERM".into(), Some("truecolor".into())),
            ("CODEX_THREAD_ID".into(), None),
            (
                maestro_tui::TERM_PROGRAM.into(),
                Some(maestro_tui::PANE_TERM_PROGRAM.into()),
            ),
            (
                "TERM_PROGRAM_VERSION".into(),
                Some(maestro_protocol::build_info::version().into()),
            ),
        ];
        edits.extend(OUTER_TERMINAL_MARKERS.map(|name| (name.into(), None)));
        if !holds(&command.environment, parent, FISH_REFLOW.as_ref()) {
            edits.push((FISH_REFLOW.into(), Some("0".into())));
        }
        edits.extend(
            self.extra
                .iter()
                .map(|(key, value)| (key.into(), Some(value.into()))),
        );
        edits.push(("MAESTRO_ENV".into(), Some("1".into())));
        edits.push((
            "MAESTRO_SOCKET_PATH".into(),
            Some(socket.as_os_str().to_owned()),
        ));
        if let Some(executable) = executable {
            edits.push((
                "MAESTRO_BIN_PATH".into(),
                Some(executable.as_os_str().to_owned()),
            ));
        }
        match &self.identity {
            PaneLaunchIdentity::Inherit => {}
            PaneLaunchIdentity::Managed {
                workspace_id,
                tab_id,
                pane_id,
            } => {
                edits.extend([
                    ("MAESTRO_WORKSPACE_ID".into(), Some(workspace_id.into())),
                    ("MAESTRO_TAB_ID".into(), Some(tab_id.into())),
                    ("MAESTRO_PANE_ID".into(), Some(pane_id.into())),
                ]);
            }
            PaneLaunchIdentity::OmitPane => edits.push(("MAESTRO_PANE_ID".into(), None)),
        }
        apply_edits(&mut command.environment, edits);
    }
}

/// Whether the pane's environment already holds `name`, matched with exact casing.
fn holds(
    environment: &PtyEnvironment,
    parent: &dyn Fn(&OsStr) -> Option<OsString>,
    name: &OsStr,
) -> bool {
    match environment {
        PtyEnvironment::Inherit(_) => parent(name).is_some(),
        PtyEnvironment::Replace(values) => values.iter().any(|(key, _)| key == name),
    }
}

/// Read a variable of this process by its exact name, whatever the platform's casing rule.
fn process_variable(name: &OsStr) -> Option<OsString> {
    std::env::vars_os()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}

/// Preserve the caller's inheritance/replacement choice while applying ordered edits.
fn apply_edits(environment: &mut PtyEnvironment, edits: Vec<(OsString, Option<OsString>)>) {
    match environment {
        PtyEnvironment::Inherit(existing) => existing.extend(edits),
        PtyEnvironment::Replace(values) => {
            for (key, value) in edits {
                values.retain(|(name, _)| name != &key);
                if let Some(value) = value {
                    values.push((key, value));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
