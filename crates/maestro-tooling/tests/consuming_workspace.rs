//! Preparation in a workspace that builds on Maestro rather than holding its sources.
//!
//! Such a workspace names its own component crate, ships its own extension folder, and holds
//! neither Maestro's guides nor its fixed fixture components. The development tool must render
//! into that workspace and leave the checkout it was installed from alone.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use maestro_test_directory::Scratch;

use tooling::workspace::{REGION_END, REGION_START};

use maestro_tooling as tooling;

/// Write a crate's manifest and the source file its package needs.
fn crate_at(root: &Path, relative: &str, manifest: &str) {
    let directory = root.join(relative);
    fs::create_dir_all(directory.join("src")).unwrap();
    fs::write(directory.join("Cargo.toml"), manifest).unwrap();
    fs::write(directory.join("src/lib.rs"), "").unwrap();
}

/// The manifest of the crate whose generated region the index owns.
fn component_manifest() -> String {
    format!(
        "[package]\nname = \"component-fixture\"\nversion = \"0.1.0\"\n{REGION_START}\nold\n{REGION_END}\n"
    )
}

/// A consuming workspace: its root manifest, one component crate with a local example, and a
/// shipped extension folder with its documented index.
fn consuming_workspace(root: &Path, shipped: bool) {
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
    crate_at(root, "crates/component-fixture", &component_manifest());
    fs::create_dir_all(root.join("crates/component-fixture/examples")).unwrap();
    fs::write(
        root.join("crates/component-fixture/examples/local.rs"),
        "//! Local component.\n// maestro-component: MAESTRO_LOCAL_COMPONENT\n",
    )
    .unwrap();
    if shipped {
        fs::create_dir_all(root.join("examples/extensions")).unwrap();
        fs::write(
            root.join("examples/extensions/README.md"),
            format!("Before\n{REGION_START}\nold\n{REGION_END}\nAfter\n")
                .replace(REGION_START, "<!-- example-index:start -->")
                .replace(REGION_END, "<!-- example-index:end -->"),
        )
        .unwrap();
        fs::write(
            root.join("examples/extensions/hello.rs"),
            "//! Hello extension.\nfn factory() {}\n",
        )
        .unwrap();
    }
}

/// Run the development tool on `root` and report its exit code.
fn development(root: &Path, arguments: &[&str]) -> std::io::Result<ExitCode> {
    let arguments: Vec<_> = arguments.iter().map(std::ffi::OsString::from).collect();
    tooling::run(&arguments, Path::new(env!("CARGO")), root)
}

/// The generated region of the workspace's component crate.
fn region(root: &Path) -> String {
    fs::read_to_string(root.join("crates/component-fixture/Cargo.toml")).unwrap()
}

/// The index renders the generated region into the component crate of the workspace it runs in,
/// beside the shipped example, and leaves Maestro's own checkout alone.
#[test]
fn example_index_renders_the_workspace_component_crate() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    consuming_workspace(root, true);
    let own = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/maestro-extensions-wasm");
    let before = fs::read(own.join("Cargo.toml")).unwrap();

    assert_eq!(
        development(root, &["example-index"]).unwrap(),
        ExitCode::SUCCESS
    );

    let rendered = region(root);
    assert!(
        rendered.contains("name = \"hello\"\npath = \"../../examples/extensions/hello.rs\"\n"),
        "{rendered}"
    );
    assert!(
        rendered.contains("name = \"local\"\ncrate-type = [\"cdylib\"]\n"),
        "{rendered}"
    );
    assert!(!rendered.contains("old"), "{rendered}");
    assert_eq!(fs::read(own.join("Cargo.toml")).unwrap(), before);
    let index = fs::read_to_string(root.join("examples/extensions/README.md")).unwrap();
    assert!(
        index.contains("| `hello.rs` | Hello extension. |"),
        "{index}"
    );
}

/// Without a shipped folder the index renders the crate's own targets alone: it writes no index
/// page and no guide lock, which the workspace does not hold either.
#[test]
fn example_index_renders_local_targets_without_a_shipped_folder() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    consuming_workspace(root, false);

    assert_eq!(
        development(root, &["example-index"]).unwrap(),
        ExitCode::SUCCESS
    );

    let rendered = region(root);
    assert!(
        rendered.contains("name = \"local\"\ncrate-type = [\"cdylib\"]\n"),
        "{rendered}"
    );
    assert!(!rendered.contains("examples/extensions"), "{rendered}");
    assert!(!root.join("examples").exists());
    assert_eq!(
        development(root, &["example-index", "--check"]).unwrap(),
        ExitCode::SUCCESS
    );
}

/// An installed command started inside the workspace serves that workspace, not the tree it was
/// built from, so a development run never renders into Maestro's own checkout.
#[test]
fn installed_tool_serves_the_workspace_it_starts_in() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    consuming_workspace(root, false);
    let nested = root.join("crates/component-fixture/src");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_development"))
        .arg("example-index")
        .current_dir(&nested)
        .env_remove("CARGO_MANIFEST_DIR")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(region(root).contains("name = \"local\""));
}

/// The staged name of the command the executable setup exports.
const EXECUTABLE: &str = "maestro";
/// The digest of a file with no bytes, which the empty fixture files below have.
const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// A prepared bundle whose inputs are the empty file set and whose executable is empty.
fn prepared_bundle(root: &Path) -> PathBuf {
    let bundle = root.join("bundle");
    fs::create_dir_all(&bundle).unwrap();
    for name in [EXECUTABLE, "prepare_author_component"] {
        fs::write(bundle.join(name), "").unwrap();
    }
    let inputs = serde_json::json!({"paths": [], "locked": [], "digest": EMPTY});
    let manifest = serde_json::json!({
        "shared": inputs,
        "tool": inputs,
        "browser": inputs,
        "components": {},
        "example": EMPTY,
        "executable": EMPTY,
    });
    fs::write(
        bundle.join("components.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    bundle
}

/// Run the `executable` setup from `root` and return its output.
fn executable_setup(root: &Path, bundle: &Path) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_development"))
        .args(["nextest-setup", "executable"])
        .current_dir(root)
        .env("TEST_PREPARE_OUTPUT", bundle)
        .env("NEXTEST_ENV", root.join("nextest-env"))
        .env_remove("TEST_ARCHIVE")
        .output()
        .unwrap()
}

/// The setup exports the executable the prepared bundle carries.
#[test]
fn executable_setup_exports_the_prepared_bundle() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    consuming_workspace(root, false);
    let bundle = prepared_bundle(root);

    let output = executable_setup(root, &bundle);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let exported = fs::read_to_string(root.join("nextest-env")).unwrap();
    assert_eq!(
        exported,
        format!("MAESTRO_EXECUTABLE={}\n", bundle.join(EXECUTABLE).display())
    );
}

/// A workspace that builds the command itself uses Cargo's executable, so the setup refuses it
/// rather than exporting a second copy.
#[test]
fn executable_setup_refuses_a_workspace_that_builds_maestro() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    consuming_workspace(root, false);
    crate_at(root, "crates/maestro", "[package]\nname = \"maestro\"\n");
    let bundle = prepared_bundle(root);

    let output = executable_setup(root, &bundle);

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("builds maestro itself"), "{error}");
}
