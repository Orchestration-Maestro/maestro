use super::{QUEUES, queue_key, with_file_mutation_queue};
use crate::utils::diagnostics::blocking;
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

#[test]
fn cancelled_file_work_holds_the_queue_until_it_settles() {
    let owned = std::env::temp_dir().join(format!("maestro-queue-hold-{}", std::process::id()));
    let path = owned.to_string_lossy().into_owned();
    let journal = std::sync::Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));
    let (started, work_started) = tokio::sync::oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel::<()>();
    let work_journal = std::sync::Arc::clone(&journal);
    let cancelled = with_file_mutation_queue(&path, move || async move {
        blocking(move || {
            started.send(()).unwrap();
            gate.recv().unwrap();
            work_journal.lock().unwrap().push("work settled");
            Ok(())
        })
        .await
    });
    let successor_journal = std::sync::Arc::clone(&journal);
    let successor = with_file_mutation_queue(&path, move || async move {
        successor_journal.lock().unwrap().push("successor started");
        Ok(())
    });
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let task = tokio::spawn(cancelled);
            work_started.await.unwrap();
            task.abort();
            let _ = task.await;
            let mut successor = tokio::spawn(successor);
            tokio::task::yield_now().await;
            let before_settling = journal.lock().unwrap().clone();
            release.send(()).unwrap();
            (&mut successor).await.unwrap().unwrap();
            assert!(before_settling.is_empty());
        });
    assert_eq!(
        *journal.lock().unwrap(),
        ["work settled", "successor started"]
    );
}
