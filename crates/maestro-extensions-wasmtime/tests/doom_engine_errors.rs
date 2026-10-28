//! Engine errors returned through the composed component interface.
#![cfg(test)]
use wasmtime::{
    Config, Engine, Store,
    component::{Component, Linker, ResourceTable},
};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

/// Standard interfaces retained by one isolated engine.
struct State {
    /// Original standard-interface resources.
    table: ResourceTable,
    /// Standard capabilities with no inherited terminal output.
    wasi: WasiCtx,
}
impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// A fatal engine call unwinds only to its export and leaves later calls usable.
#[test]
fn engine_fatal_error_returns_without_poisoning_instance() -> wasmtime::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let mut config = Config::new();
            config.wasm_component_model_async(true);
            let engine = Engine::new(&config)?;
            let component =
                Component::from_binary(&engine, include_bytes!("fixtures/doom-sjlj-engine.wasm"))?;
            let mut linker = Linker::new(&engine);
            wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
            let mut store = Store::new(
                &engine,
                State {
                    table: ResourceTable::new(),
                    wasi: WasiCtxBuilder::new().build(),
                },
            );
            let instance = linker.instantiate_async(&mut store, &component).await?;
            let (_, interface) = instance
                .get_export(&mut store, None, "maestro:doom/engine@0.1.0")
                .unwrap();
            let (_, export) = instance
                .get_export(&mut store, Some(&interface), "tick")
                .unwrap();
            let tick =
                instance.get_typed_func::<(u32,), (Result<(), String>,)>(&mut store, &export)?;
            let (first,) = store
                .run_concurrent(async |accessor| tick.call_concurrent(accessor, (28,)).await)
                .await??;
            assert_eq!(first, Err("forced engine failure: 7".into()));
            let (second,) = store
                .run_concurrent(async |accessor| tick.call_concurrent(accessor, (57,)).await)
                .await??;
            assert_eq!(second, Ok(()));
            Ok(())
        })
}

mod real;
