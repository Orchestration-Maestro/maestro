//! Controlled acquisition decisions.

use super::*;
use std::fs::TryLockError;
use std::time::Duration;

#[test]
fn retry_returns_success_at_each_boundary() {
    for success_at in [1, 2, 9, 10] {
        let mut attempts = 0;
        let mut waits = Vec::new();
        let result = retry_contended::<()>(
            || {
                attempts += 1;
                if attempts == success_at {
                    Ok(())
                } else {
                    Err(TryLockError::WouldBlock)
                }
            },
            |delay| waits.push(delay),
        );
        assert!(result.is_ok());
        assert_eq!(attempts, success_at);
        assert_eq!(waits, vec![Duration::from_millis(20); success_at - 1]);
    }
}

#[test]
fn retry_exhausts_on_tenth_contention() {
    let mut attempts = 0;
    let mut waits = Vec::new();
    let result = retry_contended::<()>(
        || {
            attempts += 1;
            if attempts == 11 {
                Ok(())
            } else {
                Err(TryLockError::WouldBlock)
            }
        },
        |delay| waits.push(delay),
    );
    assert!(matches!(result, Err(TryLockError::WouldBlock)));
    assert_eq!(attempts, 10);
    assert_eq!(waits, vec![Duration::from_millis(20); 9]);
}

#[test]
fn retry_preserves_non_contention_failure() {
    for failure_at in [1, 4, 10] {
        let mut attempts = 0;
        let mut waits = Vec::new();
        let payload = std::sync::Arc::new(17);
        let result = retry_contended::<()>(
            || {
                attempts += 1;
                if attempts == failure_at {
                    Err(TryLockError::Error(std::io::Error::other(Payload(
                        payload.clone(),
                    ))))
                } else {
                    Err(TryLockError::WouldBlock)
                }
            },
            |delay| waits.push(delay),
        );
        let Err(TryLockError::Error(error)) = result else {
            panic!("original error required")
        };
        let retained = error.get_ref().unwrap().downcast_ref::<Payload>().unwrap();
        assert!(std::sync::Arc::ptr_eq(&payload, &retained.0));
        assert_eq!(attempts, failure_at);
        assert_eq!(waits, vec![Duration::from_millis(20); failure_at - 1]);
    }
}

/// Identifiable native error payload.
#[derive(Debug)]
struct Payload(std::sync::Arc<u32>);

impl std::fmt::Display for Payload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("controlled failure")
    }
}
impl std::error::Error for Payload {}

/// Replace `shared`'s value with 9 and poison it by panicking on a write guard.
fn poison_rwlock(shared: &std::sync::Arc<std::sync::RwLock<i32>>) {
    let writer = std::sync::Arc::clone(shared);
    let _ = std::thread::spawn(move || {
        let mut guard = writer.write().unwrap();
        *guard = 9;
        panic!("writer panics while holding the write guard");
    })
    .join();
}

/// Replace `shared`'s value with 9 and poison it by panicking on its mutex guard.
fn poison_mutex(shared: &std::sync::Arc<std::sync::Mutex<i32>>) {
    let holder = std::sync::Arc::clone(shared);
    let _ = std::thread::spawn(move || {
        let mut guard = holder.lock().unwrap();
        *guard = 9;
        panic!("holder panics while holding the mutex guard");
    })
    .join();
}

/// A read finds the value the panicked writer retained and leaves the marker set.
#[test]
fn read_finds_the_value_a_panicking_writer_left() {
    let shared = std::sync::Arc::new(std::sync::RwLock::new(7));
    assert_eq!(*read(&shared), 7);
    assert!(!shared.is_poisoned());
    poison_rwlock(&shared);
    assert!(shared.is_poisoned());
    assert_eq!(*read(&shared), 9);
    assert!(shared.is_poisoned());
}

/// A write after a panicked writer lands for the next read and leaves the marker set.
#[test]
fn write_lands_after_a_panicking_writer() {
    let shared = std::sync::Arc::new(std::sync::RwLock::new(1));
    *write(&shared) = 2;
    assert_eq!(*read(&shared), 2);
    poison_rwlock(&shared);
    assert!(shared.is_poisoned());
    assert_eq!(*write(&shared), 9);
    assert!(shared.is_poisoned());
    *write(&shared) = 3;
    assert_eq!(*read(&shared), 3);
    assert!(shared.is_poisoned());
}

/// A lock finds what the panicked holder left, leaves the marker set and can replace it.
#[test]
fn lock_finds_the_value_a_panicking_holder_left() {
    let shared = std::sync::Arc::new(std::sync::Mutex::new(3));
    assert_eq!(*lock(&shared), 3);
    assert!(!shared.is_poisoned());
    poison_mutex(&shared);
    assert!(shared.is_poisoned());
    assert_eq!(*lock(&shared), 9);
    assert!(shared.is_poisoned());
    *lock(&shared) = 4;
    assert_eq!(*lock(&shared), 4);
    assert!(shared.is_poisoned());
}

/// Every operation accepts state that borrows from its caller.
#[test]
fn acquisition_keeps_borrowed_values() {
    let text = String::from("borrowed");
    let shared = std::sync::RwLock::new(text.as_str());
    assert_eq!(*read(&shared), "borrowed");
    *write(&shared) = &text[..4];
    assert_eq!(*read(&shared), "borr");
    let mutex = std::sync::Mutex::new(vec![text.as_str()]);
    lock(&mutex).push(&text[..7]);
    assert_eq!(*lock(&mutex), vec!["borrowed", "borrowe"]);
}
