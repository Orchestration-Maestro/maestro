//! The one engine configuration every component engine is built from.
use wasmtime::{Cache, Config, Engine};

/// Creates the async component engine, with `cache` as its compiled-code store.
///
/// # Errors
/// Returns Wasmtime's failure to configure the engine.
pub fn component_engine(cache: Option<Cache>) -> wasmtime::Result<Engine> {
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.cache(cache);
    Engine::new(&config)
}

#[cfg(test)]
mod tests;
