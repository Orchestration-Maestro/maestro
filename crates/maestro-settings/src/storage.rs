use crate::{SettingsError, SettingsScope};
use serde_json::{Map, Value};

/// A transaction callback receiving the current scope map exactly once.
/// `Ok(None)` reads without writing; `Ok(Some(map))` replaces the scope;
/// `Err` rejects the transaction without mutation. The callback may borrow
/// caller state for its invocation lifetime.
pub type SettingsTransaction<'a> =
    dyn FnMut(&Map<String, Value>) -> Result<Option<Map<String, Value>>, SettingsError> + 'a;

/// Scoped read-modify-write, with no merging or governance policy.
pub trait SettingsStorage: Send {
    /// Calls `edit` exactly once on current state while owning the transaction.
    /// `None` reads, `Some` replaces only this scope, and `Err` leaves it unchanged.
    fn transact(
        &mut self,
        scope: SettingsScope,
        edit: &mut SettingsTransaction<'_>,
    ) -> Result<(), SettingsError>;
}

/// Two independent in-process settings maps; does not imply file durability.
pub struct MemorySettingsStorage {
    user: Map<String, Value>,
    project: Map<String, Value>,
}
impl MemorySettingsStorage {
    /// Seeds both scopes so reads retain their initial values.
    pub fn new(user: Map<String, Value>, project: Map<String, Value>) -> Self {
        Self { user, project }
    }
}
impl SettingsStorage for MemorySettingsStorage {
    fn transact(
        &mut self,
        scope: SettingsScope,
        edit: &mut SettingsTransaction<'_>,
    ) -> Result<(), SettingsError> {
        let map = match scope {
            SettingsScope::User => &mut self.user,
            SettingsScope::Project => &mut self.project,
        };
        if let Some(next) = edit(map)? {
            *map = next;
        }
        Ok(())
    }
}
