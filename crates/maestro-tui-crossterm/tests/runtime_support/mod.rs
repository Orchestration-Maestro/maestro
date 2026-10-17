//! Shared helpers for the native runtime tests: a driven local set and child-process reruns.
#![cfg(test)]

use std::future::Future;
use std::process::Output;
use std::rc::Rc;

use maestro_test_process::TestProcess;
use tokio::runtime::Builder;
use tokio::task::LocalSet;

/// Marks the child copy of a test, which runs its scenario instead of spawning itself.
pub const CHILD_MARKER: &str = "MAESTRO_RUNTIME_TEST_CHILD";

/// Drives `body` on a fresh local set inside a current-thread runtime with the time driver.
///
/// A paused clock advances by itself only while no task can run.
pub fn run_set<Fut: Future<Output = ()>>(paused: bool, body: impl FnOnce(Rc<LocalSet>) -> Fut) {
    let runtime = Builder::new_current_thread()
        .enable_time()
        .start_paused(paused)
        .build()
        .expect("runtime builds");
    let local = Rc::new(LocalSet::new());
    runtime.block_on(local.run_until(body(Rc::clone(&local))));
}

/// Runs the named test again in a child process and returns what it wrote.
///
/// The child must exit successfully after the checked owner proves it started exactly that
/// test, so a name that matches nothing fails here instead of passing silently, and its
/// standard output may hold only the test harness's own lines.
pub fn rerun(test: &str, setup: impl FnOnce(TestProcess) -> TestProcess) -> Output {
    let process = TestProcess::current(test).expect("running test identity");
    let process = setup(process.nocapture().env(CHILD_MARKER, "1"));
    let name = process.name().to_owned();
    let output = process.output().expect("child process starts the test");
    assert!(
        output.status.success(),
        "child {name} failed: {:?}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout)
    );
    for line in String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
    {
        assert!(
            line == "running 1 test"
                || line == format!("test {name} ... ok")
                || line.starts_with("test result: ok. 1 passed;"),
            "unexpected standard output from child {name}: {line:?}"
        );
    }
    output
}

/// Whether this process is the child copy of a test.
pub fn is_child() -> bool {
    std::env::var_os(CHILD_MARKER).is_some()
}
