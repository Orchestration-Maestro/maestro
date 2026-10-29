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
    Component::from_file(&engine, path).unwrap();
    cache.cache_hits()
}

/// Creates two independently linked executable timestamps around an older setup example.
fn cache_consumers(root: &Path) -> [std::path::PathBuf; 2] {
    let profile = root.join("target/debug");
    std::fs::create_dir_all(profile.join("deps")).unwrap();
    std::fs::create_dir_all(profile.join("examples")).unwrap();
    let binaries = [profile.join("deps/prime"), profile.join("deps/consume")];
    for (binary, seconds) in binaries.iter().zip([1, 2]) {
        std::fs::copy(std::env::current_exe().unwrap(), binary).unwrap();
        std::fs::File::open(binary)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
            .unwrap();
    }
    for source in [
        "crates/maestro-extensions-wasm/examples/prepare_author_component.rs",
        "crates/maestro-extensions-wasm/src/tests/build.rs",
        "crates/maestro-tooling/src/example_files.rs",
        "crates/maestro-extensions-wasm/Cargo.toml",
        "Cargo.lock",
    ] {
        let path = root.join(source);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::File::create(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH)
            .unwrap();
    }
    let example = profile.join("examples/prepare_author_component");
    std::fs::write(
        &example,
        "#!/bin/sh\nexec \"$COPY\" \"$TEST_NAME\" --exact --nocapture\n",
    )
    .unwrap();
    std::fs::set_permissions(
        &example,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    std::fs::File::open(example)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1))
        .unwrap();
    binaries
}

/// Observes the real persistent cache without a process-local compiled-component map.
fn observe_compilation() {
    let cache = Cache::from_file(None).unwrap();
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.cache(Some(cache.clone()));
    let engine = Engine::new(&config).unwrap();
    let start = std::time::Instant::now();
    Component::from_file(
        &engine,
        std::env::var_os("MAESTRO_CACHE_COMPONENT").unwrap(),
    )
    .unwrap();
    println!(
        "load={:?} hits={} misses={}",
        start.elapsed(),
        cache.cache_hits(),
        cache.cache_misses()
    );
    if selection().runs_case("consume") {
        assert_eq!(
            cache.cache_misses(),
            0,
            "second executable compiled unchanged bytes"
        );
        assert_eq!(cache.cache_hits(), 1);
    } else {
        assert_eq!(cache.cache_misses(), 1);
    }
}

#[test]
fn component_setup_shares_compiled_bytes_across_executables() {
    if selection().is_child() {
        observe_compilation();
        return;
    }
    let scratch = Scratch::new().unwrap();
    let binaries = cache_consumers(scratch.path());
    let component = scratch.path().join("empty.wasm");
    std::fs::write(&component, b"\0asm\x0d\0\x01\0").unwrap();
    let setup =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.config/prepare-author-component.sh");
    for (binary, case) in binaries.iter().zip(["prime", "consume"]) {
        let process =
            TestProcess::current("component_setup_shares_compiled_bytes_across_executables")
                .unwrap();
        let name = process.name().to_owned();
        let shell = if case == "prime" {
            "shift; exec sh \"$SETUP\""
        } else {
            "shift; exec \"$COPY\" \"$@\""
        };
        let output = process
            .case(case)
            .nocapture()
            .current_dir(scratch.path())
            .env("COPY", binary)
            .env("TEST_NAME", name)
            .env("SETUP", &setup)
            .env("MAESTRO_CACHE_COMPONENT", &component)
            .env(
                "NEXTEST_LD_LIBRARY_PATH",
                scratch.path().join("target/debug"),
            )
            .env("XDG_CACHE_HOME", scratch.path().join("cache"))
            .env("XDG_CONFIG_HOME", scratch.path().join("config"))
            .shell_setup("/bin/sh", shell)
            .output()
            .unwrap();
        println!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn component_setup_warms_exported_examples() {
    let path = std::env::var("MAESTRO_LOCATION_COMPONENT").unwrap();
    assert!(
        cached_compile(Path::new(&path)) > 0,
        "setup did not warm the exported example for the consuming executable"
    );
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
