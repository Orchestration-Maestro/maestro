//! Test support over the production engine: a harness that loads built components with the
//! scripted provider, and the gates and receipts the engine's own witnesses await.
//!
//! [`maestro_extensions`] and the scripted provider's types are re-exported, so a package test
//! names the runner, its events and the provider's scripts through this crate alone.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;

use tokio::sync::{Notify, mpsc, oneshot};

mod harness;
pub use harness::{
    Harness, HarnessOptions, Loaded, unconfigured_command_operations, unconfigured_context_actions,
};
pub use maestro_extensions;
pub use maestro_extensions::test_support::{
    FauxModelDefinition, FauxProviderRegistration, FauxResponseStep, ToolkitWidgets,
};

/// Runs `test` to completion on a current-thread runtime inside a local task set, which loaded
/// components need for their native work.
///
/// # Errors
/// Returns the runtime's construction failure.
pub fn local<T>(test: impl Future<Output = T>) -> std::io::Result<T> {
    let runtime = tokio::runtime::Builder::new_current_thread().build()?;
    Ok(tokio::task::LocalSet::new().block_on(&runtime, test))
}

/// Release a waiting callback only after the test observed it enter.
///
/// # Panics
/// Panics when the observer is gone or never resumes the gate.
pub async fn gate(
    entered: &mpsc::UnboundedSender<(&'static str, oneshot::Sender<()>)>,
    phase: &'static str,
) {
    let (resume, done) = oneshot::channel();
    assert!(
        entered.send((phase, resume)).is_ok(),
        "the observer receives each phase"
    );
    assert!(done.await.is_ok(), "the observer resumes the gate");
}

/// Release receipts a test awaits, without timers or scheduler assumptions.
#[derive(Default)]
pub struct Releases {
    /// Every release receipt the application reported, in arrival order.
    labels: RefCell<Vec<String>>,
    /// Wakes the waiter when a release arrives.
    changed: Notify,
}

impl Releases {
    /// Record one release receipt and wake the waiter.
    pub fn record(&self, label: String) {
        self.labels.borrow_mut().push(label);
        self.changed.notify_one();
    }

    /// Wait for positive release receipts and reject early, duplicate or foreign ones.
    ///
    /// # Panics
    /// Panics when the receipts differ from `expected`.
    pub async fn expect(&self, expected: &[&str]) {
        while self.labels.borrow().len() < expected.len() {
            self.changed.notified().await;
        }
        let mut actual = self.labels.borrow().clone();
        actual.sort();
        assert_eq!(actual, expected);
    }
}
