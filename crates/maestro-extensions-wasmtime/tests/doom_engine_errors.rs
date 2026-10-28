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
    check_errors(vec![(28, "forced engine failure: 7".into())])
}

/// Invalid leading bytes and malformed scalar encodings remain recoverable diagnostics.
#[test]
fn engine_invalid_diagnostics_return_without_poisoning_instance() -> wasmtime::Result<()> {
    let invalid = b"\xff\xc0\xaf\xe0\x80\xaf\xed\xa0\x80\xf0\x80\x80\xaf\xf4\x90\x80\x80\xe2\x82x";
    check_errors(vec![
        (29, "invalid:\u{fffd}".into()),
        (32, format!("sequence:{}", String::from_utf8_lossy(invalid))),
        (33, "incomplete:\u{fffd}".into()),
    ])
}

/// A diagnostic cut within a three-byte scalar ends at the preceding character boundary.
#[test]
fn engine_truncated_diagnostic_returns_without_poisoning_instance() -> wasmtime::Result<()> {
    check_errors(vec![(30, "a".repeat(1021))])
}

/// Complete two-, three- and four-byte scalar encodings are unchanged.
#[test]
fn engine_valid_multibyte_diagnostic_is_preserved() -> wasmtime::Result<()> {
    check_errors(vec![(31, "valid:\u{e9}\u{20ac}\u{10000}\u{10ffff}".into())])
}

/// Every error vector is followed by a successful call on the same instance.
fn check_errors(cases: Vec<(u32, String)>) -> wasmtime::Result<()> {
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
            for (milliseconds, expected) in cases {
                let (first,) = store
                    .run_concurrent(async |accessor| {
                        tick.call_concurrent(accessor, (milliseconds,)).await
                    })
                    .await??;
                assert_eq!(first, Err(expected));
                let (second,) = store
                    .run_concurrent(async |accessor| tick.call_concurrent(accessor, (57,)).await)
                    .await??;
                assert_eq!(second, Ok(()));
            }
            Ok(())
        })
}

mod real;

/// The external build recipe validates and forwards both resolution overrides.
#[test]
fn doom_engine_builder_preserves_resolution_overrides() {
    let output = std::process::Command::new("python3")
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/check_doom_builder.py"),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
