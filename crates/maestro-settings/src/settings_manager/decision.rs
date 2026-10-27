//! Publisher-supplied decisions for explicit requests and stored preferences.

/// The shared publisher handle; native managers can move between threads.
#[cfg(not(target_arch = "wasm32"))]
pub type SettingsDeciderHandle = std::sync::Arc<dyn SettingsDecider>;
/// The shared publisher handle on a single-thread browser host.
#[cfg(target_arch = "wasm32")]
pub type SettingsDeciderHandle = std::rc::Rc<dyn SettingsDecider>;

use super::{Settings, SettingsManager, SettingsScope};
use serde_json::Value;

/// One explicit edit, with literal path components and absence for removal.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsChange {
    /// The top-level field followed by its nested keys.
    pub path: Vec<String>,
    /// The requested value, or absence to remove the member.
    pub value: Option<Value>,
}

/// An operation offered to the registered publisher before its first effect.
#[derive(Debug)]
pub enum DecisionRequest<'a> {
    /// An atomic preference edit in one raw scope.
    SettingsEdit {
        /// The raw scope to edit.
        scope: SettingsScope,
        /// Every requested edit, in application order.
        changes: &'a [SettingsChange],
    },
    /// A one-level merge into the runtime preference view.
    RuntimeOverrides(&'a Settings),
}

/// The one publisher deciding explicit requests and resolving accepted preferences.
pub trait SettingsDecider: super::preferences::thread_bound::StorageThreadBound {
    /// Allows a request or returns the refusal message its caller must report.
    ///
    /// # Errors
    /// Returns the publisher's constraint message before any operation effect.
    fn decide(&self, request: &DecisionRequest<'_>) -> Result<(), String>;

    /// Resolves stored preferences in memory and returns diagnostics for changed values.
    /// Raw scopes and queued edits are never passed to this method.
    fn resolve(&self, _settings: &mut Settings) -> Vec<String> {
        Vec::new()
    }
}

impl SettingsManager {
    /// Registers the session's sole publisher and resolves the current stored view.
    ///
    /// # Errors
    /// Returns an error if a publisher is already registered, retaining the first one.
    pub fn register_decider(&mut self, decider: SettingsDeciderHandle) -> Result<(), String> {
        self.decider = Some(decider);
        Ok(())
    }

    /// Asks the publisher without changing any settings or acquiring storage locks.
    ///
    /// # Errors
    /// Returns the publisher's refusal message.
    pub fn decide(&self, request: &DecisionRequest<'_>) -> Result<(), String> {
        self.decider
            .as_ref()
            .map_or(Ok(()), |decider| decider.decide(request))
    }

    /// Reports a refusal through the ordinary settings error channel.
    pub fn report_refusal(&self, scope: SettingsScope, message: String) {
        self.queue.record_error(scope, message.into());
    }

    /// Checks all edits before publishing any of them.
    pub(super) fn refuse_edit(&self, scope: SettingsScope, changes: &[super::Change]) -> bool {
        if self.refuse_changes(scope, changes) {
            return true;
        }
        let changes: Vec<_> = changes.iter().map(super::Change::requested).collect();
        self.refuse_request(
            scope,
            &DecisionRequest::SettingsEdit {
                scope,
                changes: &changes,
            },
        )
    }

    /// Reports an explicit request's refusal without touching pending edits.
    pub(super) fn refuse_request(
        &self,
        scope: SettingsScope,
        request: &DecisionRequest<'_>,
    ) -> bool {
        if let Err(message) = self.decide(request) {
            self.report_refusal(scope, message);
            return true;
        }
        false
    }
}
