//! Canonical session operands become the original domain's optional records.
use crate::{guest_calls::Handle, local_resources::Registry};
use maestro_extensions::types::{ForkOptions, NewSessionCommandOptions, SwitchSessionOptions};
use wasmtime::component::{ComponentType, Lift};

/// Canonical plain new-session data; outer record presence is independent of its parent.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct NewSessionData {
    /// Exact optional parent-session path.
    #[component(name = "parent-session")]
    pub(crate) parent_session: Option<String>,
}

/// A new-session operation's original setup and continuation operands.
pub(crate) struct NewSession {
    /// Absent versus present-empty options remains observable by the native owner.
    pub(crate) data: Option<NewSessionData>,
    /// Guest setup invoked with the real writable session.
    pub(crate) setup: Option<Handle>,
    /// Guest continuation invoked with the real replacement context.
    pub(crate) with_session: Option<Handle>,
}

impl NewSession {
    /// Constructs native callbacks only on the local owner, never inside Send Store data.
    pub(crate) fn options(self, registry: &Registry) -> Option<NewSessionCommandOptions> {
        self.data.map(|data| NewSessionCommandOptions {
            parent_session: data.parent_session,
            setup: self
                .setup
                .map(|callback| callback.setup_callback(registry.clone())),
            with_session: self
                .with_session
                .map(|callback| callback.continuation(registry.clone())),
        })
    }
}

/// Plain fork fields use the shared domain's position vocabulary and decoder.
#[derive(serde::Deserialize)]
struct ForkData {
    /// Absent position selects no adapter default.
    position: Option<maestro_extensions::ForkPosition>,
}

/// Preserves absent versus empty fork data and its independently optional continuation.
pub(crate) fn fork_options(
    data: Option<&str>,
    callback: Option<Handle>,
    registry: &Registry,
) -> Result<Option<ForkOptions>, String> {
    crate::api_reads::optional::<ForkData>(data).map(|data| {
        data.map(|data| ForkOptions {
            position: data.position,
            with_session: callback.map(|callback| callback.continuation(registry.clone())),
        })
    })
}

/// A present switch record retains its independently optional continuation.
pub(crate) struct Switch {
    /// Absence here does not erase the surrounding options record.
    pub(crate) with_session: Option<Handle>,
}

impl Switch {
    /// Builds native options without synthesizing an outer record or a callback.
    pub(crate) fn options(self, registry: &Registry) -> SwitchSessionOptions {
        SwitchSessionOptions {
            with_session: self
                .with_session
                .map(|callback| callback.continuation(registry.clone())),
        }
    }
}
