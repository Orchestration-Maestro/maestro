//! Live ordered resource cleanup with explicit identity-based removal.

use super::diagnostics::DiagnosticErrorInfo;
use std::collections::BTreeMap;
use std::sync::Arc;

/// One fallible cleanup callback receiving the selected session identity.
#[cfg(not(target_arch = "wasm32"))]
pub type SessionResourceCleanup =
    Arc<dyn Fn(Option<&str>) -> Result<(), DiagnosticErrorInfo> + Send + Sync>;
/// One browser-local fallible cleanup callback.
#[cfg(target_arch = "wasm32")]
pub type SessionResourceCleanup = Arc<dyn Fn(Option<&str>) -> Result<(), DiagnosticErrorInfo>>;

#[derive(Default)]
struct Resources {
    next: usize,
    entries: BTreeMap<usize, SessionResourceCleanup>,
}

#[cfg(not(target_arch = "wasm32"))]
static RESOURCES: std::sync::Mutex<Resources> = std::sync::Mutex::new(Resources {
    next: 0,
    entries: BTreeMap::new(),
});
#[cfg(target_arch = "wasm32")]
thread_local! { static RESOURCES: std::cell::RefCell<Resources> = std::cell::RefCell::new(Resources::default()); }

fn with_resources<T>(operation: impl FnOnce(&mut Resources) -> T) -> T {
    #[cfg(not(target_arch = "wasm32"))]
    {
        operation(
            &mut RESOURCES
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        RESOURCES.with_borrow_mut(operation)
    }
}

/// Explicit removal by callback identity; dropping this handle does not unsubscribe.
pub struct SessionResourceRemoval {
    cleanup: SessionResourceCleanup,
}

impl SessionResourceRemoval {
    /// Remove the current registration, including a re-registration of this callback.
    pub fn remove(&self) {
        let removed = with_resources(|resources| {
            let key = resources
                .entries
                .iter()
                .find_map(|(key, entry)| Arc::ptr_eq(entry, &self.cleanup).then_some(*key));
            key.and_then(|key| resources.entries.remove(&key))
        });
        drop(removed);
    }
}

/// Register a callback once by identity, retaining insertion order.
#[must_use]
pub fn register_session_resource_cleanup(
    cleanup: SessionResourceCleanup,
) -> SessionResourceRemoval {
    with_resources(|resources| {
        if !resources
            .entries
            .values()
            .any(|entry| Arc::ptr_eq(entry, &cleanup))
        {
            let key = resources.next;
            resources.next += 1;
            resources.entries.insert(key, Arc::clone(&cleanup));
        }
    });
    SessionResourceRemoval { cleanup }
}

/// Ordered errors collected after attempting every live cleanup callback.
#[derive(Debug)]
pub struct SessionResourceCleanupError {
    /// Failures in callback encounter order.
    pub errors: Vec<DiagnosticErrorInfo>,
}
impl std::fmt::Display for SessionResourceCleanupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Failed to cleanup session resources")
    }
}
impl std::error::Error for SessionResourceCleanupError {}

/// Traverse live registrations, including additions and excluding unvisited removals.
///
/// # Errors
/// Returns all callback failures in encounter order after traversal finishes.
pub fn cleanup_session_resources(
    session_id: Option<&str>,
) -> Result<(), SessionResourceCleanupError> {
    let mut cursor = None;
    let mut errors = Vec::new();
    loop {
        let next = with_resources(|resources| {
            use std::ops::Bound::{Excluded, Unbounded};
            let lower = cursor.map_or(Unbounded, Excluded);
            resources
                .entries
                .range((lower, Unbounded))
                .next()
                .map(|(key, callback)| (*key, Arc::clone(callback)))
        });
        let Some((key, callback)) = next else { break };
        cursor = Some(key);
        if let Err(error) = callback(session_id) {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(SessionResourceCleanupError { errors })
    }
}
