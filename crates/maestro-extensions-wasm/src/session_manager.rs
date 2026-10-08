//! Session access: the read-only view, the setup writer and the tree nodes.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::ops::Deref;
use std::rc::Rc;

use serde_json::Value;

use crate::agent::AgentMessage;
pub use crate::bindings::maestro::extension::session::{
    BranchSummaryEntry, CompactionEntry, CustomEntry, CustomMessageEntry, FileEntry, LabelEntry,
    ModelChangeEntry, ModelReference, NewSessionOptions, SessionContext, SessionEntry,
    SessionEntryBase, SessionHeader, SessionInfoEntry, SessionMessageEntry,
    ThinkingLevelChangeEntry,
};
use crate::compaction::CompactionResult;
use crate::messages::CustomMessageInput;
use crate::types::ExtensionResult;

port! {
    /// Read-only session capability behind a [`ReadonlySessionManager`].
    SessionReaderPort for ReadonlySessionManager via 0 {
        /// The working directory of the session.
        fn get_cwd() -> String;
        /// The directory session files live in.
        fn get_session_dir() -> String;
        /// The identifier of the session.
        fn get_session_id() -> String;
        /// The path of the session file, when the session is persisted.
        fn get_session_file() -> Option<String>;
        /// The identifier of the current leaf entry.
        fn get_leaf_id() -> Option<String>;
        /// The current leaf entry.
        fn get_leaf_entry() -> Option<SessionEntry>;
        /// The entry with the given identifier.
        fn get_entry(id: &str) -> Option<SessionEntry>;
        /// The label of an entry.
        fn get_label(id: &str) -> Option<String>;
        /// The entries from `from_id` (or the current leaf) back to the root.
        fn get_branch(from_id: Option<&str>) -> Vec<SessionEntry>;
        /// The header of the session file.
        fn get_header() -> Option<SessionHeader>;
        /// Every entry of the session.
        fn get_entries() -> Vec<SessionEntry>;
        /// The roots of the session tree.
        fn get_tree() -> Vec<SessionTreeNode>;
        /// The display name of the session.
        fn get_session_name() -> Option<String>;
    }
}

/// Read-only view of a session. It has no write operations:
///
/// ```compile_fail,E0599
/// fn read(session: maestro_extensions_wasm::ReadonlySessionManager) {
///     let _ = session.append_message();
/// }
/// ```
#[derive(Clone)]
pub struct ReadonlySessionManager(Rc<dyn SessionReaderPort>);

impl ReadonlySessionManager {
    /// Wraps the host's read-only session.
    #[must_use]
    pub fn new(port: Rc<dyn SessionReaderPort>) -> Self {
        Self(port)
    }
}

port! {
    /// Capability behind a [`SessionTreeNode`].
    SessionTreeNodePort for SessionTreeNode via 0 {
        /// The entry of the node.
        fn entry() -> SessionEntry;
        /// The children of the node, oldest first.
        fn children() -> Vec<SessionTreeNode>;
        /// The resolved label of the entry.
        fn label() -> Option<String>;
        /// The timestamp of the latest label change of the entry.
        fn label_timestamp() -> Option<String>;
    }
}

/// A node of the session tree.
#[derive(Clone)]
pub struct SessionTreeNode(Rc<dyn SessionTreeNodePort>);

impl SessionTreeNode {
    /// Wraps the host's node.
    #[must_use]
    pub fn new(port: Rc<dyn SessionTreeNodePort>) -> Self {
        Self(port)
    }
}

port! {
    /// Write capability behind a [`SessionManager`]; reads come from its supertrait.
    SessionWriterPort: SessionReaderPort for SessionManager via port {
        /// Points the session at another file.
        fn set_session_file(path: &str) -> ();
        /// Starts a new session and returns the file it was assigned, when it has one.
        fn new_session(options: Option<NewSessionOptions>) -> Option<String>;
        /// Whether the session is stored in a file.
        fn is_persisted() -> bool;
        /// The children of an entry.
        fn get_children(parent_id: &str) -> Vec<SessionEntry>;
        /// The conversation the session builds for the model.
        fn build_session_context() -> SessionContext;
        /// Appends a message and returns the identifier of its entry.
        fn append_message(message: AgentMessage) -> String;
        /// Appends a thinking level change and returns the identifier of its entry.
        fn append_thinking_level_change(level: &str) -> String;
        /// Appends a model change and returns the identifier of its entry.
        fn append_model_change(provider: &str, model_id: &str) -> String;
        /// Appends a compaction and returns the identifier of its entry.
        fn append_compaction(compaction: CompactionResult, from_hook: Option<bool>) -> String;
        /// Appends extension state and returns the identifier of its entry.
        fn append_custom_entry(custom_type: &str, data: Option<Value>) -> String;
        /// Appends the display name and returns the identifier of its entry.
        fn append_session_info(name: &str) -> String;
        /// Appends a message for the model context and returns the identifier of its entry.
        fn append_custom_message_entry(message: CustomMessageInput) -> String;
        /// Appends a label change and returns the identifier of its entry.
        fn append_label_change(target_id: &str, label: Option<&str>) -> String;
        /// Moves the leaf to an earlier entry.
        fn branch(from_id: &str) -> ();
        /// Moves the leaf before the first entry.
        fn reset_leaf() -> ();
        /// Appends a branch summary and returns the identifier of its entry.
        fn branch_with_summary(
            from_id: Option<&str>,
            summary: &str,
            details: Option<Value>,
            from_hook: Option<bool>,
        ) -> String;
        /// Copies the branch ending at a leaf into a new session file.
        fn create_branched_session(leaf_id: &str) -> Option<String>;
    }
}

/// Write access to a session being set up, plus the read-only view of the same session.
pub struct SessionManager {
    /// The host's write capability.
    port: Rc<dyn SessionWriterPort>,
    /// The read-only view over the same capability.
    reader: ReadonlySessionManager,
}

impl SessionManager {
    /// Wraps the host's session writer.
    #[must_use]
    pub fn new(port: Rc<dyn SessionWriterPort>) -> Self {
        let reader = ReadonlySessionManager::new(Rc::clone(&port) as Rc<dyn SessionReaderPort>);
        Self { port, reader }
    }
}

impl Deref for SessionManager {
    type Target = ReadonlySessionManager;

    fn deref(&self) -> &ReadonlySessionManager {
        &self.reader
    }
}
