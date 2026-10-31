//! The engine's compiled code does not depend on the CPU that builds it.
use super::component_engine;
use std::hash::{DefaultHasher, Hash, Hasher};
use wasmtime::{Config, Engine};

/// Reduces an engine's compiled-code compatibility identity to one number.
fn identity(engine: &Engine) -> u64 {
    let mut hasher = DefaultHasher::new();
    engine.precompile_compatibility_hash().hash(&mut hasher);
    hasher.finish()
}

#[test]
fn engine_matches_an_explicit_baseline_target() {
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    let host = Engine::new(&config).unwrap().get_target().unwrap();
    config.target(&host).unwrap();
    let explicit = Engine::new(&config).unwrap();
    assert_eq!(
        identity(&component_engine(None).unwrap()),
        identity(&explicit)
    );
}
