//! Warming compiles every path once and reports a worker's failure instead of hiding it.
use std::{
    ffi::OsStr,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use super::build::{compile, warm, warm_outside_cache_run, warm_with};
use maestro_test_directory::Scratch;

/// Every queued path reaches the worker exactly once, whichever thread takes it.
#[test]
fn warming_visits_each_path_once() {
    let paths: Vec<PathBuf> = (0..9)
        .map(|index| PathBuf::from(format!("{index}")))
        .collect();
    let visited = AtomicUsize::new(0);
    warm_with(&paths, |_| {
        visited.fetch_add(1, Ordering::Relaxed);
        Ok(())
    })
    .unwrap();
    assert_eq!(visited.load(Ordering::Relaxed), 9);
}

/// A worker's own message is what the caller receives.
#[test]
fn warming_returns_a_worker_failure() {
    let paths = [PathBuf::from("first"), PathBuf::from("second")];
    let error = warm_with(&paths, |path| Err(format!("failed {}", path.display()))).unwrap_err();
    assert!(error.starts_with("failed "), "{error}");
}

/// A panicking worker becomes an error rather than unwinding through the caller.
#[test]
fn warming_reports_a_worker_panic() {
    let paths = [PathBuf::from("only")];
    let error = warm_with(&paths, |_| panic!("worker failed")).unwrap_err();
    assert_eq!(error, "a warming thread panicked");
}

/// The compile failure of a path that is not a component reaches the caller unchanged.
#[test]
fn warming_a_path_that_is_not_a_component_returns_the_compile_error() {
    let scratch = Scratch::new().unwrap();
    let path = scratch.path().join("core-module.wasm");
    std::fs::write(&path, b"\0asm\x01\0\0\0").unwrap();
    let expected = compile(&path).unwrap_err().to_string();
    assert_eq!(warm(&[path]).unwrap_err(), expected);
}

/// The compile-cache warm-up run compiles nothing, because it throws the warm away; every
/// other run still warms.
#[test]
fn warming_is_skipped_only_in_the_compile_cache_warm_up_run() {
    let missing = [PathBuf::from("absent.wasm")];
    assert_eq!(
        warm_outside_cache_run(&missing, Some(OsStr::new("true"))),
        Ok(())
    );
    for value in [None, Some(OsStr::new("false")), Some(OsStr::new(""))] {
        assert!(warm_outside_cache_run(&missing, value).is_err());
    }
}
