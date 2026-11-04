//! The scenario driver the Windows terminal witnesses share.
#![cfg(test)]
#![cfg(windows)]

use std::future::Future;
use std::rc::Rc;

use tokio::runtime::Builder;
use tokio::task::LocalSet;

/// Runs `scenario` on a current-thread runtime with a real clock and a local set.
pub fn drive<F: Future<Output = ()>>(scenario: impl FnOnce(Rc<LocalSet>) -> F) {
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let local = Rc::new(LocalSet::new());
    runtime.block_on(local.run_until(scenario(Rc::clone(&local))));
}
