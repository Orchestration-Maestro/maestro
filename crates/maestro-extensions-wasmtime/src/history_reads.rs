//! Operations on the original read-only history, independent of guarded contexts.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::local_resources::{Lease, held::History};
use std::sync::Arc;

/// Validates a canonical optional string without JSON-quoting its content.
pub(crate) fn optional_string(value: Option<&serde_json::Value>) -> Result<Option<String>, String> {
    value
        .map(<String as serde::Deserialize>::deserialize)
        .transpose()
        .map_err(|error| error.to_string())
}

/// Encodes optional raw JSON without collapsing missing and explicit null.
fn encode_optional(value: Option<&serde_json::Value>) -> Result<Option<String>, String> {
    value
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| error.to_string())
}

/// Store-owned identity of an independently retained live history.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Reads the original working directory.
    pub(crate) fn get_cwd(&self) -> Result<String, String> {
        self.0
            .run::<History, _>(|history, _| Ok(history.borrow().get_cwd().to_owned()))
    }

    /// Reads the original session directory.
    pub(crate) fn get_session_dir(&self) -> Result<String, String> {
        self.0
            .run::<History, _>(|history, _| Ok(history.borrow().get_session_dir().to_owned()))
    }

    /// Reads the current session identity.
    pub(crate) fn get_session_id(&self) -> Result<String, String> {
        self.0
            .run::<History, _>(|history, _| Ok(history.borrow().get_session_id().to_owned()))
    }

    /// Reads the selected session filename.
    pub(crate) fn get_session_file(&self) -> Result<Option<String>, String> {
        self.0.run::<History, _>(|history, _| {
            Ok(history.borrow().get_session_file().map(str::to_owned))
        })
    }

    /// Reads the selected leaf identity.
    pub(crate) fn get_leaf_id(&self) -> Result<Option<String>, String> {
        self.0
            .run::<History, _>(|history, _| Ok(history.borrow().get_leaf_id().map(str::to_owned)))
    }

    /// Reads the selected leaf record.
    pub(crate) fn get_leaf_entry(&self) -> Result<Option<String>, String> {
        self.0
            .run::<History, _>(|history, _| encode_optional(history.borrow().get_leaf_entry()))
    }

    /// Looks up an exact entry identity.
    pub(crate) fn get_entry(&self, id: String) -> Result<Option<String>, String> {
        self.0
            .run::<History, _>(move |history, _| encode_optional(history.borrow().get_entry(&id)))
    }

    /// Reads the resolved label of an exact identity.
    pub(crate) fn get_label(&self, id: String) -> Result<Option<String>, String> {
        self.0
            .run::<History, _>(move |history, _| optional_string(history.borrow().get_label(&id)))
    }

    /// Reads ancestry from the supplied identity or current leaf.
    pub(crate) fn get_branch(&self, from: Option<String>) -> Result<String, String> {
        self.0.run::<History, _>(move |history, _| {
            history
                .borrow()
                .get_branch(from.as_deref())
                .and_then(|entries| serde_json::to_string(&entries))
                .map_err(|error| error.to_string())
        })
    }

    /// Reads the retained header.
    pub(crate) fn get_header(&self) -> Result<Option<String>, String> {
        self.0
            .run::<History, _>(|history, _| encode_optional(history.borrow().get_header()))
    }

    /// Reads retained records in their original order.
    pub(crate) fn get_entries(&self) -> Result<String, String> {
        self.0.run::<History, _>(|history, _| {
            serde_json::to_string(&history.borrow().get_entries())
                .map_err(|error| error.to_string())
        })
    }

    /// Captures the domain-produced tree once, retaining independent nodes.
    pub(crate) fn get_tree(&self) -> Result<Vec<crate::tree_reads::Endpoint>, String> {
        self.0.run::<History, _>(|history, registry| {
            crate::tree_reads::Owner::capture(history.borrow().get_tree())
                .and_then(|roots| roots.into_iter().map(|node| registry.node(node)).collect())
        })
    }

    /// Reads the latest session name.
    pub(crate) fn get_session_name(&self) -> Result<Option<String>, String> {
        self.0.run::<History, _>(|history, _| {
            history
                .borrow()
                .get_session_name()
                .map(|name| name.map(str::to_owned))
                .map_err(|error| error.to_string())
        })
    }
}
