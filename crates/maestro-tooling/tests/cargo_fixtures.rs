//! Cargo fixture cache isolation through the shared command builder.

use crate::recipe_support::{self, Workspace};
use crate::toolchain_support;
use maestro_test_directory::Scratch;
use std::fs;
use std::path::Path;

/// Dependency-free fixture resolution creates only its private cache lock.
#[test]
fn fixture_cargo_resolution_uses_private_home() {
    let root = Scratch::new().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "").unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = 'cache-fixture'\nversion = '0.0.0'\nedition = '2024'\n[workspace]\n",
    )
    .unwrap();
    let output = toolchain_support::fixture_command(env!("CARGO"), root.path(), None)
        .unwrap()
        .args(["metadata", "--offline", "--format-version", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        root.path().join("cargo-home/.package-cache").is_file(),
        "fixture resolution must create its private package-cache lock"
    );
}

/// Registry fixtures resolve supplied vendored sources without an ambient cache.
#[test]
fn fixture_cargo_resolution_uses_supplied_sources() {
    let root = Scratch::new().unwrap();
    let dependency = root.path().join("vendor/cache-dependency");
    fs::create_dir_all(dependency.join("src")).unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "").unwrap();
    fs::write(dependency.join("src/lib.rs"), "").unwrap();
    fs::write(
        dependency.join("Cargo.toml"),
        "[package]\nname = 'cache-dependency'\nversion = '0.0.0'\nedition = '2024'\n",
    )
    .unwrap();
    fs::write(
        dependency.join(".cargo-checksum.json"),
        r#"{"files":{},"package":null}"#,
    )
    .unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = 'cache-fixture'\nversion = '0.0.0'\nedition = '2024'\n\
         [workspace]\n[dependencies]\ncache-dependency = '=0.0.0'\n",
    )
    .unwrap();
    let sources = root.path().join("sources.toml");
    fs::write(
        &sources,
        format!(
            "[source.crates-io]\nreplace-with = 'fixtures'\n[source.fixtures]\n\
             directory = {}\n[net]\noffline = true\n",
            serde_json::to_string(&root.path().join("vendor")).unwrap()
        ),
    )
    .unwrap();
    let output = toolchain_support::fixture_command(env!("CARGO"), root.path(), Some(&sources))
        .unwrap()
        .args(["metadata", "--offline", "--format-version", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(metadata["packages"].as_array().unwrap().len(), 2);
    let expected = serde_json::to_value(dependency.join("Cargo.toml")).unwrap();
    assert!(
        metadata["packages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|package| package["manifest_path"] == expected)
    );
}

/// The recipe tool builds under a private Cargo home, not the shared package cache.
#[test]
fn recipe_tool_build_uses_a_private_cargo_home() {
    let root = Scratch::new().unwrap();
    assert!(recipe_support::build_tool(root.path()).is_file());
    assert!(
        root.path().join("cargo-home/.package-cache").is_file(),
        "the tool build must lock its private package cache"
    );
}

/// The compiler wrapper behind a fixture command never sees the private Cargo home, so the
/// compiler cache keys of the fixture's dependencies do not depend on the fixture that builds them.
#[cfg(unix)]
#[test]
fn fixture_command_hides_cargo_home_from_the_compiler_wrapper() {
    use maestro_test_process::TestProcess;
    use std::os::unix::fs::PermissionsExt;

    const ROOT: &str = "MAESTRO_FIXTURE_WRAPPER_ROOT";
    if let Some(root) = std::env::var_os(ROOT) {
        let root = Path::new(&root);
        let output = toolchain_support::fixture_command(env!("CARGO"), root, None)
            .unwrap()
            .args(["build", "--offline", "--target-dir"])
            .arg(root.join("target"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let root = Scratch::new().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "").unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = 'wrapper-fixture'\nversion = '0.0.0'\nedition = '2024'\n[workspace]\n",
    )
    .unwrap();
    let tools = Scratch::new().unwrap();
    let recorder = tools.path().join("recorder.sh");
    fs::write(
        &recorder,
        "#!/bin/sh\necho \"${CARGO_HOME-unset}\" >> \"$0.log\"\nexec \"$@\"\n",
    )
    .unwrap();
    fs::set_permissions(&recorder, fs::Permissions::from_mode(0o755)).unwrap();
    let output = TestProcess::current("fixture_command_hides_cargo_home_from_the_compiler_wrapper")
        .unwrap()
        .nocapture()
        .env(ROOT, root.path())
        .env("RUSTC_WRAPPER", &recorder)
        .env_remove("__CARGO_LLVM_COV_RUSTC_WRAPPER")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(tools.path().join("recorder.sh.log")).unwrap();
    assert!(!log.is_empty(), "the compiler wrapper must run");
    assert!(
        log.lines().all(|home| home == "unset"),
        "the wrapper saw a Cargo home: {log}"
    );
    assert!(
        root.path().join("cargo-home/.package-cache").is_file(),
        "Cargo keeps its private home"
    );
}

/// The wrapper is replaced only when the command ends up with a non-empty one: an empty
/// inherited wrapper and a wrapper the command removes stay unwrapped, and a wrapper the command
/// sets is the one the script runs.
#[cfg(unix)]
#[test]
fn fixture_command_wraps_only_a_wrapper_the_command_keeps() {
    use maestro_test_process::TestProcess;

    const REPORT: &str = "MAESTRO_FIXTURE_WRAPPER_REPORT";
    const COVERAGE: &str = "__CARGO_LLVM_COV_RUSTC_WRAPPER";
    const NAME: &str = "fixture_command_wraps_only_a_wrapper_the_command_keeps";
    if std::env::var_os(REPORT).is_some() {
        let root = Scratch::new().unwrap();
        let command = toolchain_support::fixture_command("true", root.path(), None).unwrap();
        let env = |name: &str| {
            command
                .get_envs()
                .find(|(key, _)| *key == name)
                .and_then(|(_, value)| value)
                .map(|value| value.to_string_lossy().into_owned())
        };
        let wrapped =
            env("RUSTC_WRAPPER").is_some_and(|wrapper| wrapper.ends_with("rustc-wrapper.sh"));
        println!("{wrapped} {:?}", env("MAESTRO_FIXTURE_RUSTC_WRAPPER"));
        return;
    }
    let report = |configure: &dyn Fn(TestProcess) -> TestProcess| {
        let base = TestProcess::current(NAME)
            .unwrap()
            .nocapture()
            .env(REPORT, "1")
            .env_remove(COVERAGE);
        let output = configure(base).output().unwrap();
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|line| line.starts_with("true ") || line.starts_with("false "))
            .map(str::to_owned)
            .unwrap()
    };
    assert_eq!(
        report(&|process| process.env("RUSTC_WRAPPER", "")),
        "false None"
    );
    assert_eq!(
        report(&|process| process
            .env("RUSTC_WRAPPER", "/inherited")
            .env(COVERAGE, "1")),
        "false None"
    );
    assert_eq!(
        report(&|process| process
            .env("RUSTC_WRAPPER", "/inherited")
            .env(COVERAGE, "1")
            .env("__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING", "/restored")),
        "true Some(\"/restored\")"
    );
}

/// The run's prepared sources resolve a registry crate without the shared package cache.
#[test]
fn prepared_sources_resolve_registry_crates_offline() {
    let root = Scratch::new().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "").unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = 'registry-fixture'\nversion = '0.0.0'\nedition = '2024'\n\
         [workspace]\n[dependencies]\nserde = '=1.0.229'\n",
    )
    .unwrap();
    let sources = recipe_support::prepared_sources();
    let output = toolchain_support::fixture_command(env!("CARGO"), root.path(), Some(&sources))
        .unwrap()
        .args(["metadata", "--format-version", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        metadata["packages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|package| package["name"] == "serde")
    );
}

/// Nextest runs the setup script and exports its configuration before test admission.
#[test]
fn nextest_exports_prepared_sources_to_registry_fixtures() {
    if std::env::var_os("NEXTEST").is_none() {
        return;
    }
    let config = std::env::var_os("MAESTRO_TEST_CARGO_SOURCES")
        .expect("the fixture-sources setup script must export its configuration");
    assert!(std::path::Path::new(&config).is_file());
}

/// Write a dependency-free package into a workspace checkout.
fn write_package(workspace: &Workspace, dependencies: &str) {
    fs::create_dir_all(workspace.path().join("src")).unwrap();
    fs::write(workspace.path().join("src/lib.rs"), "").unwrap();
    fs::write(
        workspace.path().join("Cargo.toml"),
        format!(
            "[package]\nname = 'workspace-fixture'\nversion = '0.0.0'\nedition = '2024'\n\
             [workspace]\n[dependencies]\n{dependencies}"
        ),
    )
    .unwrap();
}

/// Workspace commands use a Cargo home beside the checkout, not the shared one.
#[test]
fn workspace_commands_use_a_private_cargo_home() {
    let workspace = Workspace::new();
    let command = workspace.toolchain_command(env!("CARGO"));
    let home = command
        .get_envs()
        .find(|(key, _)| *key == "CARGO_HOME")
        .and_then(|(_, value)| value);
    assert_eq!(
        home,
        Some(workspace.path().with_file_name("cargo-home").as_os_str())
    );
}

/// A workspace that asks for the prepared sources resolves registry crates from them.
#[test]
fn workspace_registry_fixtures_resolve_prepared_sources() {
    let workspace = Workspace::new();
    write_package(&workspace, "serde = '=1.0.229'\n");
    workspace.use_prepared_sources();
    let output = workspace
        .toolchain_command(env!("CARGO"))
        .arg("generate-lockfile")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fs::read_to_string(workspace.path().join("Cargo.lock"))
            .unwrap()
            .contains("name = \"serde\"")
    );
    assert!(
        workspace
            .path()
            .with_file_name("cargo-home/config.toml")
            .is_file(),
        "registry fixtures must resolve through their private source configuration"
    );
}

/// Recipe workspaces finish while the shared Cargo home is unusable, so none can wait on it.
#[test]
fn workspace_fixtures_never_reach_the_shared_cargo_home() {
    if !recipe_support::without_shared_cargo_home() {
        recipe_support::rerun_without_shared_cargo_home(
            "workspace_fixtures_never_reach_the_shared_cargo_home",
        );
        return;
    }
    let workspace = Workspace::new();
    for dependencies in ["", "serde = '=1.0.229'\n"] {
        write_package(&workspace, dependencies);
        workspace.use_prepared_sources();
        assert!(
            workspace
                .toolchain_command(env!("CARGO"))
                .arg("generate-lockfile")
                .status()
                .unwrap()
                .success()
        );
    }
}

/// Cargo loads every vendored package on each run, so the shared set holds only the fixtures'
/// closure.
#[test]
fn prepared_sources_hold_only_the_fixture_closure() {
    let config: toml::Table = fs::read_to_string(recipe_support::prepared_sources())
        .unwrap()
        .parse()
        .unwrap();
    let directory = config["source"]["vendored-sources"]["directory"]
        .as_str()
        .unwrap();
    let packages = fs::read_dir(directory).unwrap().count();
    assert!(packages <= 100, "{packages} vendored packages");
}

/// Read one manifest below the workspace's crates directory.
fn read_manifest(path: &str) -> toml::Table {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path))
        .unwrap()
        .parse()
        .unwrap()
}

/// Names of the dependencies a manifest declares, including its target-specific tables.
fn declared_dependencies(manifest: &toml::Table) -> Vec<&String> {
    let targets = manifest
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::values)
        .filter_map(toml::Value::as_table);
    std::iter::once(manifest)
        .chain(targets)
        .flat_map(|table| {
            ["dependencies", "dev-dependencies"]
                .into_iter()
                .filter_map(|kind| table.get(kind))
        })
        .filter_map(toml::Value::as_table)
        .flat_map(toml::Table::keys)
        .collect()
}

/// Every registry crate the fixture workspaces resolve is named in the closure manifest.
#[test]
fn registry_sources_name_every_tooling_dependency() {
    let closure = read_manifest("maestro-tooling/tests/fixtures/registry_sources/Cargo.toml");
    let named = closure["dependencies"].as_table().unwrap();
    for owner in [
        "maestro-tooling",
        "maestro-path",
        "maestro-test-process",
        "maestro-test-directory",
    ] {
        let manifest = read_manifest(&format!("{owner}/Cargo.toml"));
        for name in declared_dependencies(&manifest) {
            assert!(
                name.starts_with("maestro-") || named.contains_key(name),
                "{owner} declares {name}, which the closure manifest does not name"
            );
        }
    }
}
