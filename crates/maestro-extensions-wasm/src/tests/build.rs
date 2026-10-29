//! Builds and compiles the author component for test harnesses.
use std::path::Path;

use wasmtime::component::Component;
use wasmtime::{Cache, Config, Engine};

#[path = "build/components.rs"]
mod components;
pub use components::{author_component, component, example_component};

/// Compiles the component with the test host's engine and optional user cache.
///
/// # Errors
/// Returns Wasmtime's failure to configure the engine or to compile the component.
pub fn compile(path: &Path) -> wasmtime::Result<(Engine, Component)> {
    let engine = engine()?;
    let component = Component::from_file(&engine, path)?;
    Ok((engine, component))
}

/// Creates an async component engine with the optional user cache.
///
/// # Errors
/// Returns Wasmtime's failure to configure the engine.
pub fn engine() -> wasmtime::Result<Engine> {
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.cache(Cache::from_file(None).ok());
    Engine::new(&config)
}
