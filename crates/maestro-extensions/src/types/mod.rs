//! Owned extension registration records.
mod contexts;
mod execution;
mod registration;
mod renderers;
mod ui;
pub use contexts::*;
pub use execution::ExtensionExecution;
pub use registration::*;
pub use renderers::*;
pub use ui::*;

use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
/// Borrows retained state, recovering ownership after poisoning.
pub(crate) fn read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}
/// Mutably borrows retained state, recovering ownership after poisoning.
pub(crate) fn write<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(PoisonError::into_inner)
}

mod providers;
mod runtime;
pub use providers::*;
pub use runtime::*;
