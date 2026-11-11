use std::cell::Cell;
use std::path::Path;

use cargo_metadata::Metadata;
use maestro_test_directory::Scratch;

use super::read;

/// A workspace with one package.
fn workspace() -> Scratch {
    let root = Scratch::new().unwrap();
    std::fs::create_dir_all(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/lib.rs"), "").unwrap();
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"one\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n",
    )
    .unwrap();
    root
}

/// A document of an empty workspace, which is all the cache needs to hold.
fn document() -> Metadata {
    serde_json::from_str(
        r#"{"packages":[],"workspace_members":[],"workspace_default_members":[],"resolve":null,
        "target_directory":"/target","version":1,"workspace_root":"/","metadata":null}"#,
    )
    .unwrap()
}

/// Read the workspace once more and say whether the document had to be produced again.
fn produced(root: &Path, host: Option<&str>) -> bool {
    let produced = Cell::new(false);
    read(root, host, || {
        produced.set(true);
        Ok(document())
    })
    .unwrap();
    produced.get()
}

#[test]
fn an_unchanged_workspace_is_read_once() {
    let root = workspace();

    assert!(produced(root.path(), None));
    std::fs::write(root.path().join("src/lib.rs"), "pub fn changed() {}").unwrap();
    std::fs::create_dir(root.path().join("target")).unwrap();
    std::fs::write(root.path().join("target/output"), "build output").unwrap();

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
    .unwrap();

    assert!(produced(root.path(), None));
}

#[test]
fn an_added_source_file_is_read_again_because_it_can_add_a_target() {
    let root = workspace();
    assert!(produced(root.path(), None));

    std::fs::create_dir(root.path().join("examples")).unwrap();
    std::fs::write(root.path().join("examples/demo.rs"), "fn main() {}").unwrap();

    assert!(produced(root.path(), None));
}

#[test]
fn each_host_filter_is_read_separately() {
    let root = workspace();
    assert!(produced(root.path(), None));

    assert!(produced(root.path(), Some("x86_64-unknown-linux-gnu")));
    assert!(!produced(root.path(), Some("x86_64-unknown-linux-gnu")));
}
