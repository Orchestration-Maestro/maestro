//! Retained agent-notification decisions and delivery data.

use maestro_detection::agents::{Agent, AgentState};
use maestro_protocol::{schema::NotificationShowSound, shared::ToastMaestroPosition};
use maestro_workspace::layout::PaneId;
use std::time::Instant;

/// Agent transition presented by a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    /// Agent needs user input.
    NeedsAttention,
    /// Agent has finished its work.
    Finished,
    /// An application version or detection update is available.
    UpdateInstalled,
}

/// Logical target retained by a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastTarget {
    /// Owning workspace identity.
    pub workspace_id: String,
    /// Owning pane identity.
    pub pane_id: PaneId,
}

/// Notification content for a workspace or attached client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastNotification {
    /// Transition being presented.
    pub kind: ToastKind,
    /// Agent label and event text.
    pub title: String,
    /// Resolved workspace and optional tab label.
    pub context: String,
    /// Explicit placement, absent for the configured default.
    pub position: Option<ToastMaestroPosition>,
    /// Optional logical navigation target.
    pub target: Option<ToastTarget>,
}

/// Delayed transition awaiting live-state validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAgentNotification {
    /// Pane whose transition is pending.
    pub pane_id: PaneId,
    /// Stable workspace identity.
    pub workspace_id: String,
    /// Expected effective agent label.
    pub agent_label: String,
    /// Known agent used for sound preferences.
    pub known_agent: Option<Agent>,
    /// Transition being presented.
    pub kind: ToastKind,
    /// Expected effective state at delivery.
    pub state: AgentState,
    /// Earliest delivery time.
    pub deadline: Instant,
}

/// Independently selected workspace, client and sound outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentNotificationDelivery {
    /// Pane producing the transition.
    pub pane_id: PaneId,
    /// Stable workspace identity.
    pub workspace_id: String,
    /// Effective agent label.
    pub agent_label: String,
    /// Known agent used for sound preferences.
    pub known_agent: Option<Agent>,
    /// Transition being presented.
    pub kind: ToastKind,
    /// Workspace notification, only outside the active tab.
    pub toast: Option<ToastNotification>,
    /// Client notification, subject to outer terminal focus.
    pub client_notification: Option<ToastNotification>,
    /// Selected sound for the effects owner to play.
    pub sound: Option<NotificationShowSound>,
}
