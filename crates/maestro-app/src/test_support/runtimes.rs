//! Current-thread runtimes that complete a test body inside a local task set.
use std::{future::Future, io, rc::Rc};
use tokio::{runtime::Builder, task::LocalSet};

/// Completes `body` on a fresh current-thread runtime inside a task set it receives.
///
/// # Errors
/// Returns the failure to build the runtime.
pub fn on_task_set<F: Future>(body: impl FnOnce(Rc<LocalSet>) -> F) -> io::Result<F::Output> {
    run(Builder::new_current_thread().enable_all(), body)
}

/// Completes `body` on a fresh current-thread runtime with a paused clock, inside a task set
/// it receives; the clock moves when a test advances it or when every task waits on a timer.
///
/// # Errors
/// Returns the failure to build the runtime.
pub fn on_paused_task_set<F: Future>(
    body: impl FnOnce(Rc<LocalSet>) -> F,
) -> io::Result<F::Output> {
    run(
        Builder::new_current_thread()
            .enable_all()
            .start_paused(true),
        body,
    )
}

/// Completes `body` on the runtime `builder` describes, inside a task set it receives.
fn run<F: Future>(
    builder: &mut Builder,
    body: impl FnOnce(Rc<LocalSet>) -> F,
) -> io::Result<F::Output> {
    let local = Rc::new(LocalSet::new());
    let runtime = builder.build()?;
    Ok(runtime.block_on(local.run_until(body(Rc::clone(&local)))))
}
