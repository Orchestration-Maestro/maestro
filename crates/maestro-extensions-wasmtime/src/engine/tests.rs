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

/// Builds Wasmtime's own engine for the component model, with no target named.
fn default_engine() -> Engine {
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    Engine::new(&config).unwrap()
}

/// On x86-64 the engine matches one that names its host triple, the baseline feature level.
#[test]
#[cfg(target_arch = "x86_64")]
fn engine_matches_an_explicit_baseline_target() {
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    let host = default_engine().get_target().unwrap();
    config.target(&host).unwrap();
    let explicit = Engine::new(&config).unwrap();
    assert_eq!(
        identity(&component_engine(None).unwrap()),
        identity(&explicit)
    );
}

/// Other architectures keep Wasmtime's defaults, which tune code to the host's CPU features.
#[test]
#[cfg(not(target_arch = "x86_64"))]
fn engine_keeps_wasmtime_defaults_off_x86_64() {
    assert_eq!(
        identity(&component_engine(None).unwrap()),
        identity(&default_engine())
    );
}
