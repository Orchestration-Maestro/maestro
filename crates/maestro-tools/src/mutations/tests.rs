use super::{QUEUES, queue_key, with_file_mutation_queue};
use maestro_models::{BoxFuture, DiagnosticErrorInfo};
use std::{
    sync::{Arc, Weak},
    task::{Context, Poll, Waker},
};

/// Whether the file has a live queue.
fn has_queue(path: &str) -> bool {
    QUEUES
        .lock()
        .unwrap()
        .contains_key(&queue_key(path).unwrap())
}

/// Drive `future` to completion without a scheduler.
fn finish<T>(mut future: BoxFuture<T>) -> T {
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("future is pending"),
    }
}

#[test]
fn mutation_queue_releases_waiter_ownership_when_empty() {
    let owned = std::env::temp_dir().join(format!("maestro-queue-unit-{}", std::process::id()));
    let path = owned.to_string_lossy().into_owned();
    let state = Arc::new(());
    let witness: Weak<()> = Arc::downgrade(&state);

    let held = Arc::clone(&state);
    let success = with_file_mutation_queue(&path, move || async move {
        drop(held);
        Ok(())
    });
    assert!(has_queue(&path));
    finish(success).unwrap();
    assert!(!has_queue(&path));

    let held = Arc::clone(&state);
    let failure = with_file_mutation_queue(&path, move || async move {
        drop(held);
        Err::<(), _>(DiagnosticErrorInfo {
            name: None,
            message: "failed".into(),
            stack: None,
            code: None,
        })
    });
    finish(failure).unwrap_err();
    assert!(!has_queue(&path));

    let held = Arc::clone(&state);
    let unpolled = with_file_mutation_queue(&path, move || async move {
        drop(held);
        Ok(())
    });
    assert!(has_queue(&path));
    drop(unpolled);
    assert!(!has_queue(&path));
    drop(state);
    assert!(witness.upgrade().is_none());
}
