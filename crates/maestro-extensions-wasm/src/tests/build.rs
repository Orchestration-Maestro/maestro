//! Builds and compiles the author component for test harnesses.
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use wasmtime::component::Component;
use wasmtime::{Cache, Config, Engine};

#[path = "build/cargo.rs"]
pub mod cargo;

/// The example target of the shared author extension.
#[cfg(test)]
const AUTHOR_EXAMPLE: &str = cargo::FIXED_EXAMPLES[0];

/// The component built from the shared author extension.
///
/// Nextest supplies the artifact its setup example prepared; plain Cargo builds once per process.
///
/// # Errors
/// Returns the build's failure, which names the target to install when it is missing.
#[cfg(test)]
pub fn author_component() -> Result<&'static Path, String> {
    static BUILT: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    BUILT
        .get_or_init(|| example_component(AUTHOR_EXAMPLE))
        .as_deref()
        .map_err(Clone::clone)
}

/// Builds one registered example and finds the component artifact it reports, unless its
/// artifact is supplied by the environment.
///
/// # Errors
/// Returns the build's failure, which names the target to install when it is missing.
#[cfg(test)]
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

/// Compiles each component through the test host's cache on a few threads.
///
/// # Errors
/// Returns a compile failure, or a message when a worker thread panicked.
pub fn warm(paths: &[PathBuf]) -> Result<(), String> {
    warm_with(paths, |path| {
        compile(path).map(drop).map_err(|error| error.to_string())
    })
}

/// Runs `work` once for every path from one shared queue, on a few threads.
///
/// # Errors
/// Returns a worker's failure, or a message when a worker thread panicked.
pub fn warm_with<F>(paths: &[PathBuf], work: F) -> Result<(), String>
where
    F: Fn(&Path) -> Result<(), String> + Sync,
{
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(4);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| scope.spawn(|| drain(paths, &next, &work)))
            .collect();
        workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|_| Err("a warming thread panicked".to_owned()))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .collect()
    })
}

/// Takes paths from the shared queue until none remain or `work` fails.
fn drain<F>(paths: &[PathBuf], next: &AtomicUsize, work: &F) -> Result<(), String>
where
    F: Fn(&Path) -> Result<(), String>,
{
    while let Some(path) = paths.get(next.fetch_add(1, Ordering::Relaxed)) {
        work(path)?;
    }
    Ok(())
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
