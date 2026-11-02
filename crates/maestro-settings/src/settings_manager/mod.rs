//! The settings manager: accepted preferences for a global and a project scope.

mod accessors;
mod base_layer;
mod change;
mod composition;
mod conversion;
mod decision;
mod entries;
#[cfg(not(target_arch = "wasm32"))]
mod file_storage;
mod memory_storage;
mod persistence;
mod preferences;
mod presentation;
mod records;
mod resources;
mod vocabulary;

use std::future::Future;

use serde_json::{Map, Value};

use change::{Change, Edit};
use conversion::{convert, parse_document};
use maestro_json::pretty_object;
use persistence::Queue;

pub use base_layer::BaseSettingsLayer;
pub use decision::{
    DecisionRequest, PreparedSettingsEdit, SettingsChange, SettingsDecider, SettingsDeciderHandle,
    SettingsEdit,
};
pub use entries::{FilteredPackage, PackageSource, SettingsListEntry};
#[cfg(not(target_arch = "wasm32"))]
pub use file_storage::FileSettingsStorage;
pub use memory_storage::InMemorySettingsStorage;
use preferences::share;
pub use preferences::{
    Settings, SettingsError, SettingsErrorKind, SettingsScope, SettingsStorage,
    SettingsStorageError, SettingsStorageHandle, SettingsUpdate,
};
pub use records::{
    BranchSummarySettings, CompactionSettings, ImageSettings, MarkdownSettings,
    ProviderRetrySettings, ResolvedBranchSummarySettings, ResolvedCompactionSettings,
    ResolvedProviderRetrySettings, ResolvedRetrySettings, RetrySettings, TerminalSettings,
    ThinkingBudgetsSettings, WarningSettings,
};
pub use vocabulary::{
    DoubleEscapeAction, MessageDeliveryMode, ThinkingLevel, TransportSetting, TreeFilterMode,
};

/// One accepted preference document and whether its last load failed.
#[derive(Default)]
struct Scope {
    /// The accepted preferences.
    settings: Map<String, Value>,
    /// Set after a failed load; the scope accepts edits but is never written.
    load_failed: bool,
}

/// Typed `NaN` setter results, which a JSON snapshot can only hold as `null`.
#[derive(Default)]
struct NotANumber {
    /// The padding was set to `NaN`.
    editor_padding_x: bool,
    /// The autocomplete height was set to `NaN`.
    autocomplete_max_visible: bool,
}

impl NotANumber {
    /// Retains typed results only where composition kept their JSON null representation.
    fn retaining_nulls(&self, document: &Map<String, Value>) -> Self {
        Self {
            editor_padding_x: self.editor_padding_x
                && document.get("editorPaddingX") == Some(&Value::Null),
            autocomplete_max_visible: self.autocomplete_max_visible
                && document.get("autocompleteMaxVisible") == Some(&Value::Null),
        }
    }

    /// The results that a document does not define itself.
    fn unless_defined_by(&self, document: &Map<String, Value>) -> Self {
        Self {
            editor_padding_x: self.editor_padding_x && !document.contains_key("editorPaddingX"),
            autocomplete_max_visible: self.autocomplete_max_visible
                && !document.contains_key("autocompleteMaxVisible"),
        }
    }
}

/// Owns the accepted global and project preferences, their merge, runtime
/// overrides and the queue that persists edits.
pub struct SettingsManager {
    /// Where scopes are loaded from and saved to.
    storage: SettingsStorageHandle,
    /// The sole session publisher; decisions run outside persistence locks.
    decider: Option<SettingsDeciderHandle>,
    /// Read-only published layers, in global/project order.
    base_layers: [Option<BaseSettingsLayer>; 2],
    /// The accepted global scope.
    global: Scope,
    /// The accepted project scope.
    project: Scope,
    /// Global merged with project, then with any runtime overrides.
    effective: Map<String, Value>,
    /// The `NaN` results of the accepted global scope, which setters change and
    /// only a successful global load discards.
    accepted_nan: NotANumber,
    /// The accepted `NaN` results that no project value or applied override
    /// supersedes.
    effective_nan: NotANumber,
    /// The ordered write queue and unsaved edits.
    queue: Queue,
}

impl SettingsManager {
    /// Loads both scopes through the supplied storage.
    #[must_use]
    pub fn from_storage(storage: SettingsStorageHandle) -> Self {
        let queue = Queue::new(storage.clone());
        let mut manager = Self {
            storage,
            decider: None,
            base_layers: [None, None],
            global: Scope::default(),
            project: Scope::default(),
            effective: Map::new(),
            accepted_nan: NotANumber::default(),
            effective_nan: NotANumber::default(),
            queue,
        };
        manager.load_scope(SettingsScope::Global);
        manager.load_scope(SettingsScope::Project);
        manager.load_base_layers();
        manager.rebuild();
        manager
    }

