//! Compiled bundles follow the engine's reported cache directory on each host.
use super::*;
use maestro_test_directory::Scratch;
use std::time::Duration;

#[test]
fn cache_directory_reply_requires_one_absolute_path() {
    let scratch = Scratch::new().unwrap();
    let path = scratch.path().join("Local/BytecodeAlliance/wasmtime/cache");
    assert_eq!(
        reported_directory(&serde_json::to_vec(&path).unwrap()).unwrap(),
        path
    );
    for reply in [
        b"".as_slice(),
        br#""""#,
        br#""relative/cache""#,
        br#""/first" "/second""#,
        b"null",
        b"[]",
    ] {
        let error = reported_directory(reply).unwrap_err().to_string();
        assert!(
            error.contains("cache directory"),
            "reply: {reply:?}: {error}"
        );
    }
}

#[test]
fn reported_cache_directory_is_collected_and_installed() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    let cache = root.join("preparing/Local/BytecodeAlliance/wasmtime/cache");
    let namespace = cache.join("modules/engine-11000");
    std::fs::create_dir_all(&namespace).unwrap();
    std::fs::write(namespace.join("compiled-component"), b"compiled bytes").unwrap();
    let example = root.join("prepare_author_component.exe");
    std::fs::write(&example, b"prepared executable").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&example)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(11))
        .unwrap();
    let staged = root.join("bundle/cache");

    let code = CompiledCode::collect(&cache, &example, &staged, None)
        .unwrap()
        .expect("the engine's reported cache must be collected");
    code.verify(&staged).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&example)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(22))
        .unwrap();
    let consuming = root.join("consuming/Local/BytecodeAlliance/wasmtime/cache");
    code.install(&staged, &consuming, &example).unwrap();

    assert_eq!(
        std::fs::read(consuming.join("modules/engine-22000/compiled-component")).unwrap(),
        b"compiled bytes"
    );
}
