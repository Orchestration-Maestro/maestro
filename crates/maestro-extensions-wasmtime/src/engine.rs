//! The one engine configuration every component engine is built from.
use wasmtime::{Cache, Config, Engine};

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

#[cfg(test)]
mod tests;
