//! Shared admission for tests of process-wide registrations.
use std::sync::{Mutex, MutexGuard, PoisonError};
/// Serializes tests that read or change process-wide provider and cleanup registries.
static REGISTRY: Mutex<()> = Mutex::new(());

/// Hold provider and cleanup registry admission for one test.
pub fn registry() -> MutexGuard<'static, ()> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Registered API identifiers in their visible order.
pub fn registered() -> Vec<String> {
    maestro_models::get_api_providers()
        .into_iter()
        .map(|provider| provider.api)
        .collect()
}
