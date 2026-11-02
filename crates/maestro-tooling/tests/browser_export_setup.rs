//! Browser export preparation through the repository development command.

use std::{fs, path::Path, process::ExitCode};

use maestro_test_directory::Scratch;
use maestro_tooling as tooling;

/// A registry package whose identity must remain unchanged by scratch resolution.
const LOCK: &str = "version=4\n[[package]]\nname='wasm-bindgen'\nversion='0.2.129'\nsource='registry+https://example.invalid/index'\nchecksum='original'\n";

/// Write the actual preparation inputs inside a disposable checkout.
fn checkout(root: &Path) {
    fs::create_dir_all(root.join("crates/maestro-web/tests")).unwrap();
    fs::write(root.join("Cargo.lock"), LOCK).unwrap();
    fs::write(
        root.join("rust-toolchain.toml"),
        "[toolchain]\nchannel='1.98.0'\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/maestro-web/tests/integration.rs"),
        "pub fn witness() {}\n",
    )
    .unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace.dependencies]\nalias={package='actual',version='=1.2.3',features=['base','base'],default-features=false}\nlocal={path='crates/local'}\n[patch.crates-io]\npatched={path='patches/patched'}\n").unwrap();
    fs::write(root.join("crates/maestro-web/Cargo.toml"), "[dependencies]\nalias={workspace=true,features=['normal']}\nlocal.workspace=true\nplain='=2.0.0'\n[target.'cfg(target_arch = \"wasm32\")'.dependencies]\nalias={workspace=true,features=['wasm','base']}\n[target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]\nnative-only='=9.0.0'\n[dev-dependencies]\nalias={workspace=true,features=['dev','wasm']}\nmaestro-web={path='.',features=['test-support']}\n").unwrap();
}

/// Invoke preparation against the supplied checkout without invoking Cargo or a browser.
fn run(root: &Path, operation: &str, value: &str) -> std::io::Result<ExitCode> {
    tooling::run(
        &[
            "browser-export-setup".into(),
            operation.into(),
            value.into(),
        ],
        Path::new(env!("CARGO")),
        root,
    )
}

/// CLI binding versions must equal the one unambiguous locked package version.
#[test]
fn binding_version_matches_the_locked_package() {
    let scratch = Scratch::new().unwrap();
    checkout(scratch.path());
    assert_eq!(
        run(scratch.path(), "binding-version", "wasm-bindgen 0.2.129\n").unwrap(),
        ExitCode::SUCCESS
    );
    for version in ["wasm-bindgen 0.2.128", "other 0.2.129", ""] {
        let error = run(scratch.path(), "binding-version", version).unwrap_err();
        assert!(error.to_string().contains("wasm-bindgen"), "{error}");
    }
}

/// Missing, malformed, incomplete and ambiguous lock records fail before browser compilation.
#[test]
fn binding_version_rejects_invalid_lock_data() {
    let scratch = Scratch::new().unwrap();
    checkout(scratch.path());
    for lock in [
        "bad=[",
        "version=4\n",
        "[[package]]\nname='other'\nversion='1'\n",
        "[[package]]\nname='wasm-bindgen'\n",
        "[[package]]\nname='wasm-bindgen'\nversion=129\n",
        &format!("{LOCK}[[package]]\nname='wasm-bindgen'\nversion='0.2.128'\n"),
    ] {
        fs::write(scratch.path().join("Cargo.lock"), lock).unwrap();
        let error = run(scratch.path(), "binding-version", "wasm-bindgen 0.2.129").unwrap_err();
        assert!(error.to_string().contains("Cargo.lock"), "{lock}: {error}");
    }
    fs::remove_file(scratch.path().join("Cargo.lock")).unwrap();
    assert!(run(scratch.path(), "binding-version", "wasm-bindgen 0.2.129").is_err());
}

