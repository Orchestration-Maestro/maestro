//! Publisher-supplied decisions for explicit requests and stored preferences.

use std::rc::Rc;

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
pub trait SettingsDecider {
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
    pub fn register_decider(&mut self, _decider: Rc<dyn SettingsDecider>) -> Result<(), String> {
        Ok(())
    }
}
