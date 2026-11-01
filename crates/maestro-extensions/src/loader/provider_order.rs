//! Retains early provider operations until their ordinary extension position.
use crate::{
    EventError, ExtensionExecution,
    types::{ExtensionRuntime, PendingProviderRegistration, read, write},
};

/// An early factory's provider operation, before queue admission or cancellation.
pub(crate) enum DeferredProviderOperation {
    /// A registration with its original diagnostic owner.
    Register(PendingProviderRegistration),
    /// A removal that cancels only registrations preceding its ordinary position.
    Unregister(String),
}

impl<E: ExtensionExecution> ExtensionRuntime<E> {
    /// Holds these factories' provider operations until the application restores their order.
    pub fn defer_provider_operations(&self, paths: &[String]) {
        let retired = std::mem::replace(
            &mut write(&self.providers).deferred,
            paths
                .iter()
                .map(|path| (path.clone(), Vec::new()))
                .collect(),
        );
        drop(retired);
    }

    /// Admits an early factory's operations at its ordinary extension position, once.
    ///
    /// # Errors
    /// Returns a provider operation's error if the runtime is already bound.
    pub fn replay_provider_operations(&self, path: &str) -> Result<(), EventError> {
        let operations = write(&self.providers)
            .deferred
            .shift_remove(path)
            .unwrap_or_default();
        for operation in operations {
            match operation {
                DeferredProviderOperation::Register(pending) => self.register_provider(
                    pending.name,
                    pending.config,
                    Some(pending.extension_path),
                )?,
                DeferredProviderOperation::Unregister(name) => self.unregister_provider(&name)?,
            }
        }
        Ok(())
    }

    /// Admits unreached early factories, including failed ones, in original source order.
    ///
    /// # Errors
    /// Returns a provider operation's error if the runtime is already bound.
    pub fn replay_remaining_provider_operations(&self) -> Result<(), EventError> {
        let paths: Vec<_> = read(&self.providers).deferred.keys().cloned().collect();
        for path in paths {
            self.replay_provider_operations(&path)?;
        }
        Ok(())
    }
}
