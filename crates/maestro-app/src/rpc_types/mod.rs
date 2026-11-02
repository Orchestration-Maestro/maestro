//! The JSONL protocol records of a session: commands, responses, state, invocable commands
//! and extension interface requests, sharing one `type` tag with the workspace commands.
mod commands;
mod responses;

pub(crate) use commands::names;
pub use commands::{
    RpcCommand, RpcCommandBody, RpcCommandRejection, RpcSessionCommand, parse_rpc_command,
};
pub use responses::{
    RpcAvailableModels, RpcCancelled, RpcCommandList, RpcData, RpcExportPath, RpcForkMessages,
    RpcForkResult, RpcLastAssistantText, RpcMessages, RpcResponse, RpcThinkingLevel,
};

/// The workspace command family, its correlated responses and the shared envelope writer.
pub use maestro_protocol::rpc_types as workspace;
/// The workspace payload records: results, failures and events.
pub use maestro_protocol::schema as workspace_schema;

use maestro_extensions::{WidgetPlacement, types::NotificationType};
use maestro_models::{Model, ModelThinkingLevel};
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::{Arc, RwLock};

/// The state of a session.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcSessionState {
    /// The selected model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<Arc<RwLock<Model>>>,
    /// The effective thinking level.
    pub thinking_level: ModelThinkingLevel,
    /// Whether a response is streaming.
    pub is_streaming: bool,
    /// Whether a compaction is running.
    pub is_compacting: bool,
    /// How queued steering messages are delivered.
    pub steering_mode: maestro_agent::QueueMode,
    /// How queued follow-up messages are delivered.
    pub follow_up_mode: maestro_agent::QueueMode,
    /// The session file, absent in memory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_file: Option<String>,
    /// The session identity.
    pub session_id: String,
    /// The session name, when one is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    /// Whether automatic compaction is on.
    pub auto_compaction_enabled: bool,
    /// The messages in the session.
    pub message_count: usize,
    /// The queued steering and follow-up messages.
    pub pending_message_count: usize,
}

/// A command a prompt can invoke.
pub type RpcSlashCommand = maestro_resources::SlashCommandInfo;

/// An extension interface request: a dialog that waits for its response, or a notice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum RpcExtensionUIMethod {
    /// Choose one option.
    #[serde(rename = "select")]
    Select {
        /// The dialog title.
        title: String,
        /// The options.
        options: Vec<String>,
        /// Milliseconds before the dialog gives up.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<f64>,
    },
    /// Confirm or decline.
    #[serde(rename = "confirm")]
    Confirm {
        /// The dialog title.
        title: String,
        /// The question.
        message: String,
        /// Milliseconds before the dialog gives up.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<f64>,
    },
    /// Enter one line of text.
    #[serde(rename = "input")]
    Input {
        /// The dialog title.
        title: String,
        /// The hint shown while empty.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
        /// Milliseconds before the dialog gives up.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<f64>,
    },
    /// Edit multi-line text.
    #[serde(rename = "editor")]
    Editor {
        /// The dialog title.
        title: String,
        /// The initial text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prefill: Option<String>,
    },
    /// Show a notification.
    #[serde(rename = "notify", rename_all = "camelCase")]
    Notify {
        /// The notification text.
        message: String,
        /// Its severity.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notify_type: Option<NotificationType>,
    },
    /// Set or clear a status text.
    #[serde(rename = "setStatus", rename_all = "camelCase")]
    SetStatus {
        /// The status key.
        status_key: String,
        /// The text; absent clears it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status_text: Option<String>,
    },
    /// Set or clear a widget.
    #[serde(rename = "setWidget", rename_all = "camelCase")]
    SetWidget {
        /// The widget key.
        widget_key: String,
        /// The lines; absent clears the widget.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        widget_lines: Option<Vec<String>>,
        /// Where the widget goes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        widget_placement: Option<WidgetPlacement>,
    },
    /// Set the window title.
    #[serde(rename = "setTitle")]
    SetTitle {
        /// The title.
        title: String,
    },
    /// Replace the editor text.
    #[serde(rename = "set_editor_text")]
    SetEditorText {
        /// The text.
        text: String,
    },
}

/// An extension interface request with its fresh identifier.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename = "extension_ui_request")]
pub struct RpcExtensionUIRequest {
    /// The identifier a response repeats.
    pub id: String,
    /// The request.
    #[serde(flatten)]
    pub method: RpcExtensionUIMethod,
}

/// The client's answer to a dialog. A present member has its declared kind; `null` is not
/// absence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename = "extension_ui_response")]
pub struct RpcExtensionUIResponse {
    /// The identifier of the answered request.
    pub id: String,
    /// The chosen or entered text.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub value: Option<String>,
    /// Whether a confirmation was accepted.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub confirmed: Option<bool>,
    /// Whether the dialog was dismissed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub cancelled: Option<bool>,
}

/// Decodes a present member, refusing `null`; an absent one takes the default.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    decoder: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(decoder).map(Some)
}