    /// Loads both scopes from `settings.json` files at the supplied locations.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn create(cwd: &str, agent_dir: &str, config_dir: &str) -> Self {
        Self::from_storage(std::sync::Arc::new(FileSettingsStorage::new(
            cwd, agent_dir, config_dir,
        )))
    }

    /// Creates a manager over in-memory storage seeded with global settings after
    /// the stored-format conversions.
    #[must_use]
    pub fn in_memory(settings: Settings) -> Self {
        let mut seed = settings.0;
        convert(&mut seed);
        let (storage, failure) = match pretty_object(&seed) {
            Ok(text) => (InMemorySettingsStorage::seeded(text), None),
            Err(error) => (InMemorySettingsStorage::new(), Some(error)),
        };
        let manager = Self::from_storage(share(storage));
        if let Some(error) = failure {
            manager
                .queue
                .record_error(SettingsScope::Global, Box::new(error));
        }
        manager
    }

    /// Returns an owned copy of the accepted global settings.
    #[must_use]
    pub fn get_global_settings(&self) -> Settings {
        Settings(self.global.settings.clone())
    }

    /// Returns an owned copy of the accepted project settings.
    #[must_use]
    pub fn get_project_settings(&self) -> Settings {
        Settings(self.project.settings.clone())
    }

    /// Waits for queued writes, then reloads both scopes. A scope that fails to
    /// load keeps its accepted settings and records an error; runtime overrides
    /// and unsaved edits are discarded.
    pub async fn reload(&mut self) {
        self.queue.barrier().await;
        self.reload_from_storage();
    }

    /// Reloads both scopes as [`Self::reload`] does, without waiting for queued
    /// writes; await [`Self::flush`] first, so a shared manager can be reloaded
    /// without holding its borrow across the wait.
    pub fn reload_from_storage(&mut self) {
        self.load_scope(SettingsScope::Global);
        self.queue.clear_dirty();
        self.load_scope(SettingsScope::Project);
        self.rebuild();
    }

    /// Layers runtime preferences over the effective settings without saving them.
    pub fn apply_overrides(&mut self, overrides: Settings) {
        if self.refuse_layer_changes(
            SettingsScope::Global,
            self.get_base_layer(),
            &Change::overrides(&overrides.0),
        ) || self.refuse_request(
            SettingsScope::Global,
            &DecisionRequest::RuntimeOverrides(&overrides),
        ) {
            return;
        }
        let Settings(overrides) = overrides;
        self.effective_nan = self.effective_nan.unless_defined_by(&overrides);
        let effective = std::mem::take(&mut self.effective);
        self.effective = self.compose_effective_settings(
            self.get_base_layer(),
            effective,
            &[(SettingsScope::Global, &overrides)],
        );
        self.effective_nan = self.effective_nan.retaining_nulls(&self.effective);
        self.apply_base_nan_locks();
    }

    /// Returns a future that completes once every write queued so far has run.
    /// Dropping it does not cancel any write.
    pub fn flush(&self) -> impl Future<Output = ()> + use<> {
        self.queue.barrier()
    }

    /// Takes the recorded load and save failures.
    pub fn drain_errors(&mut self) -> Vec<SettingsError> {
        self.queue.drain_errors()
    }

    /// Loads one scope, accepting it only when the load succeeds.
    fn load_scope(&mut self, scope: SettingsScope) {
        match self.read_scope(scope) {
            Ok(settings) => {
                if scope == SettingsScope::Global {
                    self.accepted_nan = NotANumber::default();
                }
                *self.scope_mut(scope) = Scope {
                    settings,
                    load_failed: false,
                };
            }
            Err(error) => {
                self.scope_mut(scope).load_failed = true;
                self.queue.record_error(scope, error);
            }
        }
    }

    /// Reads and converts one scope's stored text.
    fn read_scope(&self, scope: SettingsScope) -> Result<Map<String, Value>, SettingsStorageError> {
        let mut text = None;
        self.storage.with_lock(scope, &mut |current| {
            text = current.map(str::to_owned);
            Ok(None)
        })?;
        Ok(parse_document(text.as_deref().unwrap_or_default())?)
    }

    /// The accepted state of one scope.
    fn scope_mut(&mut self, scope: SettingsScope) -> &mut Scope {
        match scope {
            SettingsScope::Global => &mut self.global,
            SettingsScope::Project => &mut self.project,
        }
    }

    /// Recomputes the effective settings, discarding runtime overrides.
    fn rebuild(&mut self) {
        let base = self
            .get_base_layer()
            .map_or_else(Map::new, |layer| layer.settings.0.clone());
        self.effective = self.compose_effective_settings(
            self.get_base_layer(),
            base,
            &[
                (SettingsScope::Global, &self.global.settings),
                (SettingsScope::Project, &self.project.settings),
            ],
        );
        self.effective_nan = self.accepted_nan.unless_defined_by(&self.project.settings);
        self.effective_nan = self.effective_nan.retaining_nulls(&self.effective);
        self.apply_base_nan_locks();
    }

    /// Publishes edits to a scope immediately and queues one save of them unless
    /// the scope's load failed.
    fn edit(&mut self, scope: SettingsScope, changes: Vec<Change>) {
        if self.refuse_edit(scope, &changes) {
            return;
        }
        self.edit_accepted(scope, changes);
    }

    /// Publishes an already checked request, preserving the ordinary save order.
    fn edit_accepted(&mut self, scope: SettingsScope, changes: Vec<Change>) {
        let document = &mut self.scope_mut(scope).settings;
        let edits: Vec<Edit> = changes
            .into_iter()
            .map(|change| change.apply(document))
            .collect();
        for edit in edits {
            self.queue.mark(scope, edit);
        }
        self.rebuild();
        if !self.scope_mut(scope).load_failed {
            let snapshot = self.scope_mut(scope).settings.clone();
            self.queue.save(scope, snapshot);
        }
    }
}
