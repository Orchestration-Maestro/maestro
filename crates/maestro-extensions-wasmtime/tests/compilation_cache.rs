//! Compiled bytes persist across loads without making the user cache mandatory.
#![cfg(test)]
#![cfg(target_os = "linux")]
use super::components::{TracedBus, loader, local};
use maestro_extensions::load_extensions;
use maestro_test_directory::Scratch;
use maestro_test_process::{TestProcess, selection};
use std::{cell::RefCell, path::Path};
use wasmtime::{Cache, Config, Engine, component::Component};

/// Runs this test in a separate process with an isolated user cache and configuration.
fn isolated(test: &str) -> bool {
    if selection().is_child() {
        return false;
    }
    let scratch = Scratch::new().unwrap();
    let output = TestProcess::current(test)
        .unwrap()
        .case("isolated")
        .env("XDG_CACHE_HOME", scratch.path().join("cache"))
        .env("XDG_CONFIG_HOME", scratch.path().join("config"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

/// Every file below `directory`, empty when it does not exist yet.
fn files_below(directory: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files_below(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}

/// Removes a file the compile wrote. The cache worker renames or deletes its own statistics
/// files in the background, so a file already gone counts as removed.
fn remove_written(path: &Path) {
    if let Err(error) = std::fs::remove_file(path) {
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "{error}");
    }
}

/// Compiles the supplied bytes through a second engine, observing the persistent cache. A
/// compile that misses removes every file that appeared in the cache during the compile, so only
/// an entry written earlier counts as a hit on any later run.
fn cached_compile(path: &Path) -> usize {
    let cache = Cache::from_file(None).unwrap();
    let before = files_below(cache.directory());
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    config.cache(Some(cache.clone()));
    let engine = Engine::new(&config).unwrap();
    Component::from_file(&engine, path).unwrap();
    if cache.cache_misses() > 0 {
        for written in files_below(cache.directory()) {
            if !before.contains(&written) {
                remove_written(&written);
            }
        }
    }
    cache.cache_hits()
}

/// Creates two independently linked executable timestamps around an older setup example. An
/// archived run instead builds the example last, after a library output newer than both
/// executables, as the setup build leaves it.
fn cache_consumers(root: &Path, archived: bool) -> [std::path::PathBuf; 2] {
    let profile = root.join("target/debug");
    std::fs::create_dir_all(profile.join("deps")).unwrap();
    std::fs::create_dir_all(profile.join("examples")).unwrap();
    if archived {
        let library = profile.join("deps/libworkspace-1.rlib");
        std::fs::write(&library, b"").unwrap();
        std::fs::File::open(library)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(3))
            .unwrap();
    }
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
        "crates/maestro-extensions-wasm/src/tests/build/cargo.rs",
        "crates/maestro-tooling/src/example_files.rs",
        "crates/maestro-tooling/src/example_files/components.rs",
        "crates/maestro-tooling/src/example_files/preparation.rs",
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
    let built = if archived { 5 } else { 1 };
    std::fs::File::open(example)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(built))
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

/// Runs setup in one executable and a second executable after it, both under one scratch user
/// cache, with the archive signal shared CI gives nextest when `archived` is set.
fn share_compiled_bytes(test: &str, archived: bool) {
    let scratch = Scratch::new().unwrap();
    let binaries = cache_consumers(scratch.path(), archived);
    let component = scratch.path().join("empty.wasm");
    std::fs::write(&component, b"\0asm\x0d\0\x01\0").unwrap();
    let setup =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.config/prepare-author-component.sh");
    for (binary, case) in binaries.iter().zip(["prime", "consume"]) {
        let process = TestProcess::current(test)
            .unwrap()
            .env_remove("TEST_ARCHIVE");
        let process = if archived {
            process.env("TEST_ARCHIVE", "1")
        } else {
            process
        };
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
fn component_setup_shares_compiled_bytes_across_executables() {
    if selection().is_child() {
        observe_compilation();
        return;
    }
    share_compiled_bytes(
        "component_setup_shares_compiled_bytes_across_executables",
        false,
    );
}

#[test]
fn archived_setup_shares_compiled_bytes_with_outdated_executables() {
    if selection().is_child() {
        observe_compilation();
        return;
    }
    share_compiled_bytes(
        "archived_setup_shares_compiled_bytes_with_outdated_executables",
        true,
    );
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
fn missed_compile_leaves_no_entry_for_a_later_run() {
    if isolated("missed_compile_leaves_no_entry_for_a_later_run") {
        return;
    }
    let scratch = Scratch::new().unwrap();
    let path = scratch.path().join("empty.wasm");
    std::fs::write(&path, b"\0asm\x0d\0\x01\0").unwrap();
    assert_eq!(cached_compile(&path), 0, "nothing warmed this component");
    assert_eq!(
        cached_compile(&path),
        0,
        "the first compile left an entry that a rerun mistakes for a warmed one"
    );
}

#[test]
fn removing_a_file_the_cache_worker_already_moved_is_not_a_failure() {
    let scratch = Scratch::new().unwrap();
    remove_written(&scratch.path().join("moved.wip-atomic-write-stats"));
}

/// Loads a valid empty component through the extension loader, which compiles it and then
/// fails to start it as an extension. Returns its path and the failures reported.
fn load_empty_component(scratch: &Scratch) -> (std::path::PathBuf, Vec<String>) {
    let path = scratch.path().join("empty.wasm");
    std::fs::write(&path, b"\0asm\x0d\0\x01\0").unwrap();
    let supplied = path.to_str().unwrap().to_owned();
    let errors = RefCell::new(Vec::new());
    local(async {
        let bus = TracedBus::new();
        let loaded = load_extensions(vec![supplied], "/unused", &bus.controller, &loader())
            .await
            .unwrap();
        errors.replace(loaded.errors.into_iter().map(|error| error.error).collect());
    });
    (path, errors.into_inner())
}

#[test]
fn component_load_populates_the_user_compilation_cache() {
    if isolated("component_load_populates_the_user_compilation_cache") {
        return;
    }
    let scratch = Scratch::new().unwrap();
    let (path, _) = load_empty_component(&scratch);
    assert!(
        cached_compile(&path) > 0,
        "load did not cache compiled bytes"
    );
}

#[test]
fn component_load_survives_an_unavailable_user_cache() {
    if isolated("component_load_survives_an_unavailable_user_cache") {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("XDG_CACHE_HOME").unwrap());
    std::fs::create_dir_all(root.parent().unwrap()).unwrap();
    std::fs::write(root, "not a directory").unwrap();
    assert!(Cache::from_file(None).is_err());
    let (_, errors) = load_empty_component(&Scratch::new().unwrap());
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].starts_with("Failed to load extension: component fault "),
        "{errors:?}"
    );
    // Every load failure carries that prefix; only this one is reached after the engine is
    // configured, the component compiled and instantiated without the cache.
    assert!(
        errors[0].ends_with(": missing extension guest interface"),
        "{errors:?}"
    );
}
