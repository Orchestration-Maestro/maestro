//! The engine compiles the same code on every host and loads modules that
//! throw WebAssembly exceptions.
use super::{component_cache, component_engine, process_engine};
use std::hash::{DefaultHasher, Hash, Hasher};
use wasmtime::{Config, Engine, Module};

/// A module whose one function throws a declared exception tag.
///
/// Sections, in their required order: a `() -> ()` type, a function of that
/// type, one tag of that type, and a body that throws tag 0.
const EXCEPTION_MODULE: &[u8] = b"\x00asm\x01\x00\x00\x00\
\x01\x04\x01\x60\x00\x00\
\x03\x02\x01\x00\
\x0d\x03\x01\x00\x00\
\x0a\x06\x01\x04\x00\x08\x00\x0b";

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

/// The engine matches one that names its host triple, the baseline feature level.
#[test]
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

/// The component engine loads code that raises a WebAssembly exception.
#[test]
fn engine_loads_webassembly_exceptions() {
    let engine = component_engine(None).unwrap();
    Module::new(&engine, EXCEPTION_MODULE).unwrap();
}

/// The accessor returns the same engine on every call.
#[test]
fn process_engine_reuses_one_engine() {
    let first = process_engine("").unwrap();
    let second = process_engine("").unwrap();
    assert!(Engine::same(&first, &second));
}

/// The cache sits in `component-cache` below the agent directory, which it creates.
#[test]
fn component_cache_sits_below_the_agent_directory() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let agent = scratch.path().join("agent");
    let cache = component_cache(agent.to_str().unwrap()).unwrap();
    assert_eq!(
        cache.directory(),
        &agent.canonicalize().unwrap().join("component-cache")
    );
}

/// A directory the engine cannot use leaves the engine without a cache.
#[test]
fn component_cache_is_absent_without_a_usable_agent_directory() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let file = scratch.path().join("file");
    std::fs::write(&file, "not a directory").unwrap();
    for agent in ["", file.to_str().unwrap()] {
        assert!(component_cache(agent).is_none(), "{agent:?}");
    }
}

/// Returns the process to the directory it found, so a failing test leaves no working directory
/// behind for its scratch root to clean up.
struct RestoringDirectory(std::path::PathBuf);

impl Drop for RestoringDirectory {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// A relative agent directory resolves against the working directory, as every other path below
/// the agent directory does, instead of leaving the engine without a cache.
#[test]
fn component_cache_resolves_a_relative_agent_directory_against_the_working_directory() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let _restored = RestoringDirectory(std::env::current_dir().unwrap());
    std::env::set_current_dir(scratch.path()).unwrap();
    let cache = component_cache("relative-agent").unwrap();
    assert_eq!(
        cache.directory(),
        &std::env::current_dir()
            .unwrap()
            .join("relative-agent")
            .canonicalize()
            .unwrap()
            .join("component-cache")
    );
}
