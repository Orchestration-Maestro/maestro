//! Accepted user and project preferences with detached reads and queued persistence.
//!
//! A caller supplies raw storage and a deferred executor. Setters publish the
//! accepted cache immediately; `flush` observes queued writes, and errors drain
//! separately. Raw values are not admitted through a whole-document schema.
#![doc = include_str!("../../../docs/settings.md")]

mod settings;
pub use settings::settings_manager::*;
