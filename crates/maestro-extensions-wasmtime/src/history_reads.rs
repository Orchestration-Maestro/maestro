//! Requests on the original read-only history, independent of guarded contexts.
use crate::local_resources::{Lease, Registry};
use maestro_extensions::ReadonlySessionManager;
use std::sync::{Arc, mpsc};

/// A catchable result after the history operation leaves its local owner.
type Reply<T> = mpsc::Sender<Result<T, String>>;

/// Canonical read operations, preserving optional records and authored identifiers.
pub(crate) enum Request {
    /// Reads the original working directory.
    Cwd(Reply<String>),
    /// Reads the original session directory.
    SessionDir(Reply<String>),
    /// Reads the current session identity.
    SessionId(Reply<String>),
    /// Reads the selected session filename.
    SessionFile(Reply<Option<String>>),
    /// Reads the selected leaf identity.
    LeafId(Reply<Option<String>>),
    /// Reads the selected leaf record.
    LeafEntry(Reply<Option<String>>),
    /// Looks up an exact entry identity.
    Entry(String, Reply<Option<String>>),
    /// Reads the resolved label of an exact identity.
    Label(String, Reply<Option<String>>),
    /// Reads ancestry from the supplied identity or current leaf.
    Branch(Option<String>, Reply<String>),
    /// Reads the retained header.
    Header(Reply<Option<String>>),
    /// Reads retained records in their original order.
    Entries(Reply<String>),
    /// Captures the domain-produced tree once, retaining independent nodes.
    Tree(Reply<Vec<crate::tree_reads::Endpoint>>),
    /// Reads the latest session name.
    SessionName(Reply<Option<String>>),
}

impl Request {
    /// Settles a missing capability without leaving its Store caller waiting.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Cwd(reply)
            | Self::SessionDir(reply)
            | Self::SessionId(reply)
            | Self::Branch(_, reply)
            | Self::Entries(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Tree(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::SessionFile(reply)
            | Self::LeafId(reply)
            | Self::LeafEntry(reply)
            | Self::Entry(_, reply)
            | Self::Label(_, reply)
            | Self::Header(reply)
            | Self::SessionName(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }

    /// Reads the retained live owner only when the local pump consumes the request.
    pub(crate) fn answer(self, history: &dyn ReadonlySessionManager, registry: &Registry) {
        match self {
            Self::Cwd(reply) => {
                let _ = reply.send(Ok(history.get_cwd().to_owned()));
            }
            Self::SessionDir(reply) => {
                let _ = reply.send(Ok(history.get_session_dir().to_owned()));
            }
            Self::SessionId(reply) => {
                let _ = reply.send(Ok(history.get_session_id().to_owned()));
            }
            Self::SessionFile(reply) => {
                let _ = reply.send(Ok(history.get_session_file().map(str::to_owned)));
            }
            Self::LeafId(reply) => {
                let _ = reply.send(Ok(history.get_leaf_id().map(str::to_owned)));
            }
            Self::LeafEntry(reply) => {
                let _ = reply.send(encode_optional(history.get_leaf_entry()));
            }
            Self::Entry(id, reply) => {
                let _ = reply.send(encode_optional(history.get_entry(&id)));
            }
            Self::Label(id, reply) => {
                let _ = reply.send(optional_string(history.get_label(&id)));
            }
            Self::Branch(id, reply) => {
                let result = history
                    .get_branch(id.as_deref())
                    .and_then(|entries| serde_json::to_string(&entries));
                let _ = reply.send(result.map_err(|error| error.to_string()));
            }
            Self::Header(reply) => {
                let _ = reply.send(encode_optional(history.get_header()));
            }
            Self::Entries(reply) => {
                let _ = reply.send(
                    serde_json::to_string(&history.get_entries())
                        .map_err(|error| error.to_string()),
                );
            }
            Self::Tree(reply) => {
                let result = crate::tree_reads::Owner::capture(history.get_tree())
                    .and_then(|roots| roots.into_iter().map(|node| registry.node(node)).collect());
                let _ = reply.send(result);
            }
            Self::SessionName(reply) => {
                let _ = reply.send(
                    history
                        .get_session_name()
                        .map(|name| name.map(str::to_owned))
                        .map_err(|error| error.to_string()),
                );
            }
        }
    }
}

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
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);
