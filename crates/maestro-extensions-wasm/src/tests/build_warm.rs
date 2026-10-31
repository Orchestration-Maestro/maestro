//! Warming compiles every path once and reports a worker's failure instead of hiding it.
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use super::build::{compile, engine, warm, warm_with};
use maestro_test_directory::Scratch;
use std::hash::{DefaultHasher, Hash, Hasher};
use wasmtime::{Config, Engine};

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

/// The compiled-code identity of the test engine is the explicit baseline target's, so the
/// components a setup compiles load on any host of the architecture.
#[test]
fn test_engine_matches_an_explicit_baseline_target() {
    let identity = |engine: &Engine| {
        let mut hasher = DefaultHasher::new();
        engine.precompile_compatibility_hash().hash(&mut hasher);
        hasher.finish()
    };
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    let host = Engine::new(&config).unwrap().get_target().unwrap();
    config.target(&host).unwrap();
    let explicit = Engine::new(&config).unwrap();
    assert_eq!(identity(&engine().unwrap()), identity(&explicit));
}
