//! Session records and retained read-only host capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::types::object;
use crate::{AgentMessage, ExtensionResult, Presence, UserContent};
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
/// Identity and timestamp shared by session entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntryBase {
    /// Uninterpreted entry identity.
    pub id: String,
    /// Parent identity, or null for a root entry.
    pub parent_id: Option<String>,
    /// Uninterpreted entry timestamp.
    pub timestamp: String,
}
/// A conversation message stored as an entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "message", rename_all = "camelCase")]
pub struct SessionMessageEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Stored conversation message.
    pub message: AgentMessage,
}
/// An entry recording an open thinking-level string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename = "thinking_level_change",
    rename_all = "camelCase"
)]
pub struct ThinkingLevelChangeEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Authored thinking-level text.
    pub thinking_level: String,
}
/// An entry recording provider and model selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "model_change", rename_all = "camelCase")]
pub struct ModelChangeEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Provider identity.
    pub provider: String,
    /// Model identity.
    pub model_id: String,
}
/// A persisted compaction with identity and origin metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "compaction", rename_all = "camelCase")]
pub struct CompactionEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Summary text.
    pub summary: String,
    /// First entry kept after compaction.
    pub first_kept_entry_id: String,
    /// Context size before compaction.
    pub tokens_before: f64,
    /// Opaque JSON text, carried without parsing or reformatting.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<String>,
    /// Whether a hook supplied the entry.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub from_hook: Presence<bool>,
}
/// A branch summary with its source entry and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "branch_summary", rename_all = "camelCase")]
pub struct BranchSummaryEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Entry from which the branch was summarized.
    pub from_id: String,
    /// Summary text.
    pub summary: String,
    /// Opaque JSON text, carried without parsing or reformatting.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<String>,
    /// Whether a hook supplied the entry.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub from_hook: Presence<bool>,
}
/// Extension-defined state stored as an entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "custom", rename_all = "camelCase")]
pub struct CustomEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Extension-defined entry kind.
    pub custom_type: String,
    /// Opaque JSON text, carried without parsing or reformatting.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub data: Presence<String>,
}
/// An extension-authored display message entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "custom_message", rename_all = "camelCase")]
pub struct CustomMessageEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Extension-defined entry kind.
    pub custom_type: String,
    /// Display message content.
    pub content: UserContent,
    /// Opaque JSON text, carried without parsing or reformatting.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<String>,
    /// Whether to display the message.
    pub display: bool,
}
/// A label change targeting an entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "label", rename_all = "camelCase")]
pub struct LabelEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Entry whose label changes.
    pub target_id: String,
    /// Supplied label, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub label: Presence<String>,
}
/// Session display-name metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "session_info", rename_all = "camelCase")]
pub struct SessionInfoEntry {
    /// Entry identity and timestamp.
    #[serde(flatten)]
    pub base: SessionEntryBase,
    /// Supplied session display name, null or omitted.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub name: Presence<String>,
}
/// The nine tagged session-entry variants.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SessionEntry {
    /// A stored conversation message.
    Message(Box<SessionMessageEntry>),
    /// An entry recording an open thinking-level string.
    ThinkingLevelChange(ThinkingLevelChangeEntry),
    /// An entry recording provider and model selection.
    ModelChange(ModelChangeEntry),
    /// A persisted compaction with identity and origin metadata.
    Compaction(CompactionEntry),
    /// A branch summary with its source entry and metadata.
    BranchSummary(BranchSummaryEntry),
    /// Extension-defined state stored as an entry.
    Custom(CustomEntry),
    /// An extension-authored display message entry.
    CustomMessage(CustomMessageEntry),
    /// A label change targeting an entry.
    Label(LabelEntry),
    /// Session display-name metadata.
    SessionInfo(SessionInfoEntry),
}
impl SessionEntry {
    /// Read the discriminator, then the selected record from its original JSON.
    fn decode(text: &str) -> Result<Self, serde_json::Error> {
        /// The entry discriminator selected before decoding its fields.
        #[derive(Deserialize)]
        struct Tag {
            /// Serialized entry kind.
            #[serde(rename = "type")]
            kind: String,
        }
        let Tag { kind } = object::from_str(text)?;
        Ok(match kind.as_str() {
            "message" => Self::Message(object::from_str(text)?),
            "thinking_level_change" => Self::ThinkingLevelChange(object::from_str(text)?),
            "model_change" => Self::ModelChange(object::from_str(text)?),
            "compaction" => Self::Compaction(object::from_str(text)?),
            "branch_summary" => Self::BranchSummary(object::from_str(text)?),
            "custom" => Self::Custom(object::from_str(text)?),
            "custom_message" => Self::CustomMessage(object::from_str(text)?),
            "label" => Self::Label(object::from_str(text)?),
            "session_info" => Self::SessionInfo(object::from_str(text)?),
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "unknown session entry type: {kind}"
                )));
            }
        })
    }
}
impl<'de> Deserialize<'de> for SessionEntry {
    /// Preserve raw nested-message input for the selected record decoder.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;
        Self::decode(raw.get()).map_err(D::Error::custom)
    }
}

