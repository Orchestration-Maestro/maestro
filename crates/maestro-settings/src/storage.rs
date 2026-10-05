use crate::{SettingsError, SettingsScope};
use serde_json::{Map, Value};

/// Receives the fresh current scope exactly once after admission and parsing.
/// `Ok(None)` leaves settings bytes unchanged; `Ok(Some(map))` replaces the scope;
/// `Err` writes no settings bytes. Pre-admission or read failures invoke it zero
/// times. The callback may borrow caller state for its invocation lifetime.
pub type SettingsTransaction<'a> =
    dyn FnMut(&Map<String, Value>) -> Result<Option<Map<String, Value>>, SettingsError> + 'a;

/// Scoped read-modify-write, with no merging or governance policy.
pub trait SettingsStorage: Send {
    /// Returns detached current values without creation or a write lock.
    /// A missing scope is empty; malformed data and native I/O failures are errors.
    fn read(&mut self, scope: SettingsScope) -> Result<Map<String, Value>, SettingsError>;

    /// Calls `edit` exactly once after admission and reading current state.
    /// Admission/read failures call it zero times. `None` and callback `Err` write
    /// no settings bytes; `Some` replaces only this scope. Native write failure
    /// may leave partial bytes. Transactions may create directories/sidecars.
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
    fn read(&mut self, scope: SettingsScope) -> Result<Map<String, Value>, SettingsError> {
        Ok(match scope {
            SettingsScope::User => self.user.clone(),
            SettingsScope::Project => self.project.clone(),
        })
    }
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
