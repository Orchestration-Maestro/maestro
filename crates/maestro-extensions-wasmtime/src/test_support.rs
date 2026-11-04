//! Controlled-test support shared by the engine's component and host witnesses.

use std::cell::RefCell;

use tokio::sync::{Notify, mpsc, oneshot};

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
    /// Every distinct release the application reported.
    pub labels: RefCell<Vec<String>>,
    /// Wakes the waiter when a release arrives.
    pub changed: Notify,
}

impl Releases {
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
