#![doc = include_str!("../../../docs/storage.md")]

mod memory;
mod types;
pub use memory::MemoryStorage;
pub use types::{
    Record, RecordSession, SessionHeader, SessionMetadata, SessionSnapshot, Storage, StorageError,
};

pub mod conformance;
