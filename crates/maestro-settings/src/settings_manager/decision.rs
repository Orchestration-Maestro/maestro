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
    /// A model selection with its requested thinking level, before capability clamping.
    Model {
        /// The requested provider.
        provider: &'a str,
        /// The requested model identifier.
        model_id: &'a str,
        /// The requested thinking level.
        thinking_level: &'a str,
    },
    /// A requested thinking level before capability clamping.
    ThinkingLevel(&'a str),
    /// A requested active-tool set, or one candidate during automatic registry refresh.
    Tools(&'a [String]),
    /// Explicit acquisition of a package source.
    PackageInstall {
        /// The authored package source, including its requested pin.
        source: &'a str,
        /// The selected raw configuration scope.
        scope: SettingsScope,
    },
    /// Explicit removal of a package source.
    PackageRemove {
        /// The authored package source.
        source: &'a str,
        /// The selected raw configuration scope.
        scope: SettingsScope,
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
    /// Locked base values are restored after it returns.
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
        if self.decider.is_some() {
            return Err("A settings decider is already registered".into());
        }
        self.decider = Some(decider);
        self.rebuild();
        Ok(())
    }

    /// Whether explicit operations must preflight with the registered publisher.
    #[must_use]
    pub fn has_decider(&self) -> bool {
        self.decider.is_some()
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

/// An admitted preference edit whose effects may follow other owner effects.
/// Only the preparing manager should apply it.
pub struct PreparedSettingsEdit {
    /// The scope checked before the operation started.
    scope: SettingsScope,
    /// The exact edits already admitted by the publisher and base locks.
    changes: Vec<super::Change>,
}

impl PreparedSettingsEdit {
    /// Supplies the recomputed list for the already-admitted delta.
    ///
    /// # Errors
    /// Returns an error unless exactly one admitted edit has an array value and
    /// retains its scope and key.
    pub fn with_recomputed_list(
        mut self,
        scope: SettingsScope,
        path: &[&str],
        entries: Vec<Value>,
    ) -> Result<Self, String> {
        let [change] = self.changes.as_slice() else {
            return Err("Recomputed list requires one admitted list edit".into());
        };
        let admitted = change.requested();
        if self.scope != scope
            || change.components() != path
            || !matches!(admitted.value, Some(Value::Array(_)))
        {
            return Err("Recomputed list must retain its admitted key and scope".into());
        }
        self.changes = vec![super::Change::from_requested(SettingsChange {
            path: admitted.path,
            value: Some(Value::Array(entries)),
        })];
        Ok(self)
    }

    /// Splits admitted edits so an owner can retain its existing effect order.
    #[must_use]
    pub fn split_at(mut self, index: usize) -> (Self, Self) {
        let changes = self.changes.split_off(index.min(self.changes.len()));
        let remaining = Self {
            scope: self.scope,
            changes,
        };
        (self, remaining)
    }
}

impl SettingsManager {
    /// Admits an operation and all of its derived preference writes without publishing them.
    /// The request describes intent; the edits may contain a capability-clamped result.
    ///
    /// # Errors
    /// Returns the publisher's refusal, a locked-path message or an empty-path error.
    pub fn prepare_operation(
        &self,
        request: &DecisionRequest<'_>,
        scope: SettingsScope,
        changes: Vec<SettingsChange>,
    ) -> Result<PreparedSettingsEdit, String> {
        self.decide(request)?;
        if changes.iter().any(|change| change.path.is_empty()) {
            return Err("Settings edit path cannot be empty".into());
        }
        let changes: Vec<_> = changes
            .into_iter()
            .map(super::Change::from_requested)
            .collect();
        self.check_changes(scope, &changes)?;
        Ok(PreparedSettingsEdit { scope, changes })
    }

    /// Publishes a prepared edit at its owner's original save position, without deciding twice.
    pub fn apply_prepared_edit(&mut self, edit: PreparedSettingsEdit) {
        if !edit.changes.is_empty() {
            self.edit_accepted(edit.scope, edit.changes);
        }
    }
}
