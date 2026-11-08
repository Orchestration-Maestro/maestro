//! The one engine configuration every component engine is built from.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use wasmtime::{Cache, CacheConfig, Config, Engine, Error, Result};

/// The directory below the agent directory that holds compiled components.
const COMPONENT_CACHE: &str = "component-cache";

/// The engine every loaded component in this process reuses.
static PROCESS_ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();

/// Creates the async component engine, with `cache` as its compiled-code store.
///
/// On x86-64 the engine compiles for the host's triple at its baseline feature level, so the
/// compiled code and its cache keys are the same on every x86-64 host. Wasmtime tunes code to the
/// exact CPU only when no target is named. Other architectures keep Wasmtime's defaults.
///
/// # Errors
/// Returns Wasmtime's failure to configure the engine.
pub fn component_engine(cache: Option<Cache>) -> wasmtime::Result<Engine> {
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    if cfg!(target_arch = "x86_64")
        && let Some(host) = Engine::new(&config)?.get_target()
    {
        config.target(&host)?;
    }
    config.cache(cache);
    Engine::new(&config)
}

/// The directory that holds the compiled components of the installation in `agent_dir`.
#[must_use]
pub fn component_cache_directory(agent_dir: &str) -> PathBuf {
    Path::new(agent_dir).join(COMPONENT_CACHE)
}

/// Opens the compiled-code cache that lives in `agent_dir`.
///
/// Every platform keeps the cache in the same place, below the agent directory, so a run that
/// moves the agent directory moves its cache with it. Returns `None` when the directory cannot
/// be used, which leaves the engine without a cache.
#[must_use]
pub fn component_cache(agent_dir: &str) -> Option<Cache> {
    let mut config = CacheConfig::new();
    config.with_directory(component_cache_directory(agent_dir));
    Cache::new(config).ok()
}

/// Returns the one engine every loaded component in this process reuses.
///
/// A component keeps its own workers, stores and instances; the engine itself
/// holds no per-component state, so the process pays one engine setup.
/// The optional cache in `agent_dir` is configured once, as in the first load.
///
/// # Errors
/// Returns Wasmtime's failure to configure the engine.
pub(crate) fn process_engine(agent_dir: &str) -> Result<Engine> {
    let configured = PROCESS_ENGINE.get_or_init(|| {
        component_engine(component_cache(agent_dir)).map_err(|error| format!("{error:#}"))
    });
    configured.clone().map_err(Error::msg)
}

#[cfg(test)]
mod tests;
