//! Opt-in qualification of an external engine with user-supplied game data.
use super::State;
use maestro_test_directory::Scratch;
use wasmtime::{
    Config, Engine, Store,
    component::{Component, Instance, Linker, ResourceTable},
};
use wasmtime_wasi::{FsPerms, WasiCtxBuilder};

/// Runs the external engine without inherited standard streams or filesystem roots.
#[test]
#[ignore = "requires MAESTRO_DOOM_ENGINE and MAESTRO_DOOM_WAD external files"]
fn real_engine_initializes_and_advances_frames() -> wasmtime::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

/// Loads the explicit opt-in files into an isolated standard-interface store.
async fn run() -> wasmtime::Result<()> {
    let root = Scratch::new()?;
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    let engine = Engine::new(&config)?;
    let component = Component::from_file(&engine, std::env::var("MAESTRO_DOOM_ENGINE")?)?;
    let mut wasi = WasiCtxBuilder::new();
    wasi.preopened_dir(root.path(), "/private", FsPerms::ReadWrite)?;
    let mut store = Store::new(
        &engine,
        State {
            table: ResourceTable::new(),
            wasi: wasi.build(),
        },
    );
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
    let invalid = linker.instantiate_async(&mut store, &component).await?;
    let failure = initialize(&mut store, &invalid, b"invalid WAD".to_vec()).await?;
    assert!(failure.is_err(), "invalid data must return an engine error");
    let instance = linker.instantiate_async(&mut store, &component).await?;
    let wad = std::fs::read(std::env::var("MAESTRO_DOOM_WAD")?)?;
    assert_eq!(initialize(&mut store, &instance, wad).await?, Ok(()));
    advance(&mut store, &instance).await?;
    assert!(root.path().join("doom1.wad").is_file());
    assert!(root.path().join("saves").is_dir());
    Ok(())
}

/// Resolves one engine function through its public interface.
fn export(
    store: &mut Store<State>,
    instance: &Instance,
    name: &str,
) -> wasmtime::component::ComponentExportIndex {
    let (_, interface) = instance
        .get_export(&mut *store, None, "maestro:doom/engine@0.1.0")
        .unwrap();
    instance
        .get_export(store, Some(&interface), name)
        .unwrap()
        .1
}

/// Starts with explicit bytes and a root that is the only writable capability.
async fn initialize(
    store: &mut Store<State>,
    instance: &Instance,
    wad: Vec<u8>,
) -> wasmtime::Result<Result<(), String>> {
    let index = export(store, instance, "init");
    let init =
        instance.get_typed_func::<(Vec<u8>, String), (Result<(), String>,)>(&mut *store, &index)?;
    let (result,) = store
        .run_concurrent(async |accessor| {
            init.call_concurrent(accessor, (wad, "/private".into()))
                .await
        })
        .await??;
    Ok(result)
}

/// The virtual clock must pass waiting loops and produce changing opaque RGBA frames.
async fn advance(store: &mut Store<State>, instance: &Instance) -> wasmtime::Result<()> {
    let index = export(store, instance, "dimensions");
    let dimensions = instance.get_typed_func::<(), ((u32, u32),)>(&mut *store, &index)?;
    let ((width, height),) = store
        .run_concurrent(async |accessor| dimensions.call_concurrent(accessor, ()).await)
        .await??;
    assert!(width > 0 && height > 0);
    let bytes = usize::try_from(
        width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .unwrap(),
    )?;
    let index = export(store, instance, "tick");
    let tick = instance.get_typed_func::<(u32,), (Result<(), String>,)>(&mut *store, &index)?;
    let index = export(store, instance, "frame");
    let frame = instance.get_typed_func::<(), (Vec<u8>,)>(&mut *store, &index)?;
    let mut first = Vec::new();
    let mut changed = false;
    for count in 1..=210 {
        let (result,) = store
            .run_concurrent(async |accessor| {
                tick.call_concurrent(accessor, (count * 1000 / 35,)).await
            })
            .await??;
        assert_eq!(result, Ok(()));
        let (pixels,) = store
            .run_concurrent(async |accessor| frame.call_concurrent(accessor, ()).await)
            .await??;
        assert_eq!(pixels.len(), bytes);
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255)
        );
        if first.is_empty() {
            first = pixels;
        } else {
            changed |= pixels != first;
        }
    }
    assert!(first.iter().any(|value| *value != 0));
    assert!(changed, "the running engine must advance its framebuffer");
    Ok(())
}
