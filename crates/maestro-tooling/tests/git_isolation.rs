//! Native fixture setup isolates Git launched by production code during tests.

use maestro_test_directory::Scratch;
use maestro_test_process::git_command;
use std::{fs, process::Command};

/// The real setup exports a policy that excludes inherited Git configuration.
#[test]
fn native_setup_isolates_inherited_git_configuration() {
    let root = Scratch::new().unwrap();
    let inherited = root.path().join("inherited-gitconfig");
    fs::write(
        &inherited,
        "[fixture]\n inherited = true\n[tag]\n gpgsign = true\n",
    )
    .unwrap();
    let exports = root.path().join("environment");
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new(env!("CARGO_BIN_EXE_development"));
    command
        .current_dir(checkout)
        .args(["nextest-setup", "native-fixture"])
        .env("NEXTEST_ENV", &exports)
        .env("GIT_CONFIG_GLOBAL", &inherited);
    maestro_test_process::without_prepared_bundles(&mut command);
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let exports = fs::read_to_string(exports).unwrap();
    let environment: Vec<_> = exports
        .lines()
        .map(|line| line.split_once('=').unwrap())
        .collect();
    let probe = |args: &[&str]| {
        git_command::command()
            .current_dir(root.path())
            .env("GIT_CONFIG_GLOBAL", &inherited)
            .env("GIT_CONFIG_COUNT", "0")
            .env("GIT_CONFIG_PARAMETERS", "'tag.gpgsign=true'")
            .envs(environment.iter().copied())
            .args(args)
            .output()
            .unwrap()
    };

    let inherited = probe(&["config", "--get", "fixture.inherited"]);
    assert_eq!(inherited.status.code(), Some(1), "{inherited:?}");
    let signing = probe(&["config", "--get", "tag.gpgsign"]);
    assert!(signing.status.success(), "{signing:?}");
    assert_eq!(signing.stdout, b"false\n");
}
