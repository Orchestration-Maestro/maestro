//! Retained reader and node resources behind the author ports.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::Imports;
use crate::{
    ExtensionResult, ReadonlySessionManager, SessionEntry, SessionHeader, SessionReaderPort,
    SessionTreeNode, SessionTreeNodePort,
};
use std::rc::Rc;
/// Owns one host reader independently of the context that supplied it.
struct Reader<I: Imports> {
    /// Host imports.
    imports: I,
    /// Owned reader handle.
    resource: I::Reader,
}
/// Retains an acquired host reader.
pub(super) fn reader<I: Imports>(imports: I, resource: I::Reader) -> ReadonlySessionManager {
    ReadonlySessionManager::new(Rc::new(Reader { imports, resource }))
}
impl<I: Imports> SessionReaderPort for Reader<I> {
    fn get_cwd(&self) -> ExtensionResult<String> {
        self.imports.reader_get_cwd(&self.resource)
    }
    fn get_session_dir(&self) -> ExtensionResult<String> {
        self.imports.reader_get_session_dir(&self.resource)
    }
    fn get_session_id(&self) -> ExtensionResult<String> {
        self.imports.reader_get_session_id(&self.resource)
    }
    fn get_session_file(&self) -> ExtensionResult<Option<String>> {
        self.imports.reader_get_session_file(&self.resource)
    }
    fn get_leaf_id(&self) -> ExtensionResult<Option<String>> {
        self.imports.reader_get_leaf_id(&self.resource)
    }
    fn get_leaf_entry(&self) -> ExtensionResult<Option<SessionEntry>> {
        self.imports
            .reader_get_leaf_entry(&self.resource)?
            .as_deref()
            .map(decode)
            .transpose()
    }
    fn get_entry(&self, id: &str) -> ExtensionResult<Option<SessionEntry>> {
        self.imports
            .reader_get_entry(&self.resource, id)?
            .as_deref()
            .map(decode)
            .transpose()
    }
    fn get_label(&self, id: &str) -> ExtensionResult<Option<String>> {
        self.imports.reader_get_label(&self.resource, id)
    }
    fn get_branch(&self, from_id: Option<&str>) -> ExtensionResult<Vec<SessionEntry>> {
        decode(&self.imports.reader_get_branch(&self.resource, from_id)?)
    }
    fn get_header(&self) -> ExtensionResult<Option<SessionHeader>> {
        self.imports
            .reader_get_header(&self.resource)?
            .as_deref()
            .map(decode)
            .transpose()
    }
    fn get_entries(&self) -> ExtensionResult<Vec<SessionEntry>> {
        decode(&self.imports.reader_get_entries(&self.resource)?)
    }
    fn get_tree(&self) -> ExtensionResult<Vec<SessionTreeNode>> {
        Ok(self
            .imports
            .reader_get_tree(&self.resource)?
            .into_iter()
            .map(|resource| node(self.imports.clone(), resource))
            .collect())
    }
    fn get_session_name(&self) -> ExtensionResult<Option<String>> {
        self.imports.reader_get_session_name(&self.resource)
    }
}
/// Owns one node of the captured host tree.
struct Node<I: Imports> {
    /// Host imports.
    imports: I,
    /// Owned node handle.
    resource: I::Node,
}
/// Retains an acquired host node.
fn node<I: Imports>(imports: I, resource: I::Node) -> SessionTreeNode {
    SessionTreeNode::new(Rc::new(Node { imports, resource }))
}
impl<I: Imports> SessionTreeNodePort for Node<I> {
    fn entry(&self) -> ExtensionResult<SessionEntry> {
        decode(&self.imports.node_entry(&self.resource)?)
    }
    fn children(&self) -> ExtensionResult<Vec<SessionTreeNode>> {
        Ok(self
            .imports
            .node_children(&self.resource)?
            .into_iter()
            .map(|resource| node(self.imports.clone(), resource))
            .collect())
    }
    fn label(&self) -> ExtensionResult<Option<String>> {
        self.imports.node_label(&self.resource)
    }
    fn label_timestamp(&self) -> ExtensionResult<Option<String>> {
        self.imports.node_label_timestamp(&self.resource)
    }
}
/// Decodes a selected payload without an intermediate JSON value tree.
fn decode<T: serde::de::DeserializeOwned>(text: &str) -> ExtensionResult<T> {
    serde_json::from_str(text).map_err(|error| error.to_string())
}
