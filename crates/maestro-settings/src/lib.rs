//! Resolves engine → manifest → user → project settings through one governed
//! interface above injected scoped storage. Objects merge recursively; scalars,
//! arrays and present null replace. Origins describe leaves, whole arrays and
//! empty objects. Only manifest declarations freeze exact JSON values/subtrees.
//! Paths are literal property segments, not dotted names or array indexes.
//!
//! Stored updates discard ephemeral overrides; reload retains stored memory
//! state and publishes only after validation. Value locks are not filesystem
//! locks, sandboxing or persistent durability.
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
//! assert_eq!(settings.reload()?.values["theme"], json!("dark"));
//! # Ok::<(), SettingsError>(())
//! ```

mod defaults;
mod resolve;
mod settings;
mod storage;
mod types;

pub use settings::Settings;
pub use storage::{MemorySettingsStorage, SettingsStorage, SettingsTransaction};
pub use types::{
    ManifestSettings, SettingsError, SettingsOrigin, SettingsScope, SettingsSnapshot,
    SettingsTarget,
};