/// Inherited aliases retain their package identity and union all browser test features.
#[test]
fn browser_manifest_resolves_inheritance_features_paths_and_patches() {
    let scratch = Scratch::new().unwrap();
    checkout(scratch.path());
    let output = scratch.path().join("prepared 雪");
    run(scratch.path(), "prepare", output.to_str().unwrap()).unwrap();
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(output.join("Cargo.toml")).unwrap()).unwrap();
    let canonical = scratch.path().canonicalize().unwrap();
    assert_dependencies(&manifest["dependencies"], &canonical);
    assert_library(&manifest, &canonical);
    for file in ["Cargo.lock", "rust-toolchain.toml"] {
        assert_eq!(
            fs::read(output.join(file)).unwrap(),
            fs::read(scratch.path().join(file)).unwrap()
        );
    }
    assert_eq!(
        fs::read(output.join("tests/integration.rs")).unwrap(),
        b"pub fn witness() {}\n"
    );
    assert!(
        fs::symlink_metadata(output.join("tests"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

/// Browser dependencies keep the alias, combined features and absolute local paths.
fn assert_dependencies(dependencies: &toml::Value, root: &Path) {
    assert_eq!(dependencies["alias"]["package"].as_str(), Some("actual"));
    assert_eq!(dependencies["alias"]["version"].as_str(), Some("=1.2.3"));
    assert_eq!(
        dependencies["alias"]["default-features"].as_bool(),
        Some(false)
    );
    assert_eq!(
        dependencies["alias"]["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["base", "normal", "wasm", "dev"]
    );
    assert_eq!(dependencies["plain"]["version"].as_str(), Some("=2.0.0"));
    assert_eq!(
        Path::new(dependencies["local"]["path"].as_str().unwrap()),
        root.join("crates/local")
    );
    assert_eq!(
        Path::new(dependencies["maestro-web"]["path"].as_str().unwrap()),
        root.join("crates/maestro-web")
    );
    assert_eq!(
        dependencies["maestro-web"]["features"][0].as_str(),
        Some("test-support")
    );
    assert!(dependencies.get("native-only").is_none());
}

/// The library source, patches and compiler profile retain their authored meaning.
fn assert_library(manifest: &toml::Value, root: &Path) {
    assert_eq!(
        Path::new(
            manifest["patch"]["crates-io"]["patched"]["path"]
                .as_str()
                .unwrap()
        ),
        root.join("patches/patched")
    );
    assert_eq!(
        Path::new(manifest["lib"]["path"].as_str().unwrap()),
        root.join("crates/maestro-web/tests/integration.rs")
    );
    assert_eq!(manifest["lib"]["crate-type"][0].as_str(), Some("cdylib"));
    assert_eq!(
        manifest["profile"]["dev"]["debug"].as_str(),
        Some("line-tables-only")
    );
    assert_eq!(
        manifest["profile"]["dev"]["package"]["*"]["debug"].as_bool(),
        Some(false)
    );
}

/// Bad manifest values fail instead of silently omitting dependencies or their features.
#[test]
fn browser_manifest_rejects_malformed_dependencies() {
    let scratch = Scratch::new().unwrap();
    checkout(scratch.path());
    for (member, reason) in [
        ("bad=[", "unclosed array"),
        ("dependencies=[]", "expected a dependency table"),
        ("target=[]", "expected a dependency table"),
        (
            "[target]\n'cfg(target_arch = \"wasm32\")'=[]",
            "expected a dependency table",
        ),
        ("[dependencies]\nbad=3", "expected a dependency table"),
        (
            "[dependencies]\nmissing.workspace=true",
            "missing workspace dependency missing",
        ),
        (
            "[dependencies]\nalias={workspace=true,features=3}",
            "dependency features must be an array",
        ),
        (
            "[dependencies]\nlocal={path=3}",
            "dependency path must be a string",
        ),
        (
            "[dependencies]\nalias={workspace='true'}",
            "dependency workspace must be a boolean",
        ),
    ] {
        fs::write(scratch.path().join("crates/maestro-web/Cargo.toml"), member).unwrap();
        let error = run(
            scratch.path(),
            "prepare",
            scratch.path().join("output").to_str().unwrap(),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains(reason),
            "{member}: expected {reason}: {error}"
        );
        assert!(
            !scratch.path().join("output/Cargo.toml").exists(),
            "{member}: {error}"
        );
    }
}

/// Scratch resolution may remove packages or add local ones, never change an external identity.
#[test]
fn browser_lock_refuses_every_new_external_identity() {
    let scratch = Scratch::new().unwrap();
    checkout(scratch.path());
    let output = scratch.path().join("output");
    fs::create_dir(&output).unwrap();
    let check = || run(scratch.path(), "check-lock", output.to_str().unwrap());
    for lock in [
        LOCK.to_owned(),
        format!("{LOCK}[[package]]\nname='local'\nversion='1.0.0'\n"),
        "version=4\n[[package]]\nname='local'\nversion='1'\n".into(),
    ] {
        fs::write(output.join("Cargo.lock"), lock).unwrap();
        assert_eq!(check().unwrap(), ExitCode::SUCCESS);
    }
    for lock in [
        LOCK.replace("wasm-bindgen", "added"),
        LOCK.replace("0.2.129", "0.2.130"),
        LOCK.replace("example.invalid", "other.invalid"),
        LOCK.replace("original", "changed"),
        LOCK.replace("checksum='original'\n", ""),
        format!(
            "{LOCK}[[package]]\nname='added'\nversion='1'\nsource='git+https://example.invalid/repo'\n"
        ),
    ] {
        fs::write(output.join("Cargo.lock"), lock).unwrap();
        let error = check().unwrap_err().to_string();
        assert!(error.contains("external"), "{error}");
    }
}

/// An unreadable or malformed scratch lock never counts as an empty successful comparison.
#[test]
fn browser_lock_rejects_missing_or_malformed_packages() {
    let scratch = Scratch::new().unwrap();
    checkout(scratch.path());
    let output = scratch.path().join("output");
    fs::create_dir(&output).unwrap();
    for lock in [
        "bad=[",
        "version=4",
        "package={}",
        "[[package]]\nname='external'\nsource='registry+example'\n",
        "[[package]]\nname='external'\nversion='1'\nsource=3\n",
    ] {
        fs::write(output.join("Cargo.lock"), lock).unwrap();
        let error = run(scratch.path(), "check-lock", output.to_str().unwrap()).unwrap_err();
        assert!(error.to_string().contains("Cargo.lock"), "{lock}: {error}");
    }
    fs::remove_file(output.join("Cargo.lock")).unwrap();
    assert!(run(scratch.path(), "check-lock", output.to_str().unwrap()).is_err());
}
