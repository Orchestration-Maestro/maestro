//! A standalone author project built from the entry guide and loaded by the native host.
#![cfg(test)]
use maestro_extensions::{ExtensionRunner, Local, discover_and_load_extensions};
use maestro_test_directory::Scratch;
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::RwLock,
    time::Instant,
};

#[path = "../../maestro-extensions-wasm/src/tests/build/cargo.rs"]
pub(super) mod cargo;

/// Creates the independent project using the guide's manifest and the shipped source.
fn project() -> (Scratch, PathBuf) {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    let guide = std::fs::read_to_string(checkout.join("docs/extensions.md")).unwrap();
    let manifest = include_str!("fixtures/hello-extension/Cargo.toml.in");
    assert_eq!(
        guide
            .split("```toml\n")
            .nth(1)
            .unwrap()
            .split("```")
            .next()
            .unwrap(),
        manifest
    );
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    std::fs::create_dir(root.join("src")).unwrap();
    let manifest = manifest.replace(
        "\"../maestro/crates/maestro-extensions-wasm\"",
        &serde_json::to_string(&checkout.join("crates/maestro-extensions-wasm")).unwrap(),
    );
    std::fs::write(root.join("Cargo.toml"), manifest).unwrap();
    std::fs::write(
        root.join("Cargo.lock"),
        include_str!("fixtures/hello-extension/Cargo.lock"),
    )
    .unwrap();
    std::fs::copy(
        checkout.join("examples/extensions/hello.rs"),
        root.join("src/lib.rs"),
    )
    .unwrap();
    std::fs::copy(
        checkout.join("rust-toolchain.toml"),
        root.join("rust-toolchain.toml"),
    )
    .unwrap();
    let component = cargo::component(root, &["--lib"], "hello_extension").unwrap();
    (scratch, component)
}

/// The guide's project builds outside the workspace and registers a live greeting tool.
#[test]
fn extension_guide_builds_and_loads_from_a_clean_folder() {
    let started = Instant::now();
    let (scratch, component) = project();
    let root = scratch.path();
    let extension = root.join(".maestro/extensions/hello");
    std::fs::create_dir_all(&extension).unwrap();
    std::fs::copy(component, extension.join("index.wasm")).unwrap();
    super::components::local(async {
        let bus = super::components::TracedBus::new();
        let loaded = discover_and_load_extensions(
            &[],
            root.to_str().unwrap(),
            root.join("agent").to_str().unwrap(),
            &bus.controller,
            &super::components::loader(),
        )
        .await
        .unwrap();
        assert!(loaded.errors.is_empty(), "{:?}", loaded.errors);
        assert_eq!(loaded.extensions.len(), 1);
        let runner = Rc::new(ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(loaded.extensions)),
            loaded.runtime,
            super::session::runner_session(root.to_str().unwrap()).0,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        ));
        let tools = runner.get_all_registered_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].definition.metadata().definition.name, "hello");
        let tool = maestro_extensions::wrap_registered_tool(&tools[0], runner);
        let result = (tool.execute)("greeting".into(), json!({"name":"Ada"}), None, None)
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(result.content).unwrap(),
            json!([{"type":"text","text":"Hello, Ada!"}])
        );
        assert_eq!(result.details, Some(json!({"greeted":"Ada"})));
    });
    eprintln!("standalone build and native load: {:?}", started.elapsed());
}
