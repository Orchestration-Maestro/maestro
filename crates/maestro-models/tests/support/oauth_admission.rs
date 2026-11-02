//! Serial admission to the test program's process-wide OAuth provider registry.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use std::sync::{Mutex, MutexGuard};

/// Serialize observations of this program's registry.
static ADMISSION: Mutex<()> = Mutex::new(());

/// Reset registry membership on entry and on drop, including unwinding.
pub(crate) struct Admission {
    /// Retain admission until cleanup completes.
    _guard: MutexGuard<'static, ()>,
}

impl Admission {
    /// Start an isolated registry observation.
    pub(crate) fn enter() -> Self {
        let guard = ADMISSION
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        maestro_models::oauth::reset_oauth_providers();
        Self { _guard: guard }
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        maestro_models::oauth::reset_oauth_providers();
    }
}
