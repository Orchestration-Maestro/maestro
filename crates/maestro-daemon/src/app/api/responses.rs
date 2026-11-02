//! Failure records shared by the workspace command handlers.

use maestro_protocol::schema::ErrorBody;

/// A command failure with its machine-readable `code` and readable `message`.
pub(super) fn api_error(code: &str, message: impl Into<String>) -> ErrorBody {
    ErrorBody {
        code: code.into(),
        message: message.into(),
    }
}

/// The failure for a workspace target that names no live workspace.
pub(super) fn workspace_not_found(workspace_id: &str) -> ErrorBody {
    api_error(
        "workspace_not_found",
        format!("workspace {workspace_id} not found"),
    )
}

/// The failure for a tab target that names no live tab.
pub(super) fn tab_not_found(tab_id: &str) -> ErrorBody {
    api_error("tab_not_found", format!("tab {tab_id} not found"))
}

/// The failure for an explicit pane target that names no live pane.
pub(super) fn pane_not_found(pane_id: &str) -> ErrorBody {
    api_error("pane_not_found", format!("pane {pane_id} not found"))
}

/// The failure for a pane that cannot be resolved because no target names one.
pub(super) fn pane_unresolved() -> ErrorBody {
    api_error("pane_not_found", "pane not found")
}
