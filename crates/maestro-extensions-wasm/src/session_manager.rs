//! Session records and retained read-only host capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::types::object;
use crate::{AgentMessage, ExtensionResult};
pub use entry::{
    LabelEntry, ModelChangeEntry, SessionEntryBase, SessionHeader, SessionInfoEntry,
    ThinkingLevelChangeEntry,
};
use maestro_entry::session_manager as entry;
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
/// Opaque JSON payload text that serializes as its semantic JSON value.
///
/// Reads use compact `serde_json` text, preserving object key order and numbers.
/// Constructed text is validated when serialized; malformed JSON returns an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonText(String);

impl std::ops::Deref for JsonText {
    type Target = str;

    /// Borrows the stored JSON text without reparsing it.
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for JsonText {
    /// Borrows the stored JSON text.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for JsonText {
    /// Writes the stored JSON text unchanged.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<String> for JsonText {
    /// Retains authored text, validating it at the serialization boundary.
    fn from(text: String) -> Self {
        Self(text)
    }
}

impl From<JsonText> for String {
    /// Returns the stored text without another allocation.
    fn from(text: JsonText) -> Self {
        text.0
    }
}

impl Serialize for JsonText {
    /// Restores the semantic JSON value instead of quoting its encoded text.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        let value: serde_json::Value = serde_json::from_str(&self.0).map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for JsonText {
    /// Captures arbitrary JSON as compact text rather than requiring a JSON string.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        serde_json::to_string(&value)
            .map(Self)
            .map_err(D::Error::custom)
    }
}

/// Message entry holding a typed agent message.
pub type SessionMessageEntry = entry::SessionMessageEntry<AgentMessage>;
/// A persisted compaction whose details are opaque JSON text.
pub type CompactionEntry = entry::CompactionEntry<JsonText>;
/// A branch summary whose details are opaque JSON text.
pub type BranchSummaryEntry = entry::BranchSummaryEntry<JsonText>;
/// Extension state whose data is opaque JSON text.
pub type CustomEntry = entry::CustomEntry<JsonText>;
/// An extension display message whose details are opaque JSON text.
pub type CustomMessageEntry = entry::CustomMessageEntry<JsonText>;
/// Plain parameters of a setup-created session.
pub type NewSessionOptions = entry::NewSessionOptions<Option<String>>;
/// Selected model identity in a built context.
pub type SessionContextModel = entry::SessionContextModel<String>;
/// Host-built conversation context.
pub type SessionContext = entry::SessionContext<AgentMessage, String>;
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

#[cfg(test)]
mod tests;
