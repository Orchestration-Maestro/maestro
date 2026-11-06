//! Production ownership of poison-tolerant lock acquisition through the workspace check.
#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// Diagnostic for one located independent recovery helper.
fn expected(path: &std::path::Path, lines: &[usize]) -> Result<(), String> {
    if lines.is_empty() {
        return Ok(());
    }
    Err(lines
        .iter()
        .map(|line| {
            format!(
                "{}:{line}: production code defines its own poison-tolerant lock acquisition; call maestro_lock::lock, maestro_lock::read or maestro_lock::write",
                path.display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Write one member's source and compare the complete located diagnostics.
fn check(file: &str, contents: &str, lines: &[usize]) {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-agent"]);
    let path = source(&workspace, "maestro-agent", file, contents);
    assert_eq!(
        check_workspace(workspace.root.path()),
        expected(&path, lines)
    );
}

/// A free helper that recovers a read-write lock.
const FREE_HELPER: &str = r"//! Agent.

use std::sync::{PoisonError, RwLock, RwLockReadGuard};

/// Reads the retained queue.
fn read<T>(queue: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    queue.read().unwrap_or_else(PoisonError::into_inner)
}

/// Reads the retained queue.
fn held(queue: &RwLock<u8>) -> u8 {
    *read(queue)
}
";

/// An inherent accessor that recovers a mutex.
const ACCESSOR: &str = r"//! Agent.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Holds the admitted entries.
struct Queue {
    /// Admitted entries in order.
    entries: Mutex<Vec<u8>>,
}

impl Queue {
    /// Locks the entries for one short step.
    fn locked(&self) -> MutexGuard<'_, Vec<u8>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
";

/// A closure that recovers a mutex.
const CLOSURE: &str = r"//! Agent.

/// Reads the admitted entry.
fn entry() -> u8 {
    let read = |value: &std::sync::Mutex<u8>| {
        *value.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    read(&std::sync::Mutex::new(1))
}
";

/// A macro definition that declares a recovering accessor.
const MACRO_DEFINITION: &str = r"//! Agent.

/// Declares an accessor for one retained store.
macro_rules! accessor {
    ($name:ident) => {
        fn $name<T>(store: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
            store.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
        }
    };
}

accessor!(locked);
";

/// An ordinary macro invoked with a recovering accessor among its arguments.
const MACRO_ARGUMENT: &str = r"//! Agent.

/// Emits the supplied items.
macro_rules! register {
    ($($item:item)*) => {
        $($item)*
    };
}

register! {
    fn locked(store: &std::sync::Mutex<u8>) -> u8 {
        *store.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
";

#[test]
fn free_recovery_helper_is_rejected() {
    check("src/lib.rs", FREE_HELPER, &[7]);
}

#[test]
fn recovering_accessor_in_an_impl_is_rejected() {
    check("src/queue.rs", ACCESSOR, &[14]);
}

#[test]
fn recovering_closure_is_rejected() {
    check("src/entry.rs", CLOSURE, &[6]);
}

#[test]
fn recovery_helper_declared_by_a_macro_is_rejected() {
    check("src/store.rs", MACRO_DEFINITION, &[7]);
}

#[test]
fn recovery_helper_passed_to_a_macro_is_rejected() {
    check("src/register.rs", MACRO_ARGUMENT, &[12]);
}

#[test]
fn owner_calls_and_test_code_pass() {
    check(
        "src/lib.rs",
        r"//! Agent.

use std::sync::{Mutex, RwLock};

/// Reads the retained queue through the owner.
fn held<T>(queue: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    maestro_lock::read(queue)
}

/// Holds the retained state for one step.
fn ignored(state: &Mutex<u8>) {
    let _ = maestro_lock::lock(state);
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, PoisonError};

    /// Locks the test's own state.
    fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
        value.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
",
        &[],
    );
}
