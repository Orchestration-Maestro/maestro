//! The local executor owns the futures a caller hands it.
#![cfg(not(target_arch = "wasm32"))]

use std::cell::Cell;
use std::rc::Rc;

use maestro_test_process::executor::{block_on, run};

#[test]
fn the_local_executor_drives_local_tasks_to_completion() {
    let completed = Rc::new(Cell::new(false));
    let local = Rc::clone(&completed);

    run(async move {
        let task = tokio::task::spawn_local(async move { local.set(true) });
        task.await.expect("the local task completes");
    });

    assert!(completed.get(), "the local task ran to completion");
}

#[test]
fn the_local_executor_returns_the_future_output_after_its_local_tasks() {
    let output = block_on(async {
        tokio::task::spawn_local(async { 7 })
            .await
            .expect("the local task completes")
    });

    assert_eq!(output.expect("the local runtime builds"), 7);
}
