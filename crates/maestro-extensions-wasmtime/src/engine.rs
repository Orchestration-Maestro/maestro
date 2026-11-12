//! The one engine configuration and host link table every component is built from.
use crate::lifecycle::State;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use wasmtime::component::Linker;
use wasmtime::{Cache, CacheConfig, Config, Engine, Error, Result};

/// The directory below the agent directory that holds compiled components.
const COMPONENT_CACHE: &str = "component-cache";

/// The engine every loaded component in this process reuses.
static PROCESS_ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();

/// The host link table every loaded component in this process instantiates from.
static PROCESS_LINKER: OnceLock<Result<Linker<State>, String>> = OnceLock::new();

/// Creates the async component engine, with `cache` as its compiled-code store.
///
/// The engine compiles for the host's triple at its baseline feature level, so the compiled code
/// and its cache keys are the same on every host of one architecture. Wasmtime tunes code to the
/// exact CPU only when no target is named.
///
/// # Errors
/// Returns Wasmtime's failure to configure the engine.
pub fn component_engine(cache: Option<Cache>) -> wasmtime::Result<Engine> {
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    if let Some(host) = Engine::new(&config)?.get_target() {
        config.target(&host)?;
    }
    config.cache(cache);
    Engine::new(&config)
}

/// The directory that holds the compiled components of the installation in `agent_dir`.
#[must_use]
pub fn component_cache_directory(agent_dir: impl AsRef<Path>) -> PathBuf {
    agent_dir.as_ref().join(COMPONENT_CACHE)
}

/// Opens the compiled-code cache that lives in `agent_dir`.
///
/// Every platform keeps the cache in the same place, below the agent directory, so a run that
/// moves the agent directory moves its cache with it. A relative agent directory resolves against
/// the working directory, as every other path below it does. Returns `None` when the directory
/// cannot be used, which leaves the engine without a cache.
#[must_use]
pub fn component_cache(agent_dir: &str) -> Option<Cache> {
    let mut config = CacheConfig::new();
    config.with_directory(component_cache_directory(
        std::path::absolute(agent_dir).ok()?,
    ));
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

/// Returns the one host link table every instance in this process instantiates from.
///
/// The table is bound to the process engine and registers every host function once; each
/// instance then only instantiates its component against it.
///
/// # Errors
/// Returns the failure to configure the engine or to register the host functions.
pub(crate) fn process_linker(agent_dir: &str) -> Result<&'static Linker<State>> {
    let linked = PROCESS_LINKER.get_or_init(|| {
        let engine = process_engine(agent_dir).map_err(|error| format!("{error:#}"))?;
        let mut linker = Linker::new(&engine);
        crate::lifecycle::link(&mut linker).map_err(|error| format!("{error:#}"))?;
        Ok(linker)
    });
    linked.as_ref().map_err(|error| Error::msg(error.clone()))
}

#[cfg(test)]
mod tests;