/// The header selected by the host session reader.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename = "session", rename_all = "camelCase")]
pub struct SessionHeader {
    /// Optional format version.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub version: Presence<f64>,
    /// Session identity.
    pub id: String,
    /// Uninterpreted creation timestamp.
    pub timestamp: String,
    /// Authored working directory.
    pub cwd: String,
    /// Optional parent session file.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub parent_session: Presence<String>,
}
/// Retained read-only host session capability. Clones share the same owner.
/// Host failures and selected-record decoding errors are returned to the caller.
///
/// ```compile_fail,E0599
/// fn readonly_session_has_no_append_message(
///     reader: maestro_extensions_wasm::ReadonlySessionManager,
///     message: maestro_extensions_wasm::AgentMessage,
/// ) {
///     reader.append_message(message);
/// }
/// ```
#[derive(Clone)]
pub struct ReadonlySessionManager(Rc<dyn SessionReaderPort>);
impl ReadonlySessionManager {
    /// Retains the supplied reader port.
    #[must_use]
    pub fn new(port: Rc<dyn SessionReaderPort>) -> Self {
        Self(port)
    }
}
/// An independently retained node of a host-produced tree.
#[derive(Clone)]
pub struct SessionTreeNode(Rc<dyn SessionTreeNodePort>);
impl SessionTreeNode {
    /// Retains the supplied node port.
    #[must_use]
    pub fn new(port: Rc<dyn SessionTreeNodePort>) -> Self {
        Self(port)
    }
}
port! {
    /// Fallible queries of a retained host capability.
    SessionReaderPort for ReadonlySessionManager via 0 {
        /// The current host working directory.
        fn get_cwd() -> String;
        /// The host session directory.
        fn get_session_dir() -> String;
        /// The host session identity.
        fn get_session_id() -> String;
        /// The host session file, when supplied.
        fn get_session_file() -> Option<String>;
        /// The selected leaf identity.
        fn get_leaf_id() -> Option<String>;
        /// The selected leaf entry.
        fn get_leaf_entry() -> Option<SessionEntry>;
        /// The entry for the literal identity.
        fn get_entry(id: &str) -> Option<SessionEntry>;
        /// The label for the literal identity.
        fn get_label(id: &str) -> Option<String>;
        /// The supplied branch, in host order.
        fn get_branch(from_id: Option<&str>) -> Vec<SessionEntry>;
        /// The selected host header.
        fn get_header() -> Option<SessionHeader>;
        /// The supplied entries, in host order.
        fn get_entries() -> Vec<SessionEntry>;
        /// The supplied roots as owned node handles.
        fn get_tree() -> Vec<SessionTreeNode>;
        /// The host-resolved session name.
        fn get_session_name() -> Option<String>;
    }
}
port! {
    /// Fallible queries of a retained host capability.
    SessionTreeNodePort for SessionTreeNode via 0 {
        /// The entry retained by this node.
        fn entry() -> SessionEntry;
        /// The supplied children as owned node handles.
        fn children() -> Vec<SessionTreeNode>;
        /// The label retained by this node.
        fn label() -> Option<String>;
        /// The label timestamp retained by this node.
        fn label_timestamp() -> Option<String>;
    }
}

/// Plain parameters of a setup-created session.
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional caller-supplied session identity.
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional literal parent-session path.
    pub parent_session: Option<String>,
}
/// Selected model identity in a built context.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextModel {
    /// Provider identity chosen by the host.
    pub provider: String,
    /// Model identity chosen by the host.
    pub model_id: String,
}
/// Host-built conversation context.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContext {
    /// Messages supplied by the host projection.
    pub messages: Vec<AgentMessage>,
    /// Open thinking-level string supplied by the host.
    pub thinking_level: String,
    /// Optional selected model identity.
    pub model: Option<SessionContextModel>,
}
/// Writable setup capability, retaining its read view over the same session.
#[derive(Clone)]
pub struct SessionManager {
    /// Retained writable host capability.
    port: Rc<dyn SessionWriterPort>,
    /// Retained reader over the same host session.
    reader: ReadonlySessionManager,
}
impl SessionManager {
    /// Retains the writer and its read view over the same host session.
    pub fn new(port: Rc<dyn SessionWriterPort>) -> Self {
        let reader = port.reader();
        Self { port, reader }
    }
}
impl std::ops::Deref for SessionManager {
    type Target = ReadonlySessionManager;
    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}
port! {
 /// Setup-only writes to a supplied session.
 SessionWriterPort for SessionManager via port {
 /// Forwards set session file to the host.
 fn set_session_file(path: &str) -> ();
 /// Forwards new session to the host.
 fn new_session(options: Option<NewSessionOptions>) -> Option<String>;
 /// Forwards is persisted to the host.
 fn is_persisted() -> bool;
 /// Forwards get children to the host.
 fn get_children(parent_id: &str) -> Vec<SessionEntry>;
 /// Forwards build session context to the host.
 fn build_session_context() -> SessionContext;
 /// Forwards append message to the host.
 fn append_message(message: AgentMessage) -> String;
 /// Forwards append thinking level change to the host.
 fn append_thinking_level_change(level: &str) -> String;
 /// Forwards append model change to the host.
 fn append_model_change(provider: &str, model_id: &str) -> String;
 /// Forwards append compaction to the host.
 fn append_compaction(compaction: crate::CompactionResult, from_hook: Option<bool>) -> String;
 /// Forwards append custom entry to the host.
 fn append_custom_entry(custom_type: &str, data: Option<serde_json::Value>) -> String;
 /// Forwards append session info to the host.
 fn append_session_info(name: &str) -> String;
 /// Forwards append custom message entry to the host.
 fn append_custom_message_entry(message: crate::CustomMessageInput) -> String;
 /// Forwards append label change to the host.
 fn append_label_change(target_id: &str, label: Option<&str>) -> String;
 /// Forwards branch to the host.
 fn branch(from_id: &str) -> ();
 /// Forwards reset leaf to the host.
 fn reset_leaf() -> ();
 /// Forwards branch with summary to the host.
 fn branch_with_summary(from_id: Option<&str>, summary: &str, details: Option<serde_json::Value>, from_hook: Option<bool>) -> String;
 /// Forwards create branched session to the host.
 fn create_branched_session(leaf_id: &str) -> Option<String>;
 }
 infallible { /// Acquires a read view bound to the same session.
 fn reader() -> ReadonlySessionManager; }
}
