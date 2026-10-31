//! Writable session requests retain the real setup capability on its local owner.
use crate::local_resources::{Lease, Message};
use maestro_extensions::{
    CompactionOptions, CompactionResult, CustomMessageInput, NewSessionOptions, Presence,
    SessionManager,
};
use serde::de::DeserializeOwned;
use std::sync::{Arc, mpsc};

/// A catchable result after the native session operation.
type Reply<T> = mpsc::Sender<Result<T, String>>;

/// Exact writable-session operands; none are replaced with a read-only stand-in.
pub(crate) enum Request {
    /// Selects a supplied session file.
    SetFile(String, Reply<()>),
    /// Creates a session using the supplied identity and parent fields.
    NewSession(Option<String>, Reply<Option<String>>),
    /// Reads the native publication policy.
    Persisted(Reply<bool>),
    /// Reads children of the exact parent identity.
    Children(String, Reply<String>),
    /// Builds model context from the current selected ancestry.
    BuildContext(Reply<String>),
    /// Appends an owned message record.
    Message(String, Reply<String>),
    /// Appends the authored thinking-level string.
    Thinking(String, Reply<String>),
    /// Appends exact provider and model identities.
    Model(String, String, Reply<String>),
    /// Appends the supplied compaction and independent origin flag.
    Compaction(String, Option<bool>, Reply<String>),
    /// Appends custom data, preserving absent versus null.
    Custom(String, Option<String>, Reply<String>),
    /// Appends a name through the domain's own normalization policy.
    Info(String, Reply<String>),
    /// Appends custom content with its display and details fields.
    CustomMessage(String, Reply<String>),
    /// Changes a label, preserving the supplied whitespace.
    Label(String, Option<String>, Reply<String>),
    /// Selects an exact existing entry.
    Branch(String, Reply<()>),
    /// Selects the position before all entries.
    ResetLeaf(Reply<()>),
    /// Appends a branch summary with independently optional metadata.
    Summary(
        Option<String>,
        String,
        (Option<String>, Option<bool>),
        Reply<String>,
    ),
    /// Extracts the exact selected entry's ancestry.
    CreateBranched(String, Reply<Option<String>>),
}

impl Request {
    /// Settles missing writable capabilities without stranding their callers.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::SetFile(_, reply) | Self::Branch(_, reply) | Self::ResetLeaf(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Persisted(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::NewSession(_, reply) | Self::CreateBranched(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Children(_, reply)
            | Self::BuildContext(reply)
            | Self::Message(_, reply)
            | Self::Thinking(_, reply)
            | Self::Model(_, _, reply)
            | Self::Compaction(_, _, reply)
            | Self::Custom(_, _, reply)
            | Self::Info(_, reply)
            | Self::CustomMessage(_, reply)
            | Self::Label(_, _, reply)
            | Self::Summary(_, _, _, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }

    /// Invokes the actual session operation after the pump selects its retained owner.
    pub(crate) fn answer(self, session: &mut SessionManager) {
        match self {
            Self::SetFile(path, reply) => send(&reply, session.set_session_file(&path)),
            Self::NewSession(options, reply) => {
                send(&reply, new_session(session, options.as_deref()));
            }
            Self::Persisted(reply) => {
                let _ = reply.send(Ok(session.is_persisted()));
            }
            Self::Children(id, reply) => {
                send(&reply, serde_json::to_string(&session.get_children(&id)));
            }
            Self::BuildContext(reply) => send(&reply, build_context(session)),
            Self::Message(value, reply) => send(
                &reply,
                decode(&value)
                    .and_then(|value| session.append_message(value).map_err(|e| e.to_string())),
            ),
            Self::Thinking(level, reply) => {
                send(&reply, session.append_thinking_level_change(&level));
            }
            Self::Model(provider, id, reply) => {
                send(&reply, session.append_model_change(&provider, &id));
            }
            Self::Info(name, reply) => send(&reply, session.append_session_info(&name)),
            Self::Label(id, label, reply) => send(
                &reply,
                session.append_label_change(&id, optional(label.as_deref())),
            ),
            Self::Branch(id, reply) => send(&reply, session.branch(&id)),
            Self::ResetLeaf(reply) => {
                session.reset_leaf();
                let _ = reply.send(Ok(()));
            }
            Self::CreateBranched(id, reply) => send(&reply, session.create_branched_session(&id)),
            Self::Compaction(value, from_hook, reply) => {
                send(&reply, append_compaction(session, &value, from_hook));
            }
            Self::Custom(kind, value, reply) => send(
                &reply,
                json_presence(value.as_deref()).and_then(|data| {
                    session
                        .append_custom_entry(&kind, data)
                        .map_err(|e| e.to_string())
                }),
            ),
            Self::CustomMessage(value, reply) => {
                send(&reply, append_custom_message(session, &value));
            }
            Self::Summary(id, summary, (details, from_hook), reply) => send(
                &reply,
                json_presence(details.as_deref()).and_then(|details| {
                    session
                        .branch_with_summary(id.as_deref(), &summary, details, optional(from_hook))
                        .map_err(|e| e.to_string())
                }),
            ),
        }
    }
}

/// Encodes the domain-built context without a second projection.
fn build_context(session: &SessionManager) -> Result<String, serde_json::Error> {
    serde_json::to_string(&session.build_session_context()?)
}

/// Decodes session metadata before replacing any retained state.
fn new_session(
    session: &mut SessionManager,
    options: Option<&str>,
) -> Result<Option<String>, String> {
    let options = options
        .map(decode::<NewSessionOptions>)
        .transpose()?
        .unwrap_or_default();
    Ok(session.new_session(options).map(str::to_owned))
}

/// Reuses the shared compaction record and native optional-field representation.
fn append_compaction(
    session: &mut SessionManager,
    value: &str,
    from_hook: Option<bool>,
) -> Result<String, String> {
    let value: CompactionResult = decode(value)?;
    session
        .append_compaction(
            &value.summary,
            &value.first_kept_entry_id,
            value.tokens_before,
            CompactionOptions {
                details: value.details,
                from_hook: optional(from_hook),
            },
        )
        .map_err(|e| e.to_string())
}

/// Reuses the shared custom-message record rather than reconstructing its content.
fn append_custom_message(session: &mut SessionManager, value: &str) -> Result<String, String> {
    let value: CustomMessageInput = decode(value)?;
    session
        .append_custom_message_entry(
            &value.custom_type,
            value.content,
            value.display,
            value.details,
        )
        .map_err(|e| e.to_string())
}

/// Preserves omitted values without conflating JSON null with absence.
fn optional<T>(value: Option<T>) -> Presence<T> {
    value.map_or(Presence::Missing, Presence::Present)
}

/// Parses explicit null separately from the omitted outer operand.
fn json_presence(value: Option<&str>) -> Result<Presence<serde_json::Value>, String> {
    value.map_or(Ok(Presence::Missing), decode)
}

/// Decodes shared native records rather than redeclaring their fields here.
pub(crate) fn decode<T: DeserializeOwned>(value: &str) -> Result<T, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

/// Converts a domain error only after preserving its native operation's outcome.
fn send<T, E: std::fmt::Display>(reply: &Reply<T>, result: Result<T, E>) {
    let _ = reply.send(result.map_err(|error| error.to_string()));
}

/// Store-owned identity of the original writable session.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Acquires an independently retained reader of the same writable history.
    pub(crate) fn reader(&self) -> Result<crate::history_reads::Endpoint, String> {
        self.0.request(Message::WriterReader)
    }
}
