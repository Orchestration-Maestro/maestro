#![cfg(not(target_arch = "wasm32"))]

/// A prepared browser check needs neither Cargo nor a compiler in the test body.
#[test]
fn prepared_browser_check_runs_without_a_build_toolchain() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let missing = scratch.path().join("missing-toolchain");
    let output = maestro_test_process::TestProcess::named(
        "browser_build::maestro_browser_environment_has_no_key",
    )
    .unwrap()
    .env("CARGO", &missing)
    .env("RUSTC", &missing)
    .env("CARGO_TARGET_DIR", scratch.path())
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// The real cross-target build is prepared before the native test reads its browser witness.
#[test]
fn maestro_browser_environment_has_no_key() {
    let artifact = std::env::var_os("MAESTRO_BROWSER_IMPORT_CHECK")
        .expect("nextest cargo-checks setup supplies the built browser example");
    let bytes = std::fs::read(artifact).unwrap();
    assert!(bytes.starts_with(b"\0asm"));
}
