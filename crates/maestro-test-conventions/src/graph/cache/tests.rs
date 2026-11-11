use std::cell::Cell;
use std::path::Path;

use cargo_metadata::Metadata;
use maestro_test_directory::Scratch;

use super::read;

/// A workspace with one package.
fn workspace() -> Scratch {
    let root = Scratch::new().expect("the scratch directory must be created");
    std::fs::create_dir_all(root.path().join("src")).expect("the source directory must be created");
    std::fs::write(root.path().join("src/lib.rs"), "").expect("the library source must be written");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"one\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n",
    )
    .expect("the manifest must be written");
    root
}

/// A document of an empty workspace, which is all the cache needs to hold.
fn document() -> Metadata {
    serde_json::from_str(
        r#"{"packages":[],"workspace_members":[],"workspace_default_members":[],"resolve":null,
        "target_directory":"/target","version":1,"workspace_root":"/","metadata":null}"#,
    )
    .expect("the empty document must parse")
}

/// Read the workspace once more and say whether the document had to be produced again.
fn produced(root: &Path, host: Option<&str>) -> bool {
    let produced = Cell::new(false);
    read(root, host, || {
        produced.set(true);
        Ok(document())
    })
    .expect("the read must succeed");
    produced.get()
}

#[test]
fn an_unchanged_workspace_is_read_once() {
    let root = workspace();

    assert!(produced(root.path(), None));
    std::fs::write(root.path().join("src/lib.rs"), "pub fn changed() {}")
        .expect("the source file must be rewritten");
    std::fs::create_dir(root.path().join("target")).expect("the target directory must be created");
    std::fs::write(root.path().join("target/output"), "build output")
        .expect("the build output must be written");

    assert!(!produced(root.path(), None));
}

#[test]
fn a_changed_manifest_is_read_again() {
    let root = workspace();
    assert!(produced(root.path(), None));

    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"two\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n",
    )
    .expect("the manifest must be rewritten");

    assert!(produced(root.path(), None));
}

#[test]
fn an_added_source_file_is_read_again_because_it_can_add_a_target() {
    let root = workspace();
    assert!(produced(root.path(), None));

    std::fs::create_dir(root.path().join("examples"))
        .expect("the examples directory must be created");
    std::fs::write(root.path().join("examples/demo.rs"), "fn main() {}")
        .expect("the example must be written");

    assert!(produced(root.path(), None));
}

#[test]
fn each_host_filter_is_read_separately() {
    let root = workspace();
    assert!(produced(root.path(), None));

    assert!(produced(root.path(), Some("x86_64-unknown-linux-gnu")));
    assert!(!produced(root.path(), Some("x86_64-unknown-linux-gnu")));
}

#[test]
fn a_changed_lock_file_is_read_again() {
    let root = workspace();
    std::fs::write(root.path().join("Cargo.lock"), "version = 4\n")
        .expect("the lock file must be written");
    assert!(produced(root.path(), None));

    std::fs::write(root.path().join("Cargo.lock"), "version = 3\n")
        .expect("the lock file must be rewritten");

    assert!(produced(root.path(), None));
}

#[test]
fn a_changed_cargo_configuration_without_an_extension_is_read_again() {
    let root = workspace();
    std::fs::create_dir(root.path().join(".cargo")).expect("the .cargo directory must be created");
    std::fs::write(
        root.path().join(".cargo/config"),
        "[term]\nverbose = false\n",
    )
    .expect("the configuration must be written");
    assert!(produced(root.path(), None));

    std::fs::write(
        root.path().join(".cargo/config"),
        "[term]\nverbose = true\n",
    )
    .expect("the configuration must be rewritten");

    assert!(produced(root.path(), None));
}
