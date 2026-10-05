//! Serialized sensitive-byte storage.
use crate::CredentialError;
use maestro_models::{Cancellation, SecretString};
use std::sync::Mutex;

/// Authorized sensitive-byte callback, not an ordinary credential export.
/// None reads without replacing; Some replaces; an error changes nothing.
pub type CredentialTransaction<'a> =
    dyn FnMut(Option<SecretString>) -> Result<Option<SecretString>, CredentialError> + 'a;
/// Own one synchronous serialized read/optional replacement without file handles.
pub trait CredentialStorage: Send + Sync {
    /// Invoke the callback once after read/admission, or zero times on failure.
    /// Cancellation rejects waiting admission, not an already admitted write.
    fn transact(
        &self,
        cancellation: &Cancellation,
        edit: &mut CredentialTransaction<'_>,
    ) -> Result<(), CredentialError>;
}
/// Serialized in-process bytes; no persistent durability guarantee.
pub struct MemoryCredentialStorage {
    bytes: Mutex<Option<SecretString>>,
}
impl MemoryCredentialStorage {
    /// Seed an isolated adapter; absent bytes mean an empty store.
    pub fn new(initial: Option<SecretString>) -> Self {
        Self {
            bytes: Mutex::new(initial),
        }
    }
}
impl CredentialStorage for MemoryCredentialStorage {
    fn transact(
        &self,
        cancellation: &Cancellation,
        edit: &mut CredentialTransaction<'_>,
    ) -> Result<(), CredentialError> {
        if cancellation.is_cancelled() {
            return Err(CredentialError::Cancelled);
        }
        let mut bytes = self.bytes.lock().map_err(|_| CredentialError::Storage)?;
        if cancellation.is_cancelled() {
            return Err(CredentialError::Cancelled);
        }
        if let Some(next) = edit(bytes.clone())? {
            *bytes = Some(next);
        }
        Ok(())
    }
}

/// Restrict stored changes without a copy or a second authoritative store.
/// Read-only storage does not contain helper effects or operating-system access.
pub struct ReadOnlyCredentialStorage {
    storage: std::sync::Arc<dyn CredentialStorage>,
}
impl ReadOnlyCredentialStorage {
    /// Wrap the same sensitive-byte seam; all no-write reads remain available.
    pub fn new(storage: std::sync::Arc<dyn CredentialStorage>) -> Self {
        Self { storage }
    }
}
impl CredentialStorage for ReadOnlyCredentialStorage {
    fn transact(
        &self,
        cancellation: &Cancellation,
        edit: &mut CredentialTransaction<'_>,
    ) -> Result<(), CredentialError> {
        self.storage
            .transact(cancellation, &mut |bytes| match edit(bytes)? {
                None => Ok(None),
                Some(_) => Err(CredentialError::ReadOnly),
            })
    }
}
