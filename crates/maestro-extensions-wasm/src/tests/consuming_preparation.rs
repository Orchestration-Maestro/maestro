//! The setup example prepares a workspace that builds on Maestro from that workspace's own
//! component crate, with only the inputs the workspace holds.
use std::path::{Path, PathBuf};
use std::process::Command;

use maestro_test_directory::Scratch;

/// The package of the fixture's component crate, which differs from its directory name.
const PACKAGE: &str = "example-components";

/// The setup example's native file name.
fn example_name() -> String {
    format!("prepare_author_component{}", std::env::consts::EXE_SUFFIX)
}

/// Write a consuming workspace at `root`: a component crate `crates/component-fixture` without a
/// fixture folder of its own, one shipped example it builds, and a tool configuration that pins
/// neither search program.
fn consuming_workspace(root: &Path) {
    let component = root.join("crates/component-fixture");
    std::fs::create_dir_all(component.join("src")).unwrap();
    std::fs::create_dir_all(root.join("examples/extensions")).unwrap();
    for (path, text) in [
        (
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n".to_owned(),
        ),
        (
            root.join("Cargo.lock"),
            format!("version = 4\n\n[[package]]\nname = \"{PACKAGE}\"\nversion = \"0.1.0\"\n"),
        ),
        (
            root.join("rust-toolchain.toml"),
            include_str!("../../../../rust-toolchain.toml").to_owned(),
        ),
        (
            root.join("mise.toml"),
            "[tools]\njust = \"1.58.0\"\n".to_owned(),
        ),
        (
            root.join("examples/extensions/hello.rs"),
            "//! Hello extension.\n// maestro-component: MAESTRO_HELLO_COMPONENT\n".to_owned(),
        ),
        (component.join("src/lib.rs"), String::new()),
        (
            component.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{PACKAGE}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
                 # example-index:start\n[[example]]\nname = \"hello\"\n\
                 path = \"../../examples/extensions/hello.rs\"\ncrate-type = [\"cdylib\"]\n\
                 # example-index:end\n"
            ),
        ),
    ] {
        std::fs::write(path, text).unwrap();
    }
}

/// Copy the setup example this test run built into `root`'s own target directory, where the
/// example finds the target directory its components build in.
fn install_example(root: &Path) -> PathBuf {
    let built = std::env::current_exe()
        .unwrap()
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("examples")
        .join(example_name());
    let installed = root.join("target/debug/examples").join(example_name());
    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    std::fs::copy(&built, &installed).unwrap();
    installed
}

/// The setup example builds the shipped example from the component crate's own package and
/// exports it, without a fixture folder in that crate or search pins in the workspace.
#[test]
fn setup_example_prepares_a_consuming_component_crate() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    consuming_workspace(root);
    let example = install_example(root);
    let environment = root.join("nextest-env");
    let mut command = Command::new(example);
    command
        .current_dir(root)
        .arg(root)
        .arg(root.join("crates/component-fixture"))
        .args([PACKAGE, ""])
        .env("CARGO", env!("CARGO"))
        .env("NEXTEST_ENV", &environment)
        .env("MAESTRO_CODING_AGENT_DIR", root.join("agent"));
    // The run's own prepared components would stand in for the fixture's.
    for (name, _) in std::env::vars_os() {
        if name
            .to_str()
            .is_some_and(|name| name.starts_with("MAESTRO_") && name.ends_with("_COMPONENT"))
        {
            command.env_remove(name);
        }
    }

    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let exported = std::fs::read_to_string(environment).unwrap();
    let component = exported
        .strip_prefix("MAESTRO_HELLO_COMPONENT=")
        .and_then(|rest| rest.strip_suffix('\n'))
        .unwrap_or_else(|| panic!("{exported}"));
    assert!(
        Path::new(component).starts_with(root.join("target")),
        "{component}"
    );
    assert!(Path::new(component).is_file(), "{component}");
}
