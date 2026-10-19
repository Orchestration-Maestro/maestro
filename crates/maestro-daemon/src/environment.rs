//! Production environment names grouped by their actual launch or selection consumers.

/// A consumer of a supplied environment value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consumer {
    /// Injected into ordinary or workspace-action pane launches.
    Pane,
    /// Injected into a workspace action or its pane entrypoint.
    Action,
    /// Observed when selecting a daemon or socket endpoint.
    DaemonSelection,
}

/// One environment name with all of its runtime consumer classes.
pub struct EnvironmentName {
    /// Public process environment spelling.
    pub name: &'static str,
    /// Consumers that inject or observe this name.
    pub consumers: &'static [Consumer],
}

use Consumer::{Action, DaemonSelection, Pane};

/// Source-backed pane/action injection and daemon-selection vocabulary.
/// Runtime pane markers are injected only on Windows. Optional action context
/// fields are injected only when that context is available.
pub const NAMES: &[EnvironmentName] = &[
    EnvironmentName {
        name: "MAESTRO_ENV",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_SOCKET_PATH",
        consumers: &[Pane, Action, DaemonSelection],
    },
    EnvironmentName {
        name: "MAESTRO_BIN_PATH",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_WORKSPACE_ID",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_TAB_ID",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_PANE_ID",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_PANE_RUNTIME_ID",
        consumers: &[Pane],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_ROOT",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_CONFIG_DIR",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_STATE_DIR",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_MANIFEST_ID",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_ENTRYPOINT_ID",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_CONTEXT_JSON",
        consumers: &[Pane, Action],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_ID",
        consumers: &[Action],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_EVENT",
        consumers: &[Action],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_EVENT_JSON",
        consumers: &[Action],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_CLICKED_URL",
        consumers: &[Action],
    },
    EnvironmentName {
        name: "MAESTRO_ACTION_LINK_HANDLER_ID",
        consumers: &[Action],
    },
    EnvironmentName {
        name: crate::instances::SESSION_ENV_VAR,
        consumers: &[DaemonSelection],
    },
    EnvironmentName {
        name: "MAESTRO_CLIENT_SOCKET_PATH",
        consumers: &[DaemonSelection],
    },
];
