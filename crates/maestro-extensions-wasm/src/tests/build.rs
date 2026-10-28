//! Builds and compiles the author component for test harnesses.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use wasmtime::component::Component;
use wasmtime::{Cache, Config, Engine};

#[path = "build/cargo.rs"]
mod cargo;

/// The component built from the shared author extension.
///
/// Nextest supplies the artifact its setup example prepared; plain Cargo builds once per process.
/// The compiler wrapper is not passed on, because coverage would instrument the component target.
///
/// # Errors
/// Returns the build's failure, which names the target to install when it is missing.
pub fn author_component() -> Result<&'static Path, String> {
    static BUILT: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    BUILT
        .get_or_init(|| component("author_component", "MAESTRO_AUTHOR_COMPONENT"))
        .as_deref()
        .map_err(Clone::clone)
}

/// Builds a named author example, unless its artifact is supplied by the environment.
///
/// # Errors
/// Returns build failure or a diagnostic when Cargo reports no component artifact.
pub fn component(example: &str, environment: &str) -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(environment) {
        return Ok(PathBuf::from(path));
    }
    example_component(example)
}

/// Builds one registered example and finds the component artifact it reports.
///
/// # Errors
/// Returns the build's failure, which names the target to install when it is missing.
pub fn example_component(name: &str) -> Result<PathBuf, String> {
    cargo::component(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &["--example", name],
        name,
    )
}

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
