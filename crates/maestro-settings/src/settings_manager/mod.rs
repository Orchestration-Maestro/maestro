//! The settings manager: accepted preferences for a global and a project scope.

mod accessors;
mod conversion;
mod entries;
#[cfg(not(target_arch = "wasm32"))]
mod file_storage;
mod memory_storage;
mod paths;
mod persistence;
mod preferences;
mod presentation;
mod records;
mod resources;
mod vocabulary;

use std::future::Future;

use serde_json::{Map, Value};

use conversion::{convert, merge_one_level, parse_document, to_text};
use persistence::{Edit, Queue};

pub use entries::{FilteredPackage, PackageSource, SettingsListEntry};
#[cfg(not(target_arch = "wasm32"))]
pub use file_storage::FileSettingsStorage;
pub use memory_storage::InMemorySettingsStorage;
use preferences::share;
pub use preferences::{
    Settings, SettingsError, SettingsScope, SettingsStorage, SettingsStorageError,
    SettingsStorageHandle, SettingsUpdate,
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
    /// The results that a document does not define itself.
    fn unless_defined_by(&self, document: &Map<String, Value>) -> Self {
        Self {
            editor_padding_x: self.editor_padding_x && !document.contains_key("editorPaddingX"),
            autocomplete_max_visible: self.autocomplete_max_visible
                && !document.contains_key("autocompleteMaxVisible"),
        }
    }
}

/// One edit to a scope: set or remove a member, or set a key inside a member.
struct Change {
    /// The top-level member.
    field: &'static str,
    /// The key inside the member, for nested edits.
    key: Option<&'static str>,
    /// The new value; `None` removes the member.
    value: Option<Value>,
}

impl Change {
    /// Sets or removes a top-level member.
    fn field(field: &'static str, value: Option<Value>) -> Self {
        Self {
            field,
            key: None,
            value,
        }
    }

    /// Sets one key of a top-level object member.
    fn key(field: &'static str, key: &'static str, value: Value) -> Self {
        Self {
            field,
            key: Some(key),
            value: Some(value),
        }
    }

    /// Applies the change to a scope document and names the edited preference.
    fn apply(self, document: &mut Map<String, Value>) -> Edit {
        let field = self.field.to_owned();
        let Some(key) = self.key else {
            match self.value {
                Some(value) => document.insert(field.clone(), value),
                None => document.shift_remove(&field),
            };
            return Edit::Field(field);
        };
        let slot = document.entry(field.clone()).or_insert(Value::Null);
        if !slot.is_object() {
            *slot = Value::Object(Map::new());
        }
        if let (Value::Object(members), Some(value)) = (slot, self.value) {
            members.insert(key.to_owned(), value);
        }
        Edit::Key(field, key.to_owned())
    }
}

/// Owns the accepted global and project preferences, their merge, runtime
/// overrides and the queue that persists edits.
pub struct SettingsManager {
    /// Where scopes are loaded from and saved to.
    storage: SettingsStorageHandle,
    /// The accepted global scope.
    global: Scope,
    /// The accepted project scope.
    project: Scope,
    /// Global merged with project, then with any runtime overrides.
    effective: Map<String, Value>,
    /// The `NaN` results of the accepted global scope, which setters change and
    /// only a global load discards.
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
            global: Scope::default(),
            project: Scope::default(),
            effective: Map::new(),
            accepted_nan: NotANumber::default(),
            effective_nan: NotANumber::default(),
            queue,
        };
        manager.load_scope(SettingsScope::Global);
        manager.load_scope(SettingsScope::Project);
        manager.rebuild();
        manager
    }

    /// Loads both scopes from `settings.json` files at the supplied locations.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn create(
        cwd: &std::path::Path,
        agent_dir: &std::path::Path,
        config_dir: &std::ffi::OsStr,
    ) -> Self {
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
        let (storage, failure) = match to_text(&seed) {
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
        self.load_scope(SettingsScope::Global);
        self.queue.clear_dirty();
        self.load_scope(SettingsScope::Project);
        self.rebuild();
    }

    /// Layers runtime preferences over the effective settings without saving them.
    pub fn apply_overrides(&mut self, overrides: Settings) {
        self.effective_nan = self.effective_nan.unless_defined_by(&overrides.0);
        let effective = std::mem::take(&mut self.effective);
        self.effective = merge_one_level(effective, overrides.0);
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
        self.effective =
            merge_one_level(self.global.settings.clone(), self.project.settings.clone());
        self.effective_nan = self.accepted_nan.unless_defined_by(&self.project.settings);
    }

    /// Publishes edits to a scope immediately and queues one save of them unless
    /// the scope's load failed.
    fn edit(&mut self, scope: SettingsScope, changes: Vec<Change>) {
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
