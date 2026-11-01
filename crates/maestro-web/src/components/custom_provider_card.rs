//! Custom provider summary with caller-owned discovery status and actions.
use crate::CustomProvider;
use std::{cell::RefCell, rc::Rc};

#[cfg(target_arch = "wasm32")]
mod dom;

/// The supplied discovery result, independent of the provider's protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderStatus {
    /// Discovery completed with this many models, including zero.
    Connected {
        /// Number of discovered models.
        model_count: usize,
    },
    /// Discovery is still running.
    Checking,
    /// The server could not be reached.
    Disconnected,
}
/// Retained action receiving the original provider handle.
type ProviderAction = Rc<dyn Fn(Rc<RefCell<CustomProvider>>)>;

/// A provider summary mounted as native page content.
#[derive(Clone)]
pub struct CustomProviderCard {
    /// Original mutable provider handle passed to each current action.
    pub provider: Rc<RefCell<CustomProvider>>,
    /// Whether the supplied discovery status replaces the manual model count.
    pub is_auto_discovery: bool,
    /// Discovery result; absence omits the discovery row.
    pub status: Option<ProviderStatus>,
    /// Current refresh action, visible only for discovery.
    pub on_refresh: Option<ProviderAction>,
    /// Current edit action.
    pub on_edit: Option<ProviderAction>,
    /// Current delete action.
    pub on_delete: Option<ProviderAction>,
}
impl CustomProviderCard {
    /// Retain `provider` with manual model counting and no status or actions.
    #[must_use]
    pub fn new(provider: Rc<RefCell<CustomProvider>>) -> Self {
        Self {
            provider,
            is_auto_discovery: false,
            status: None,
            on_refresh: None,
            on_edit: None,
            on_delete: None,
        }
    }
}
