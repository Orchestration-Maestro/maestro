//! Builds and compiles the author component for test harnesses.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use wasmtime::component::Component;
use wasmtime::{Cache, CacheConfig, Config, Engine};

/// Build target of author components.
const TARGET: &str = "wasm32-wasip2";

/// The component built from the shared author extension.
///
/// Nextest supplies the artifact its setup example prepared; plain Cargo builds once per process.
/// The compiler wrapper is not passed on, because coverage would instrument the component target.
///
/// # Errors
/// Returns the build's failure, which names the target to install when it is missing.
pub fn author_component() -> Result<&'static Path, String> {
    static BUILT: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    BUILT.get_or_init(build).as_deref().map_err(Clone::clone)
}

/// Runs the author build and finds the artifact it reports.
fn build() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("MAESTRO_AUTHOR_COMPONENT") {
        return Ok(PathBuf::from(path));
    }
    example_component("author_component")
}

/// Builds one registered example and finds the component artifact it reports.
///
/// # Errors
/// Returns the build's failure, which names the target to install when it is missing.
pub fn example_component(name: &str) -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("RUSTC_WRAPPER")
        .args(["build", "--example", name, "--target", TARGET])
        .args(["--release", "--locked", "--message-format=json"])
        .output()
        .map_err(|error| format!("cannot run cargo: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "building the author component failed (is the {TARGET} target installed? run \
             `rustup target add {TARGET}`):\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .filter(|message| message["target"]["name"] == name)
        .flat_map(|message| message["filenames"].as_array().cloned().unwrap_or_default())
        .filter_map(|file| file.as_str().map(PathBuf::from))
        .find(|file| {
            file.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .ok_or_else(|| "the build reported no component".to_owned())
}

/// Compiles the component with the test host's engine, caching the result beside the artifact.
///
/// # Errors
/// Returns Wasmtime's failure to configure the cache or to compile the component.
pub fn compile(path: &Path) -> wasmtime::Result<(Engine, Component)> {
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    let mut cache = CacheConfig::new();
    cache.with_directory(path.with_extension("wasmtime-cache"));
    config.cache(Some(Cache::new(cache)?));
    let engine = Engine::new(&config)?;
    let component = Component::from_file(&engine, path)?;
    Ok((engine, component))
}
