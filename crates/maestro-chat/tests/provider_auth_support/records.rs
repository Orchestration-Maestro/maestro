//! Strict constructor, operation and observation records.
use crate::bindings::Binding;
use maestro_app::{ApiKeyCredential, AuthCredential, AuthSource, AuthStatus, OAuthCredential};
use maestro_chat::{AuthSelectorProvider, AuthType};

use serde::Deserialize;
use std::collections::BTreeMap;

/// One independently recorded query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Test.
    pub test: String,
    /// Input.
    pub input: serde_json::Value,
    /// Expected.
    pub expected: serde_json::Value,
}
/// Supplied provider identity and authentication kind.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    /// Id.
    pub id: String,
    /// Name.
    pub name: String,
    #[serde(rename = "authType")]
    /// Auth type.
    pub auth_type: Kind,
}
/// Declared authentication choices.
#[derive(Deserialize)]
pub enum Kind {
    #[serde(rename = "oauth")]
    OAuth,
    #[serde(rename = "api_key")]
    ApiKey,
}
impl From<Provider> for AuthSelectorProvider {
    fn from(value: Provider) -> Self {
        Self {
            id: value.id,
            name: value.name,
            auth_type: match value.auth_type {
                Kind::OAuth => AuthType::OAuth,
                Kind::ApiKey => AuthType::ApiKey,
            },
        }
    }
}
/// Stored fixture records with no unconsumed fields.
#[derive(Clone, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Credential {
    #[serde(rename = "api_key")]
    ApiKey { key: String },
    #[serde(rename = "oauth")]
    OAuth {
        access: String,
        refresh: String,
        expires: f64,
    },
}
impl From<Credential> for AuthCredential {
    fn from(value: Credential) -> Self {
        match value {
            Credential::ApiKey { key } => Self::ApiKey(ApiKeyCredential { env: None, key }),
            Credential::OAuth {
                access,
                refresh,
                expires,
            } => Self::OAuth(OAuthCredential {
                access,
                refresh,
                expires,
                extra: serde_json::Map::new(),
            }),
        }
    }
}
/// Recorded metadata independently of stored credentials.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    /// Configured.
    pub configured: bool,
    /// Source.
    pub source: Option<Source>,
    /// Label.
    pub label: Option<String>,
}
/// Every declared status source.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Stored,
    Runtime,
    Environment,
    Fallback,
    ModelsJsonKey,
    ModelsJsonCommand,
}
impl From<Status> for AuthStatus {
    fn from(value: Status) -> Self {
        Self {
            configured: value.configured,
            source: value.source.map(|source| match source {
                Source::Stored => AuthSource::Stored,
                Source::Runtime => AuthSource::Runtime,
                Source::Environment => AuthSource::Environment,
                Source::Fallback => AuthSource::Fallback,
                Source::ModelsJsonKey => AuthSource::ModelsJsonKey,
                Source::ModelsJsonCommand => AuthSource::ModelsJsonCommand,
            }),
            label: value.label,
        }
    }
}
/// A replacement stored record.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stored {
    /// Id.
    pub id: String,
    /// Credential.
    pub credential: Credential,
}
/// Ordered public-interface operations.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Operation {
    Key(String),
    Query {
        query: String,
    },
    Bindings {
        bindings: BTreeMap<String, Binding>,
    },
    #[serde(rename_all = "camelCase")]
    ChildSubmit {
        child_submit: String,
    },
    Focus {
        focus: bool,
    },
    #[serde(rename_all = "camelCase")]
    InputFocus {
        input_focus: bool,
    },
    Theme {
        theme: u8,
    },
    Invalidate {
        invalidate: bool,
    },
    #[serde(rename_all = "camelCase")]
    SetStored {
        set_stored: Stored,
    },
    #[serde(rename_all = "camelCase")]
    RemoveStored {
        remove_stored: String,
    },
    Statuses {
        statuses: BTreeMap<String, Status>,
    },
    Clear {
        clear: bool,
    },
    #[serde(rename_all = "camelCase")]
    ReplaceChildren {
        replace_children: bool,
    },
    #[serde(rename_all = "camelCase")]
    RestoreChildren {
        restore_children: bool,
    },
}
/// Constructor inputs, with absence kept where it selects a default resolver.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Query {
    /// Mode.
    pub mode: Option<String>,
    /// Providers.
    pub providers: Option<Vec<Provider>>,
    /// Ops.
    pub ops: Vec<Operation>,
    /// Stored.
    pub stored: BTreeMap<String, Credential>,
    /// Statuses.
    pub statuses: Option<BTreeMap<String, Status>>,
    /// Runtime.
    pub runtime: BTreeMap<String, String>,
    /// Fallback.
    pub fallback: Option<String>,
    /// Env.
    pub env: BTreeMap<String, String>,
    /// Width.
    pub width: Option<usize>,
    /// Bindings.
    pub bindings: BTreeMap<String, Binding>,
    /// Reenter.
    pub reenter: bool,
    #[serde(rename = "reenterCancel")]
    /// Reenter cancel.
    pub reenter_cancel: bool,
}
/// Complete observable frame with exact order and repeated occurrences.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// Lines.
    pub lines: Vec<String>,
    /// Events.
    pub events: Vec<Vec<String>>,
    /// Status calls.
    pub status_calls: Vec<String>,
    /// Focus.
    pub focus: Option<[bool; 2]>,
}
/// Construction and post-operation observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Frames.
    pub frames: Vec<Frame>,
    /// Stored unchanged.
    pub stored_unchanged: Option<bool>,
    /// Runtime unchanged.
    pub runtime_unchanged: Option<Vec<(String, String)>>,
}
