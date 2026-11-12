//! Writable session operations run on the real setup capability on its local owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::local_resources::{Lease, held::Writer};
use maestro_extensions::{
    CompactionOptions, CompactionResult, CustomMessageInput, NewSessionOptions, Presence,
    ReadonlySessionManager, SessionManager,
};
use serde::de::DeserializeOwned;
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Converts a domain error only after preserving its native operation's outcome.
fn shown<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| error.to_string())
}

/// Store-owned identity of the original writable session.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Applies a mutation to the original setup owner, never to its reader.
    fn with<T: Send + 'static>(
        &self,
        call: impl FnOnce(&mut SessionManager) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        self.0
            .run::<Writer, _>(move |session, _| call(&mut session.borrow_mut()))
    }

    /// Acquires an independently retained reader of the same writable history.
    pub(crate) fn reader(&self) -> Result<crate::history_reads::Endpoint, String> {
        self.0.run::<Writer, _>(|session, registry| {
            let reader: Rc<RefCell<dyn ReadonlySessionManager>> = session.clone();
            registry.history(reader)
        })
    }

    /// Selects a supplied session file.
    pub(crate) fn set_session_file(&self, path: String) -> Result<(), String> {
        self.with(move |session| shown(session.set_session_file(&path)))
    }

    /// Creates a session using the supplied identity and parent fields.
    pub(crate) fn new_session(&self, options: Option<String>) -> Result<Option<String>, String> {
        self.with(move |session| new_session(session, options.as_deref()))
    }

    /// Reads the native publication policy.
    pub(crate) fn is_persisted(&self) -> Result<bool, String> {
        self.with(|session| Ok(session.is_persisted()))
    }

    /// Reads children of the exact parent identity.
    pub(crate) fn get_children(&self, parent: String) -> Result<String, String> {
        self.with(move |session| shown(serde_json::to_string(&session.get_children(&parent))))
    }

    /// Builds model context from the current selected ancestry.
    pub(crate) fn build_session_context(&self) -> Result<String, String> {
        self.with(|session| shown(build_context(session)))
    }

    /// Appends an owned message record.
    pub(crate) fn append_message(&self, value: String) -> Result<String, String> {
        self.with(move |session| {
            decode(&value).and_then(|value| shown(session.append_message(value)))
        })
    }

    /// Appends the authored thinking-level string.
    pub(crate) fn append_thinking_level_change(&self, level: String) -> Result<String, String> {
        self.with(move |session| shown(session.append_thinking_level_change(&level)))
    }

    /// Appends exact provider and model identities.
    pub(crate) fn append_model_change(
        &self,
        provider: String,
        model: String,
    ) -> Result<String, String> {
        self.with(move |session| shown(session.append_model_change(&provider, &model)))
    }

    /// Appends the supplied compaction and independent origin flag.
    pub(crate) fn append_compaction(
        &self,
        value: String,
        from_hook: Option<bool>,
    ) -> Result<String, String> {
        self.with(move |session| append_compaction(session, &value, from_hook))
    }

    /// Appends custom data, preserving absent versus null.
    pub(crate) fn append_custom_entry(
        &self,
        kind: String,
        value: Option<String>,
    ) -> Result<String, String> {
        self.with(move |session| {
            json_presence(value.as_deref())
                .and_then(|data| shown(session.append_custom_entry(&kind, data)))
        })
    }

    /// Appends a name through the domain's own normalization policy.
    pub(crate) fn append_session_info(&self, name: String) -> Result<String, String> {
        self.with(move |session| shown(session.append_session_info(&name)))
    }

    /// Appends custom content with its display and details fields.
    pub(crate) fn append_custom_message_entry(&self, value: String) -> Result<String, String> {
        self.with(move |session| append_custom_message(session, &value))
    }

    /// Changes a label, preserving the supplied whitespace.
    pub(crate) fn append_label_change(
        &self,
        id: String,
        label: Option<String>,
    ) -> Result<String, String> {
        self.with(move |session| {
            shown(session.append_label_change(&id, optional(label.as_deref())))
        })
    }

    /// Selects an exact existing entry.
    pub(crate) fn branch(&self, id: String) -> Result<(), String> {
        self.with(move |session| shown(session.branch(&id)))
    }

    /// Selects the position before all entries.
    pub(crate) fn reset_leaf(&self) -> Result<(), String> {
        self.with(|session| {
            session.reset_leaf();
            Ok(())
        })
    }

    /// Appends a branch summary with independently optional metadata.
    pub(crate) fn branch_with_summary(
        &self,
        id: Option<String>,
        summary: String,
        (details, from_hook): (Option<String>, Option<bool>),
    ) -> Result<String, String> {
        self.with(move |session| {
            json_presence(details.as_deref()).and_then(|details| {
                shown(session.branch_with_summary(
                    id.as_deref(),
                    &summary,
                    details,
                    optional(from_hook),
                ))
            })
        })
    }

    /// Extracts the exact selected entry's ancestry.
    pub(crate) fn create_branched_session(&self, leaf: String) -> Result<Option<String>, String> {
        self.with(move |session| shown(session.create_branched_session(&leaf)))
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
