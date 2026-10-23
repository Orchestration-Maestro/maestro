//! Lock recovery after a panicking holder.

use super::{lock, read, write};
use std::sync::{Arc, Mutex, RwLock};

/// Poisons `shared` by panicking while the writer holds its write guard.
fn poison_rwlock(shared: &Arc<RwLock<i32>>) {
    let writer = Arc::clone(shared);
    let _ = std::thread::spawn(move || {
        let _guard = writer.write().unwrap();
        panic!("writer panics while holding the write guard");
    })
    .join();
}

/// Poisons `shared` by panicking while the holder owns its mutex guard.
fn poison_mutex(shared: &Arc<Mutex<i32>>) {
    let holder = Arc::clone(shared);
    let _ = std::thread::spawn(move || {
        let _guard = holder.lock().unwrap();
        panic!("holder panics while holding the mutex guard");
    })
    .join();
}

/// A read finds the value the panicked writer retained.
#[test]
fn read_returns_the_retained_value_after_a_panicking_writer() {
    let shared = Arc::new(RwLock::new(7));
    assert_eq!(*read(&shared), 7);
    poison_rwlock(&shared);
    assert_eq!(*read(&shared), 7);
}

/// A write after a panicked writer lands for the next read.
#[test]
fn write_reaches_the_retained_state_after_a_panicking_writer() {
    let shared = Arc::new(RwLock::new(1));
    *write(&shared) = 2;
    assert_eq!(*read(&shared), 2);
    poison_rwlock(&shared);
    *write(&shared) = 3;
    assert_eq!(*read(&shared), 3);
}

/// A lock finds the value the panicked holder retained and can replace it.
#[test]
fn lock_returns_the_retained_value_after_a_panicking_holder() {
    let shared = Arc::new(Mutex::new(3));
    assert_eq!(*lock(&shared), 3);
    poison_mutex(&shared);
    *lock(&shared) = 4;
    assert_eq!(*lock(&shared), 4);
}
