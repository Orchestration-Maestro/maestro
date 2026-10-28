//! Compiled bytes persist across loads without making the user cache mandatory.
#![cfg(test)]
#![cfg(target_os = "linux")]
use super::components::{TracedBus, loader, local};
use maestro_extensions::load_extensions;
use maestro_test_directory::Scratch;
use maestro_test_process::{TestProcess, selection};
use std::path::Path;
use wasmtime::{Cache, Config, Engine, component::Component};

/// Runs this test in a separate process with an isolated user cache and configuration.
fn isolated(test: &str) -> bool {
    if selection().is_child() {
        return false;
    }
    let scratch = Scratch::new().unwrap();
    for mode in ["available", "unavailable"] {
        let output = TestProcess::current(test)
            .unwrap()
            .case(mode)
            .env("XDG_CACHE_HOME", scratch.path().join(mode).join("cache"))
            .env("XDG_CONFIG_HOME", scratch.path().join(mode).join("config"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    true
}

/// Compiles the supplied bytes through a second engine, observing the persistent cache.
fn cached_compile(path: &Path) -> usize {
    let cache = Cache::from_file(None).unwrap();
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.cache(Some(cache.clone()));
    let engine = Engine::new(&config).unwrap();
    let started = std::time::Instant::now();
    Component::from_file(&engine, path).unwrap();
    eprintln!(
        "component compile: {} {:?}, hits={}, misses={}",
        path.display(),
        started.elapsed(),
        cache.cache_hits(),
        cache.cache_misses()
    );
    cache.cache_hits()
}

#[test]
fn prepared_components_hit_compilation_cache() {
    for variable in [
        "MAESTRO_CONTEXT_COMPONENT",
        "MAESTRO_HOOK_COMPONENT",
        "MAESTRO_GIT_CHECKPOINT_COMPONENT",
    ] {
        let path = std::env::var_os(variable).unwrap();
        assert!(
            cached_compile(Path::new(&path)) > 0,
            "cold component: {variable}"
        );
    }
}

#[test]
fn component_load_populates_the_user_compilation_cache() {
    if isolated("component_load_populates_the_user_compilation_cache") {
        return;
    }
    let selected = selection();
    if selected.runs_case("unavailable") {
        let root = std::path::PathBuf::from(std::env::var_os("XDG_CACHE_HOME").unwrap());
        std::fs::create_dir_all(root.parent().unwrap()).unwrap();
        std::fs::write(root, "not a directory").unwrap();
        assert!(Cache::from_file(None).is_err());
    }
    local(async {
        let bus = TracedBus::new();
        let path = std::env::var("MAESTRO_LOCATION_COMPONENT").unwrap();
        let loaded = load_extensions(vec![path.clone()], "/unused", &bus.controller, &loader())
            .await
            .unwrap();
        assert!(loaded.errors.is_empty(), "{:?}", loaded.errors);
        assert_eq!(loaded.extensions.len(), 1);
        assert!(
            loaded.extensions[0]
                .read()
                .unwrap()
                .flags
                .contains_key("location")
        );
        if selected.runs_case("available") {
            assert!(
                cached_compile(Path::new(&path)) > 0,
                "load did not cache compiled bytes"
            );
        }
    });
}
