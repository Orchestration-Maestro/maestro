//! Resolves engine → manifest → user → project settings through one governed
//! interface above injected scoped storage. Objects merge recursively; scalars,
//! arrays and present null replace. Origins describe leaves, whole arrays and
//! empty objects. Only manifest declarations freeze exact JSON values/subtrees.
//! Paths are literal property segments, not dotted names or array indexes.
//!
//! Scoped reads expose detached accepted maps, including shadowed values.
//! Runtime overlays merge recursively; addressed setters replace exactly.
//! Startup load failures supply empty scope contributions and drainable errors.
//! Failed scope loads disable persistence until successful reload reads: valid
//! stored setters still update cached values without writing that scope.
//! Transactional saves preserve fresh disk keys but publish only cached values
//! plus the requested edit, discarding ephemeral overrides. Reload publishes both
//! scopes atomically after reads and governance succeed. Value locks are not filesystem
//! locks, sandboxing or persistent durability. Memory and JSON-object file
//! adapters share detached reads and exactly-once admitted transactions. Native
//! file transactions serialize cooperating writers via persistent sidecars;
//! reads create nothing. Failed in-place writes can leave partial bytes: there
//! is no crash recovery, rollback or atomic publication to lock-free readers.
//! Locations use supplied cwd/home/configuration inputs, never ambient discovery.
//! Invocation/resource text is trimmed, leading tilde expanded, and dot segments
//! normalized lexically without filesystem lookup. Session locks compare raw text.
//!
//! ```
//! use maestro_settings::{FileSettingsStorage, SettingsLocations, SettingsScope};
//! use std::{path::PathBuf, sync::{Arc, atomic::AtomicBool}};
//! let locations = SettingsLocations::new(PathBuf::from("/synthetic/cwd"), None,
//!     PathBuf::from("/synthetic/config"), PathBuf::from("/synthetic/home"))?;
//! assert_eq!(locations.configuration_directory(SettingsScope::Project),
//!     PathBuf::from("/synthetic/cwd/.maestro"));
//! // Construction and pure path selection do not touch these synthetic paths.
//! let storage = FileSettingsStorage::new(locations, Arc::new(AtomicBool::new(false)));
//! # let _ = storage;
//! # Ok::<(), maestro_settings::SettingsError>(())
//! ```
//!
//! ```
//! use maestro_settings::{ManifestSettings, MemorySettingsStorage, Settings,
//!     SettingsError, SettingsOrigin, SettingsScope, SettingsTarget};
//! use serde_json::{Map, Value, json};
//!
//! fn object(value: Value) -> Map<String, Value> {
//!     value.as_object().unwrap().clone()
//! }
//! let mut settings = Settings::new(
//!     object(json!({"theme":"light"})),
//!     ManifestSettings {
//!         values: object(json!({"endpoint":"governed"})),
//!         locks: vec![vec!["endpoint".into()]],
//!     },
//!     Box::new(MemorySettingsStorage::new(Map::new(), Map::new())),
//! )?;
//! assert_eq!(settings.resolve().origins[&vec!["theme".into()]],
//!     SettingsOrigin::Engine);
//! settings.set(SettingsTarget::Stored(SettingsScope::User),
//!     &["theme".into()], json!("dark"))?;
//! assert!(matches!(settings.set(SettingsTarget::Override("cli".into()),
//!     &["endpoint".into()], json!("other")),
//!     Err(SettingsError::LockConflict { .. })));
//! assert_eq!(settings.read_scope(SettingsScope::User)["theme"], json!("dark"));
//! settings.apply_overrides("runtime".into(), object(json!({"display":{"width":80}})))?;
//! assert_eq!(settings.resolve().values["display"]["width"], json!(80));
//! assert!(settings.drain_errors().is_empty());
//! assert_eq!(settings.reload()?.values["theme"], json!("dark"));
//! # Ok::<(), SettingsError>(())
//! ```

mod defaults;
mod file;
mod locations;
mod resolve;
mod settings;
mod storage;
mod types;

pub use file::FileSettingsStorage;
pub use locations::SettingsLocations;
pub use settings::Settings;
pub use storage::{MemorySettingsStorage, SettingsStorage, SettingsTransaction};
pub use types::{
    ManifestSettings, SettingsError, SettingsFileError, SettingsOrigin, SettingsScope,
    SettingsSnapshot, SettingsTarget,
};

#[cfg(test)]
use serde_json::Value;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/support/scratch.rs"]
mod scratch;
