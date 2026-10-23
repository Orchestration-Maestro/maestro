//! Lock recovery after a panicking holder.

use super::{lock, read, write};
use std::sync::{Arc, Mutex, RwLock};

/// A panicking writer leaves the value the host's `read` recovers.
#[test]
fn read_recovers_the_value_a_panicking_writer_left() {
    let shared = Arc::new(RwLock::new(5));
    assert_eq!(*read(&shared), 5);
    let writer = Arc::clone(&shared);
    let _ = std::thread::spawn(move || {
        let _guard = writer.write().unwrap();
        panic!("writer panics while holding the write guard");
    })
    .join();
    assert_eq!(*read(&shared), 5);
}

/// A write after a panicked writer lands for the host's next `read`.
#[test]
fn write_lands_after_a_panicking_writer() {
    let shared = Arc::new(RwLock::new(1));
    *write(&shared) = 2;
    assert_eq!(*read(&shared), 2);
    let writer = Arc::clone(&shared);
    let _ = std::thread::spawn(move || {
        let _guard = writer.write().unwrap();
        panic!("writer panics while holding the write guard");
    })
    .join();
    *write(&shared) = 3;
    assert_eq!(*read(&shared), 3);
}

/// A lock finds the value a panicked holder left in the mutex.
#[test]
fn lock_recovers_the_mutex_a_panicking_holder_left() {
    let shared = Arc::new(Mutex::new(9));
    assert_eq!(*lock(&shared), 9);
    let holder = Arc::clone(&shared);
    let _ = std::thread::spawn(move || {
        let _guard = holder.lock().unwrap();
        panic!("holder panics while holding the mutex guard");
    })
    .join();
    *lock(&shared) = 10;
    assert_eq!(*lock(&shared), 10);
}
