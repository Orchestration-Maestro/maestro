#![cfg(test)]

#[cfg(unix)]
use crate::toolchain_support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::recipe_support;
#[cfg(unix)]
use recipe_support::ProcessGroup;
use recipe_support::{Workspace, install_executable, write_input};

#[test]
fn maestro_workspace_recipes_keep_phase_order() {
    let workspace = Workspace::new();
    let result = workspace.run("prepublish");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        workspace.log(),
        "cargo clean\ncargo run|--quiet|--locked|-p|maestro-models|--example|generate_catalog_data\ncargo build|--workspace|--locked\ncargo fmt|--all\ncargo hakari|verify\ncargo clippy|--workspace|--all-targets|--locked|--|-D|warnings\ncargo doc|--workspace|--no-deps|--locked\ncargo test|-p|maestro-test-conventions|--locked|--test|integration|--|--exact|architecture::real_workspace_obeys_crate_rules|dependency_locks::maestro_locked_resolution_keeps_checkout_clean\n"
    );
    fs::remove_file(workspace.path().join("log")).unwrap();
    let result = workspace
        .command("prepublish")
        .env("MAESTRO_FAIL", "build")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(
        workspace.log(),
        "cargo clean\ncargo run|--quiet|--locked|-p|maestro-models|--example|generate_catalog_data\ncargo build|--workspace|--locked\n"
    );
    for phase in ["fmt", "hakari", "clippy", "doc", "test"] {
        fs::remove_file(workspace.path().join("log")).unwrap();
        let output = workspace
            .command("ci")
            .env("MAESTRO_FAIL", phase)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let log = workspace.log();
        assert!(
            log.lines()
                .last()
                .unwrap()
                .starts_with(&format!("cargo {phase}"))
        );
        assert!(!log.contains("cargo nextest") && !log.contains("cargo test|--workspace"));
        if phase == "hakari" {
            assert!(!log.contains("cargo clippy"), "{log}");
        }
    }
    assert_test_phase_failures(&workspace);
    fs::remove_file(workspace.path().join("log")).unwrap();
    assert!(workspace.run("ci").status.success());
    assert!(workspace.log().ends_with(
        "cargo nextest|run|--workspace|--locked\ncargo test|--workspace|--locked|--doc\n"
    ));
}

#[test]
fn maestro_scoped_check_consumes_package_selection() {
    let workspace = Workspace::new();
    let result = workspace
        .command("_check")
        .args(["-p", "maestro-models", "-p", "maestro-agent"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        workspace.log(),
        "cargo hakari|verify\ncargo clippy|-p|maestro-models|-p|maestro-agent|--all-targets|--locked|--|-D|warnings\ncargo doc|-p|maestro-models|-p|maestro-agent|--no-deps|--locked\ncargo test|-p|maestro-test-conventions|--locked|--test|integration|--|--exact|architecture::real_workspace_obeys_crate_rules|dependency_locks::maestro_locked_resolution_keeps_checkout_clean\n"
    );
    fs::remove_file(workspace.path().join("log")).unwrap();
    let result = workspace
        .command("_check")
        .args(["-p", "maestro-models"])
        .env("MAESTRO_FAIL", "clippy")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(
        workspace.log(),
        "cargo hakari|verify\ncargo clippy|-p|maestro-models|--all-targets|--locked|--|-D|warnings\n"
    );
}

/// Checks that test phases stop on failure.
fn assert_test_phase_failures(workspace: &Workspace) {
    for (phase, expected) in [
        ("nextest", "cargo nextest|run|--workspace|--locked\n"),
        (
            "test",
            "cargo nextest|run|--workspace|--locked\ncargo test|--workspace|--locked|--doc\n",
        ),
    ] {
        fs::remove_file(workspace.path().join("log")).unwrap();
        let output = workspace
            .command("test")
            .env("MAESTRO_FAIL", phase)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(workspace.log(), expected);
    }
}

#[test]
fn maestro_package_recipes_select_their_owners() {
    for (prefix, owner) in [
        ("models", "maestro-models"),
        ("agent", "maestro-agent"),
        ("tui", "maestro-tui"),
        ("app", "maestro-app"),
    ] {
        let workspace = Workspace::new();
        let output = workspace.run(&format!("{prefix}-test"));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            workspace.log(),
            format!(
                "cargo nextest|run|-p|{owner}|--locked\ncargo test|-p|{owner}|--locked|--doc\n"
            )
        );
        assert_package_build(&workspace, prefix, owner);
    }
    assert_binary_build_order();
}

#[test]
#[cfg_attr(windows, ignore = "recipes lose single quotes on Windows (#2949)")]
fn maestro_test_recipes_forward_literal_arguments() {
    for (recipe, selection, doctests) in [
        ("test", "--workspace|--locked", true),
        ("models-test", "-p|maestro-models|--locked", true),
        ("agent-test", "-p|maestro-agent|--locked", true),
        ("tui-test", "-p|maestro-tui|--locked", true),
        ("app-test", "-p|maestro-app|--locked", true),
        (
            "tui-test-ansi",
            "-p|maestro-tui|--locked|--test|integration|ansi_wrapping::",
            false,
        ),
    ] {
        let workspace = Workspace::new();
        let args: Vec<_> = include_str!("fixtures/literal-test-arguments.txt")
            .lines()
            .collect();
        let output = workspace.command(recipe).args(&args).output().unwrap();
        assert!(output.status.success(), "{recipe}");
        let suffix = args.join("|");
        let expected = if doctests {
            format!(
                "cargo nextest|run|{selection}|{suffix}\ncargo test|{selection}|--doc|{suffix}\n"
            )
        } else {
            format!("cargo nextest|run|{selection}|{suffix}\n")
        };
        assert_eq!(workspace.log(), expected, "{recipe}");
    }
}

fn assert_package_build(workspace: &Workspace, prefix: &str, owner: &str) {
    for recipe in ["build", "prepublish", "clean"] {
        fs::remove_file(workspace.path().join("log")).unwrap();
        assert!(
            workspace
                .run(&format!("{prefix}-{recipe}"))
                .status
                .success()
        );
        let log = workspace.log();
        assert!(!log.contains("clippy"));
        if recipe == "build" && prefix == "models" {
            assert!(log.starts_with(
                "cargo run|--quiet|--locked|-p|maestro-models|--example|generate_catalog_data\n"
            ));
            assert!(log.ends_with("cargo build|-p|maestro-models|--locked\n"));
        } else if recipe == "build" {
            assert!(log.starts_with(&format!("cargo build|-p|{owner}|--locked\n")));
        } else {
            assert!(log.starts_with(&format!("cargo clean|-p|{owner}\n")));
        }
        if prefix == "app" && recipe != "clean" {
            let compile = log.find("cargo build|-p|maestro-app|--locked").unwrap();
            assert!(compile < log.find("--bin|development|--|copy-assets").unwrap());
        }
    }
}

fn assert_binary_build_order() {
    let workspace = Workspace::new();
    assert!(workspace.run("build-binary").status.success());
    let log = workspace.log();
    let mut previous = 0;
    for package in [
        "maestro-tui",
        "maestro-models",
        "maestro-agent",
        "maestro-app",
        "maestro",
    ] {
        let offset = log
            .find(&format!("cargo build|-p|{package}|--locked"))
            .unwrap();
        assert!(offset >= previous);
        previous = offset;
    }
    assert!(log.find("--bin|development|--|copy-binary-assets").unwrap() > previous);
}

fn native_workspace(workspace: &Workspace) {
    fs::write(
        workspace.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    for owner in [
        "maestro-models",
        "maestro-agent",
        "maestro-tui",
        "maestro-app",
    ] {
        let root = workspace.path().join("crates").join(owner);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!("[package]\nname = '{owner}'\nversion = '0.1.0'\nedition = '2024'\n"),
        )
        .unwrap();
        fs::write(root.join("src/lib.rs"), "/// Adds one.\n/// ```\n/// assert_eq!(maestro_models::increment(1), 2);\n/// ```\npub fn increment(n: i32) -> i32 { n + 1 }\n#[test] fn selected_behavior() {}\n#[test] #[ignore] fn ignored_behavior() {}\n#[test] fn failing_behavior() { if std::env::var_os(\"MAESTRO_TEST_FAIL\").is_some() { panic!(\"controlled failure\"); } }\n".replace("maestro_models", &owner.replace('-', "_"))).unwrap();
    }
    fs::create_dir_all(workspace.path().join("crates/maestro-models/examples")).unwrap();
    fs::write(
        workspace
            .path()
            .join("crates/maestro-models/examples/generate_catalog_data.rs"),
        "fn main() {}\n",
    )
    .unwrap();
    // The installed fixture programs carry the host's executable suffix.
    let suffix = std::env::consts::EXE_SUFFIX;
    fs::rename(
        workspace.path().join(format!("bin/cargo{suffix}")),
        workspace.path().join(format!("bin/cargo-proxy{suffix}")),
    )
    .unwrap();
    install_executable(
        Path::new(env!("CARGO")),
        &workspace.path().join(format!("bin/cargo{suffix}")),
    );
    assert!(
        workspace
            .toolchain_command(env!("CARGO"))
            .arg("generate-lockfile")
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn maestro_clean_keeps_sources_and_sibling_outputs() {
    let workspace = Workspace::new();
    native_workspace(&workspace);
    fs::create_dir_all(workspace.path().join(".cargo")).unwrap();
    fs::write(
        workspace.path().join(".cargo/config.toml"),
        "[build]\ntarget-dir = 'compiler-output'\n",
    )
    .unwrap();
    assert!(workspace.run("build").status.success());
    let development = prepare_configured_assets(&workspace);
    assert!(
        workspace
            .path()
            .join("compiler-output/debug/libmaestro_models.rlib")
            .exists()
    );
    assert!(
        workspace
            .command("models-clean")
            .env("MAESTRO_REAL_CARGO", env!("CARGO"))
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !workspace
            .path()
            .join("compiler-output/debug/libmaestro_models.rlib")
            .exists()
    );
    assert!(
        workspace
            .path()
            .join("compiler-output/debug/libmaestro_agent.rlib")
            .exists()
    );
    assert!(
        workspace
            .path()
            .join("crates/maestro-models/src/lib.rs")
            .exists()
    );
    assert_asset_cleanup(&workspace, &development);
}

fn assert_asset_cleanup(workspace: &Workspace, development: &Path) {
    for tree in ["maestro-app-assets", "maestro-binary-assets"] {
        assert!(workspace.path().join("compiler-output").join(tree).is_dir());
    }
    let output = workspace
        .command("app-clean")
        .env("MAESTRO_REAL_CARGO", env!("CARGO"))
        .env("MAESTRO_DEVELOPMENT", development)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        workspace
            .path()
            .join("compiler-output/debug/libmaestro_agent.rlib")
            .exists()
    );
    assert!(
        workspace
            .path()
            .join("crates/maestro-app/src/lib.rs")
            .exists()
    );
    for tree in ["maestro-app-assets", "maestro-binary-assets"] {
        assert!(!workspace.path().join("compiler-output").join(tree).exists());
        fs::create_dir_all(workspace.path().join("compiler-output").join(tree)).unwrap();
    }
    assert!(
        workspace
            .command("clean")
            .env("MAESTRO_REAL_CARGO", env!("CARGO"))
            .output()
            .unwrap()
            .status
            .success()
    );
    for tree in ["maestro-app-assets", "maestro-binary-assets"] {
        assert!(!workspace.path().join("compiler-output").join(tree).exists());
    }
    assert!(
        !workspace
            .path()
            .join("compiler-output/debug/libmaestro_agent.rlib")
            .exists()
    );
}

fn prepare_configured_assets(workspace: &Workspace) -> PathBuf {
    let suffix = std::env::consts::EXE_SUFFIX;
    fs::rename(
        workspace.path().join(format!("bin/cargo-proxy{suffix}")),
        workspace.path().join(format!("bin/cargo{suffix}")),
    )
    .unwrap();
    create_prepared_assets(workspace);
    let binary_root = workspace.path().join("crates/maestro");
    fs::create_dir_all(binary_root.join("src")).unwrap();
    fs::write(
        binary_root.join("Cargo.toml"),
        "[package]\nname = 'maestro'\nversion = '0.1.0'\nedition = '2024'\n",
    )
    .unwrap();
    fs::write(binary_root.join("src/main.rs"), "fn main() {}\n").unwrap();
    prepare_tooling_owner(workspace);
    let development = workspace.path().join("compiler-output/debug/development");
    for recipe in ["copy-assets", "copy-binary-assets"] {
        let output = workspace
            .command(recipe)
            .env("MAESTRO_REAL_CARGO", env!("CARGO"))
            .env("MAESTRO_DEVELOPMENT", &development)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for tree in ["maestro-app-assets", "maestro-binary-assets"] {
        assert!(
            workspace.path().join("compiler-output").join(tree).is_dir(),
            "missing configured asset tree: {tree}"
        );
    }
    let standalone = workspace
        .path()
        .join("compiler-output/maestro-binary-assets");
    for (name, bytes) in [
        ("README.md", "readme"),
        ("CHANGELOG.md", "history"),
        ("docs/guide.md", "guide"),
        ("examples/example.txt", "example"),
    ] {
        assert_eq!(
            fs::read(standalone.join(name)).unwrap(),
            bytes.as_bytes(),
            "standalone layout lacks package input {name}"
        );
    }
    development
}

fn create_prepared_assets(workspace: &Workspace) {
    let root = workspace.path().join("crates/maestro-app");
    for (name, value) in [
        ("src/modes/interactive/theme/theme.json", "{}"),
        ("src/modes/interactive/assets/logo.png", "image"),
        ("src/core/export-html/template.html", "html"),
        ("src/core/export-html/template.css", "css"),
        ("src/core/export-html/template.js", "js"),
        ("src/core/export-html/vendor/vendor.js", "vendor"),
    ] {
        write_input(&root, name, value);
    }
    for (name, value) in [
        ("README.md", "readme"),
        ("CHANGELOG.md", "history"),
        ("docs/guide.md", "guide"),
        ("examples/example.txt", "example"),
    ] {
        write_input(&root, name, &format!("crate-{value}"));
        write_input(&workspace.path(), name, value);
    }
    fs::create_dir_all(workspace.path().join("target/maestro-viewer")).unwrap();
    fs::write(
        workspace.path().join("target/maestro-viewer/index.html"),
        "viewer",
    )
    .unwrap();
}

#[cfg(unix)]
#[test]
fn maestro_watch_rebuilds_source_and_retains_output() {
    assert_watch_rebuild("dev", "crates/maestro-models/src/lib.rs");
}

#[cfg(unix)]
#[test]
fn maestro_selected_watch_rebuilds_sibling_dependencies() {
    assert_watch_rebuild("models-dev", "crates/maestro-agent/src/lib.rs");
}

#[cfg(unix)]
#[test]
fn maestro_selected_watch_rebuilds_cargo_configuration() {
    assert_watch_rebuild("models-dev", ".cargo/config.toml");
}

#[cfg(unix)]
#[test]
fn maestro_watch_survives_rebuilding_its_development_binary() {
    assert_watch_rebuild("dev", "crates/maestro-tooling/src/bin/development.rs");
}

#[cfg(unix)]
fn assert_watch_rebuild(recipe: &str, changed: &str) {
    let workspace = Workspace::new();
    prepare_watch_workspace(&workspace);
    prepare_tooling_owner(&workspace);
    let binary = workspace.path().join("target/debug/development");
    let (child, receive, threads) = start_watch(&workspace, workspace.command(recipe), &binary);
    let diagnostics = watcher_diagnostics(&workspace.path());
    assert_watch_events(&workspace, changed, &receive, &diagnostics);
    drop(child);
    drop(receive);
    threads.into_iter().for_each(recipe_support::join_reader);
}

/// Isolates profiles before launch because cleanup can forcibly stop the owned group.
#[cfg(unix)]
fn start_watch(
    workspace: &Workspace,
    mut command: Command,
    binary: &Path,
) -> (
    ProcessGroup,
    std::sync::mpsc::Receiver<String>,
    [std::thread::JoinHandle<()>; 1],
) {
    let profiles = workspace.path().join("target/watch-profiles");
    fs::create_dir_all(&profiles).unwrap();
    command.env("LLVM_PROFILE_FILE", profiles.join("watch-%p-%m.profraw"));
    command
        .env("MAESTRO_REAL_CARGO", env!("CARGO"))
        .env("MAESTRO_DEVELOPMENT", binary)
        .env("MAESTRO_REAL_WATCHEXEC", watcher_path());
    let (child, receive, thread) = recipe_support::start_output(command);
    (child, receive, [thread])
}

/// Completion follows retained stdout and stderr from the same command.
#[cfg(unix)]
#[test]
fn maestro_watch_completion_retains_both_output_streams() {
    use std::io::Write;
    let workspace = Workspace::new();
    writeln!(
        fs::OpenOptions::new().append(true).open(workspace.path().join("justfile")).unwrap(),
        "\nwatch-output:\n    @printf 'source output\\n'\n    @printf 'compiler output\\nFinished `dev`\\n[Command was successful]\\n' >&2"
    ).unwrap();
    let (child, receive, threads) = start_watch(
        &workspace,
        workspace.command("watch-output"),
        Path::new(env!("CARGO_BIN_EXE_development")),
    );
    let mut transcript = String::new();
    wait_for_build(
        &receive,
        &mut transcript,
        "output completion",
        "controlled command",
    );
    assert!(
        transcript.starts_with("source output\ncompiler output\n"),
        "{transcript}"
    );
    drop(child);
    drop(receive);
    threads.into_iter().for_each(recipe_support::join_reader);
}

/// Panic cleanup closes every descendant's output pipe, not just the recipe leader's.
#[cfg(unix)]
#[test]
fn maestro_watch_stops_its_children_on_panic() {
    let workspace = Workspace::new();
    prepare_watch_workspace(&workspace);
    let (child, receive, threads) = start_watch(
        &workspace,
        workspace.command("models-dev"),
        Path::new(env!("CARGO_BIN_EXE_development")),
    );
    wait_for_build(
        &receive,
        &mut String::new(),
        "panic cleanup readiness",
        &watcher_diagnostics(&workspace.path()),
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _child = child;
        panic!("controlled panic after starting the watch");
    }));
    close_watch_output(receive, threads);
    assert!(panic.is_err());
}

/// Real instrumented descendants write outside merge inputs, even with literal checkout paths.
#[cfg(unix)]
#[test]
fn maestro_watch_profiles_are_isolated_before_group_start() {
    let mut workspace = Workspace::new();
    let literal = workspace.path().with_extension("space ' checkout");
    workspace.rename(literal.file_name().unwrap()).unwrap();
    let binary = prepare_profile_writer(&workspace);
    let merge = workspace.path().join("merge-profiles");
    let control = profile_control(&binary, &merge);
    for incoming in [
        Some(merge.join("incoming-%p-%m.profraw")),
        None,
        Some(PathBuf::new()),
    ] {
        let scratch = workspace.path().join("target/watch-profiles");
        if scratch.exists() {
            fs::remove_dir_all(&scratch).unwrap();
        }
        let mut command = Command::new(&binary);
        command.env_remove("LLVM_PROFILE_FILE");
        if let Some(value) = incoming {
            command.env("LLVM_PROFILE_FILE", value);
        }
        let (child, receive, threads) = start_watch(&workspace, command, &binary);
        wait_for_marker(
            &receive,
            &mut String::new(),
            "maestro-profile-child-exited",
            "profile producer",
        );
        let produced = scratch.is_dir().then(|| profile_snapshot(&scratch));
        drop(child);
        close_watch_output(receive, threads);
        assert_eq!(
            profile_snapshot(&merge),
            control,
            "watch group contaminated merge inputs"
        );
        assert!(
            produced.is_some_and(|files| !files.is_empty()),
            "missing instrumented descendant profile"
        );
    }
    let scratch = workspace.path().join("target/watch-profiles");
    fs::remove_dir_all(&scratch).unwrap();
    fs::write(&scratch, "not a directory").unwrap();
    let failure = std::panic::catch_unwind(|| {
        let mut command = Command::new(&binary);
        command
            .arg("exit")
            .env("LLVM_PROFILE_FILE", merge.join("failure-%p-%m.profraw"));
        start_watch(&workspace, command, &binary)
    });
    assert!(failure.is_err(), "profile setup must fail before spawning");
    assert_eq!(profile_snapshot(&merge), control);
}

/// Unwinding stops the instrumented group without adding profiles to merge inputs.
#[cfg(unix)]
#[test]
fn maestro_watch_profile_isolation_survives_panic_cleanup() {
    let workspace = Workspace::new();
    let binary = prepare_profile_writer(&workspace);
    let merge = workspace.path().join("merge-profiles");
    let control = profile_control(&binary, &merge);
    let mut command = Command::new(&binary);
    command.env("LLVM_PROFILE_FILE", merge.join("incoming-%p-%m.profraw"));
    let (child, receive, threads) = start_watch(&workspace, command, &binary);
    wait_for_marker(
        &receive,
        &mut String::new(),
        "maestro-profile-child-exited",
        "profile producer",
    );
    let scratch = workspace.path().join("target/watch-profiles");
    let produced = scratch.is_dir().then(|| profile_snapshot(&scratch));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _child = child;
        panic!("controlled panic after profile child exit");
    }));
    close_watch_output(receive, threads);
    assert!(panic.is_err());
    assert_eq!(
        profile_snapshot(&merge),
        control,
        "panic cleanup contaminated merge inputs"
    );
    assert!(
        produced.is_some_and(|files| !files.is_empty()),
        "missing instrumented descendant profile"
    );
}

/// Compiles the finite profile producer even when the integration test is uninstrumented.
#[cfg(unix)]
fn prepare_profile_writer(workspace: &Workspace) -> PathBuf {
    let source = workspace.path().join("profile_writer.rs");
    fs::write(&source, include_str!("fixtures/profile_writer.rs")).unwrap();
    let binary = workspace.path().join("profile_writer");
    assert!(
        toolchain_support::command("rustc")
            .arg("-C")
            .arg("instrument-coverage")
            .arg(source)
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    binary
}

/// Proves ordinary commands retain profiling and returns the completed merge input snapshot.
#[cfg(unix)]
fn profile_control(binary: &Path, merge: &Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    fs::create_dir(merge).unwrap();
    assert!(
        Command::new(binary)
            .arg("exit")
            .env("LLVM_PROFILE_FILE", merge.join("control-%p-%m.profraw"))
            .status()
            .unwrap()
            .success()
    );
    let files = profile_snapshot(merge);
    assert!(
        !files.is_empty(),
        "ordinary command must emit a real profile"
    );
    files
}

/// Reads completed native profiles without interpreting LLVM names or bytes.
#[cfg(unix)]
fn profile_snapshot(directory: &Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let mut files: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let contents = fs::read(entry.path()).unwrap();
            assert!(!contents.is_empty(), "empty native profile");
            (entry.file_name(), contents)
        })
        .collect();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

/// Drains output to disconnection after teardown and joins the reader.
#[cfg(unix)]
fn close_watch_output(
    receive: std::sync::mpsc::Receiver<String>,
    threads: [std::thread::JoinHandle<()>; 1],
) {
    recipe_support::record("phase: draining watch output");
    while receive.recv().is_ok() {}
    drop(receive);
    threads.into_iter().for_each(recipe_support::join_reader);
}

#[cfg(unix)]
fn prepare_watch_workspace(workspace: &Workspace) {
    native_workspace(workspace);
    for (owner, marker) in [("maestro-models", "source"), ("maestro-agent", "sibling")] {
        let path = workspace.path().join(format!("crates/{owner}/src/lib.rs"));
        let contents = fs::read_to_string(&path).unwrap();
        fs::write(
            path,
            format!("{contents}\npub const MARKER: &str = \"maestro-initial-{marker}\";\n"),
        )
        .unwrap();
    }
    fs::write(
        workspace.path().join("crates/maestro-models/src/main.rs"),
        "fn main() { println!(\"{}\\n{}\\n{}\", maestro_models::MARKER, maestro_agent::MARKER, env!(\"MAESTRO_REBUILD_MARKER\")); }\n",
    ).unwrap();
    fs::write(workspace.path().join("crates/maestro-models/Cargo.toml"), "[package]\nname = 'maestro-models'\nversion = '0.1.0'\nedition = '2024'\n[dependencies]\nmaestro-agent = { path = '../maestro-agent' }\n").unwrap();
    assert!(
        workspace
            .toolchain_command(env!("CARGO"))
            .arg("generate-lockfile")
            .status()
            .unwrap()
            .success()
    );
    fs::create_dir(workspace.path().join(".cargo")).unwrap();
    fs::write(
        workspace.path().join(".cargo/config.toml"),
        "[build]\ntarget-dir = 'target'\n[env]\nMAESTRO_REBUILD_MARKER = 'maestro-initial-config'\n",
    )
    .unwrap();
    fs::create_dir_all(workspace.path().join("target")).unwrap();
    let suffix = std::env::consts::EXE_SUFFIX;
    fs::rename(
        workspace.path().join(format!("bin/cargo-proxy{suffix}")),
        workspace.path().join(format!("bin/cargo{suffix}")),
    )
    .unwrap();
}

fn prepare_tooling_owner(workspace: &Workspace) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = workspace.path().join("crates/maestro-tooling");
    copy_tree(&source.join("src"), &root.join("src"));
    copy_patched_sources(&source.join("../.."), &workspace.path());
    let main = root.join("src/bin/development.rs");
    let contents = fs::read_to_string(&main).unwrap().replace(
        "fn main() -> std::process::ExitCode {",
        "fn main() -> std::process::ExitCode {\n    if std::env::args().any(|arg| arg == \"--rebuild-marker\") { println!(\"maestro-initial-binary\"); return std::process::ExitCode::SUCCESS; }",
    );
    let contents = contents.replace(
        "std::path::Path::new(env!(\"CARGO\"))",
        "std::path::Path::new(\"bin/cargo\")",
    );
    fs::write(main, contents).unwrap();
    let unifier = workspace.path().join("crates/maestro-dependencies");
    fs::create_dir_all(unifier.join("src")).unwrap();
    fs::write(unifier.join("src/lib.rs"), "").unwrap();
    fs::write(
        unifier.join("Cargo.toml"),
        "[package]\nname = 'maestro-dependencies'\nversion = '0.1.0'\nedition = '2024'\n[lints]\nworkspace = true\n",
    )
    .unwrap();
    fs::copy(source.join("Cargo.toml"), root.join("Cargo.toml")).unwrap();
    for owner in [
        "maestro-test-process",
        "maestro-test-directory",
        "maestro-path",
    ] {
        let dependency = workspace.path().join("crates").join(owner);
        copy_tree(
            &source.join("..").join(owner).join("src"),
            &dependency.join("src"),
        );
        fs::copy(
            source.join("..").join(owner).join("Cargo.toml"),
            dependency.join("Cargo.toml"),
        )
        .unwrap();
    }
    fs::copy(
        source.join("../../Cargo.toml"),
        workspace.path().join("Cargo.toml"),
    )
    .unwrap();
    assert!(
        workspace
            .toolchain_command(env!("CARGO"))
            .args(["build", "-p", "maestro-tooling", "--bin", "development"])
            .status()
            .unwrap()
            .success()
    );
}

/// Copy local dependency patches from the source manifest into a build fixture.
fn copy_patched_sources(source: &Path, destination: &Path) {
    let manifest: toml::Table = fs::read_to_string(source.join("Cargo.toml"))
        .unwrap()
        .parse()
        .unwrap();
    let Some(patches) = manifest.get("patch") else {
        return;
    };
    for registry in patches.as_table().unwrap().values() {
        for patch in registry.as_table().unwrap().values() {
            if let Some(path) = patch.get("path").and_then(toml::Value::as_str) {
                copy_tree(&source.join(path), &destination.join(path));
            }
        }
    }
}

#[test]
fn patched_sources_accept_manifests_without_patches() {
    let source = Workspace::new();
    let destination = Workspace::new();
    fs::write(source.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    let entries = fs::read_dir(destination.path()).unwrap().count();
    copy_patched_sources(&source.path(), &destination.path());
    assert_eq!(fs::read_dir(destination.path()).unwrap().count(), entries);
}

#[test]
fn patched_sources_follow_manifest_paths() {
    let source = Workspace::new();
    let destination = Workspace::new();
    fs::write(
        source.path().join("Cargo.toml"),
        r#"
[patch.crates-io]
first = { path = "vendor/alpha" }
remote = { git = "https://example.invalid/remote" }
[patch."https://example.invalid/registry"]
second = { path = "local/beta" }
"#,
    )
    .unwrap();
    for (path, bytes) in [
        ("vendor/alpha/nested/input", b"first".as_slice()),
        ("local/beta/nested/input", b"second".as_slice()),
        ("unrelated/nested/input", b"excluded".as_slice()),
    ] {
        let file = source.path().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }
    copy_patched_sources(&source.path(), &destination.path());
    assert_eq!(
        fs::read(destination.path().join("vendor/alpha/nested/input")).unwrap(),
        b"first"
    );
    assert_eq!(
        fs::read(destination.path().join("local/beta/nested/input")).unwrap(),
        b"second"
    );
    assert!(!destination.path().join("unrelated").exists());
    assert!(!destination.path().join("remote").exists());
}

/// Copy every file beneath a source directory to the matching destination path.
///
/// # Errors
/// Filesystem errors panic so fixture preparation cannot silently omit an input.
pub fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

#[cfg(unix)]
fn assert_watch_events(
    workspace: &Workspace,
    changed: &str,
    receive: &std::sync::mpsc::Receiver<String>,
    diagnostics: &str,
) {
    let mut transcript = String::new();
    wait_for_build(receive, &mut transcript, "initial build", diagnostics);
    let marker = mutate_watch_input(workspace, changed);
    wait_for_marker(receive, &mut transcript, &marker, diagnostics);
    if changed.starts_with("crates/maestro-tooling/") {
        let marker = mutate_watch_input(workspace, "crates/maestro-agent/src/lib.rs");
        wait_for_marker(receive, &mut transcript, &marker, diagnostics);
    }
    assert!(!transcript.contains("\u{1b}[2J"));
}

/// Gives each input mutation a marker only its compiled result can print.
#[cfg(unix)]
fn mutate_watch_input(workspace: &Workspace, changed: &str) -> String {
    let input = if changed == ".cargo/config.toml" {
        "config"
    } else if changed.starts_with("crates/maestro-tooling/") {
        "binary"
    } else if changed.starts_with("crates/maestro-agent/") {
        "sibling"
    } else {
        "source"
    };
    let marker = format!(
        "maestro-rebuild-{}-{input}",
        workspace.path().file_name().unwrap().to_str().unwrap()
    );
    let path = workspace.path().join(changed);
    let contents = fs::read_to_string(&path).unwrap();
    let contents = contents.replace(&format!("maestro-initial-{input}"), &marker);
    fs::write(path, contents).unwrap();
    marker
}

/// Waits for the output associated with this mutation, not a queued older build.
#[cfg(unix)]
fn wait_for_marker(
    receive: &std::sync::mpsc::Receiver<String>,
    transcript: &mut String,
    marker: &str,
    diagnostics: &str,
) {
    recipe_support::record(format!("phase: waiting for marker {marker:?}"));
    while !has_rebuild_marker(transcript, marker) {
        let line = receive.recv().unwrap_or_else(|error| {
            panic!("marker {marker}: {error}; {diagnostics}; watcher output:\n{transcript}")
        });
        transcript.push_str(&line);
        transcript.push('\n');
    }
    recipe_support::record(format!("phase: observed marker {marker:?}"));
}

/// Unrelated successful builds cannot witness a changed input.
#[test]
fn maestro_watch_rebuild_requires_its_mutation_marker() {
    let unrelated = "Finished `dev` profile\n[Command was successful]\n";
    assert!(!has_rebuild_marker(unrelated, "maestro-rebuild-source-1"));
    let observed = format!("{unrelated}maestro-rebuild-source-1\n");
    assert!(has_rebuild_marker(&observed, "maestro-rebuild-source-1"));
}

/// Recognizes output witnessing the compilation of a changed input.
fn has_rebuild_marker(transcript: &str, marker: &str) -> bool {
    transcript.lines().any(|line| line == marker)
}

/// Finds the pinned external watcher before fixture commands extend PATH.
#[cfg(unix)]
fn watcher_path() -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|directory| directory.join("watchexec"))
        .find(|path| path.is_file())
        .unwrap()
}

#[cfg(unix)]
fn watcher_diagnostics(root: &Path) -> String {
    let path = watcher_path();
    let version = Command::new(&path).arg("--version").output().unwrap();
    let mounts = fs::read_to_string("/proc/mounts").unwrap_or_default();
    let filesystem = mounts
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            (fields.len() >= 3 && root.starts_with(fields[1])).then(|| (fields[1], fields[2]))
        })
        .max_by_key(|(mount, _)| mount.len())
        .map_or("unknown", |(_, filesystem)| filesystem);
    format!(
        "watcher={} version={} filesystem={filesystem}",
        path.display(),
        String::from_utf8_lossy(&version.stdout).trim()
    )
}

#[cfg(unix)]
fn wait_for_build(
    receive: &std::sync::mpsc::Receiver<String>,
    transcript: &mut String,
    phase: &str,
    diagnostics: &str,
) {
    recipe_support::record(format!("phase: waiting for build {phase:?}"));
    let started = std::time::Instant::now();
    let mut compiled = false;
    loop {
        let line = receive.recv().unwrap_or_else(|error| {
            panic!(
                "{phase}: {error} after {} ms; {diagnostics}; watcher output:\n{transcript}",
                started.elapsed().as_millis()
            )
        });
        transcript.push_str(&line);
        transcript.push('\n');
        compiled |= line.contains("Finished `dev`");
        if compiled && line.contains("[Command was successful]") {
            recipe_support::record(format!("phase: observed build {phase:?}"));
            eprintln!("{phase}: {} ms", started.elapsed().as_millis());
            return;
        }
    }
}

#[test]
fn maestro_dev_compile_excludes_generation_and_other_owners() {
    let workspace = Workspace::new();
    for recipe in [
        "dev-compile",
        "models-dev-compile",
        "models-dev",
        "agent-dev",
        "tui-dev",
        "app-dev",
    ] {
        let result = workspace.run(recipe);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let log = workspace.log();
    assert_eq!(
        log.matches("cargo pkgid|-p|maestro-models|--locked")
            .count(),
        3
    );
    for owner in [
        "maestro-models",
        "maestro-agent",
        "maestro-tui",
        "maestro-app",
    ] {
        assert!(log.contains(&format!("--|watch|build|-p|{owner}|--locked")));
    }
    assert!(!log.contains("generate"));
    assert!(!log.contains("copy"));
    assert!(!log.contains("clear"));
    assert!(log.contains("--bin|development|--|watch|build"));
    native_workspace(&workspace);
    fs::remove_dir_all(workspace.path().join("crates/maestro-models")).unwrap();
    assert!(!workspace.run("models-dev").status.success());
}

#[test]
fn maestro_native_tests_keep_filters_and_ignored_cases() {
    let workspace = Workspace::new();
    native_workspace(&workspace);
    assert_model_test_selection(&workspace);
    assert_native_doctests(&workspace);
    assert_terminal_test_selection(&workspace);
    let output = workspace
        .command("test")
        .arg("selected_behavior")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for owner in [
        "maestro-models",
        "maestro-agent",
        "maestro-tui",
        "maestro-app",
    ] {
        assert!(
            stderr.lines().any(|line| line.contains("PASS")
                && line.ends_with(&format!("{owner} selected_behavior"))),
            "{owner}"
        );
    }
}

/// Checks selected, ignored, failed and unmatched ordinary tests.
fn assert_model_test_selection(workspace: &Workspace) {
    let output = workspace
        .command("models-test")
        .arg("selected_behavior")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .any(|line| line.contains("PASS") && line.ends_with("selected_behavior"))
    );
    let output = workspace
        .command("models-test")
        .args(["--", "--ignored"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .any(|line| line.contains("PASS") && line.ends_with("ignored_behavior"))
    );
    let output = workspace
        .command("models-test")
        .arg("failing_behavior")
        .env("MAESTRO_TEST_FAIL", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("1 failed"));
    let output = workspace
        .command("models-test")
        .arg("no_such_behavior")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no tests to run"));
}

/// Exercises the actual doctest after ordinary tests and through Cargo alone.
fn assert_native_doctests(workspace: &Workspace) {
    let output = workspace.run("models-test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("2 passed, 1 skipped"));
    assert!(
        !stderr
            .lines()
            .any(|line| line.contains("PASS") && line.contains("ignored_behavior"))
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
    let output = workspace
        .toolchain_command(env!("CARGO"))
        .args(["test", "-p", "maestro-models", "--locked", "--doc"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
    let source = workspace.path().join("crates/maestro-models/src/lib.rs");
    let original = fs::read_to_string(&source).unwrap();
    fs::write(
        &source,
        original.replace("increment(1), 2", "increment(1), 3"),
    )
    .unwrap();
    let output = workspace.run("models-test");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("2 passed, 1 skipped"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("0 passed; 1 failed"));
    fs::write(source, original).unwrap();
}

/// Creates distinguishable terminal tests in one integration target.
fn terminal_test_fixture(workspace: &Workspace) {
    let real_tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("../maestro-tui/tests");
    let tests = workspace.path().join("crates/maestro-tui/tests");
    fs::create_dir_all(&tests).unwrap();
    fs::write(
        workspace.path().join("crates/maestro-tui/src/lib.rs"),
        "#[test] fn terminal_behavior() {}\n#[test] fn selected_behavior() {}\n",
    )
    .unwrap();
    for (target, source) in [
        (
            "ansi_wrapping",
            "#[test] fn wrapped_lines() { println!(\"visible wrapping output\"); }\n#[test] #[ignore] fn wrapped_ignored() {}\n",
        ),
        ("ansi_columns", "#[test] fn column_slices() {}\n"),
    ] {
        assert!(
            real_tests.join(format!("{target}.rs")).is_file(),
            "{target} is a delivered test target"
        );
        fs::write(tests.join(format!("{target}.rs")), source).unwrap();
    }
    let manifest = workspace.path().join("crates/maestro-tui/Cargo.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(
        &manifest,
        format!(
            "{}\n[[test]]\nname = \"integration\"\npath = \"tests/integration.rs\"\n",
            text.replace("[package]", "[package]\nautotests = false")
        ),
    )
    .unwrap();
    fs::write(
        tests.join("integration.rs"),
        "mod ansi_wrapping;\nmod ansi_columns;\n",
    )
    .unwrap();
}

fn assert_terminal_test_selection(workspace: &Workspace) {
    terminal_test_fixture(workspace);
    let output = workspace
        .command("tui-test")
        .args(["--", "--nocapture"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    for ran in ["terminal_behavior", "wrapped_lines", "column_slices"] {
        assert!(
            stderr
                .lines()
                .any(|line| line.contains("PASS") && line.ends_with(ran)),
            "{ran}"
        );
    }
    let output = workspace
        .command("tui-test-ansi")
        .args(["--", "--nocapture"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("terminal_behavior"));
    assert!(!stderr.contains("column_slices"));
    assert!(
        stderr
            .lines()
            .any(|line| line.contains("PASS") && line.ends_with("wrapped_lines"))
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("visible wrapping output"));
    assert!(stderr.contains("1 passed, 2 skipped"), "{stderr}");
    let output = workspace
        .command("tui-test")
        .args(include_str!("fixtures/ignored-test-arguments.txt").lines())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr
            .lines()
            .any(|line| line.contains("PASS") && line.ends_with("wrapped_ignored"))
    );
    assert!(stderr.contains("1 passed, 4 skipped"));
}

#[test]
#[cfg_attr(windows, ignore = "recipes lose single quotes on Windows (#2949)")]
fn maestro_source_recipes_preserve_invocation_directory() {
    let workspace = Workspace::new();
    let outside = workspace.path().join("outside");
    fs::create_dir(&outside).unwrap();
    for recipe in ["run-source", "run-source-windows"] {
        let output = workspace
            .command("--justfile")
            .arg(workspace.path().join("justfile"))
            .arg(recipe)
            .current_dir(&outside)
            .args(["", "a space", "'quoted'", "\u{feff}\u{85}\u{2003}"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        fs::read(workspace.path().join("cwd")).unwrap(),
        outside.as_os_str().as_encoded_bytes()
    );
    // The recipes join the manifest onto the justfile directory with `/` on every host.
    let manifest = format!("{}/Cargo.toml", workspace.path().display());
    let log = workspace.log();
    for recipe in ["run-source", "run-source-windows"] {
        assert!(log.contains(&format!("cargo run|--quiet|--locked|--manifest-path|{manifest}|-p|maestro-tooling|--bin|development|--|{recipe}||a space|'quoted'|\u{feff}\u{85}\u{2003}")), "{log}");
    }
}

#[test]
fn maestro_offline_recipe_is_explicit_only() {
    let workspace = Workspace::new();
    let home = workspace.path().join("home");
    fs::create_dir_all(home.join(".maestro/agent")).unwrap();
    let auth = home.join(".maestro/agent/auth.json");
    fs::write(&auth, "synthetic authentication").unwrap();
    for (recipe, expected) in [
        ("test-offline", "key=false auth=false flag=1\n"),
        ("test", "key=true auth=true flag=kept\n"),
        ("check", "key=true auth=true flag=kept\n"),
    ] {
        let output = workspace
            .command(recipe)
            .env("HOME", &home)
            .env("OPENAI_API_KEY", "controlled")
            .env("MAESTRO_NO_LOCAL_LLM", "kept")
            .env("MAESTRO_ENV_RESULT", workspace.path().join("env-result"))
            .env("MAESTRO_DEVELOPMENT", env!("CARGO_BIN_EXE_development"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(workspace.path().join("env-result")).unwrap(),
            expected
        );
        assert_eq!(fs::read(&auth).unwrap(), b"synthetic authentication");
    }
    let output = workspace
        .command("test")
        .env("HOME", &home)
        .env("OPENAI_API_KEY", "controlled")
        .env("MAESTRO_NO_LOCAL_LLM", "kept")
        .env("MAESTRO_ENV_RESULT", workspace.path().join("env-result"))
        .env("MAESTRO_FAIL", "nextest")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        fs::read_to_string(workspace.path().join("env-result")).unwrap(),
        "key=true auth=true flag=kept\n"
    );
    assert_eq!(fs::read(&auth).unwrap(), b"synthetic authentication");
    let log = workspace.log();
    assert_eq!(log.matches("--bin|development|--|test-offline").count(), 1);
    assert!(log.contains("cargo nextest|run|--workspace|--locked"));
    assert!(log.contains("cargo test|--workspace|--locked|--doc"));
    assert!(log.contains("cargo doc|--workspace|--no-deps|--locked"));
}

fn hooks(workspace: &Workspace) {
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.pre-commit-config.yaml"),
        workspace.path().join(".pre-commit-config.yaml"),
    )
    .unwrap();
    assert!(
        Command::new("git")
            .current_dir(workspace.path())
            .args(["init", "--quiet"])
            .status()
            .unwrap()
            .success()
    );
}

fn commit_message(workspace: &Workspace, message: &str) -> Output {
    fs::write(workspace.path().join("message"), message).unwrap();
    Command::new("prek")
        .current_dir(workspace.path())
        .args([
            "run",
            "--hook-stage",
            "commit-msg",
            "--commit-msg-filename",
            "message",
        ])
        .output()
        .unwrap()
}

#[test]
fn maestro_commit_subject_limit_counts_the_whole_line() {
    let workspace = Workspace::new();
    hooks(&workspace);
    for prefix in ["feat: ", "fix(core): ", "feat!: ", "fix(core)!: "] {
        for size in [71, 72] {
            let subject = format!("{prefix}{}\n", "a".repeat(size - prefix.len()));
            let output = commit_message(&workspace, &subject);
            assert_eq!(
                output.status.success(),
                size == 71,
                "{subject}: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
    }
    for size in [80, 81] {
        assert_eq!(
            commit_message(&workspace, &format!("fix: valid\n\n{}\n", "a".repeat(size)))
                .status
                .success(),
            size == 80
        );
    }
    for invalid in [
        "feat: Capitalized\n",
        "invalid: subject\n",
        "feat subject\n",
    ] {
        assert!(!commit_message(&workspace, invalid).status.success());
    }
}

#[test]
fn maestro_setup_keeps_pins_and_existing_hooks() {
    let workspace = Workspace::new();
    assert!(workspace.run("setup").status.success());
    assert_eq!(workspace.log(), "mise install\nmise exec|--|prek|install\n");
    hooks(&workspace);
    assert!(
        Command::new("prek")
            .current_dir(workspace.path())
            .arg("install")
            .status()
            .unwrap()
            .success()
    );
    for stage in ["pre-commit", "pre-merge-commit", "commit-msg"] {
        assert!(workspace.path().join(".git/hooks").join(stage).is_file());
    }
    let prek = format!("bin/prek{}", std::env::consts::EXE_SUFFIX);
    fs::remove_file(workspace.path().join(prek)).unwrap();
    let output = workspace
        .command("--command")
        .args(["prek", "run", "workspace-check"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for (name, content, hook) in [
        ("bad.yaml", "a: [broken\n", "check-yaml"),
        ("bad.toml", "[broken\n", "check-toml"),
        (
            "conflict.rs",
            "<<<<<<< HEAD\na\n=======\nb\n>>>>>>> branch\n",
            "check-merge-conflict",
        ),
    ] {
        fs::write(workspace.path().join(name), content).unwrap();
        let output = Command::new("prek")
            .current_dir(workspace.path())
            .args(["run", hook, "--files", name])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{hook}");
        fs::write(workspace.path().join(name), "").unwrap();
        assert!(
            Command::new("prek")
                .current_dir(workspace.path())
                .args(["run", hook, "--files", name])
                .status()
                .unwrap()
                .success()
        );
    }
    assert_pinned_tools();
}

fn assert_pinned_tools() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (tool, version) in [
        ("just", "1.58.0"),
        ("prek", "0.5.4"),
        ("watchexec", "2.8.0"),
        ("cargo-hakari", "0.9.39"),
    ] {
        let output = Command::new("mise")
            .current_dir(&root)
            .args(["exec", "--", tool, "--version"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains(version));
    }
}

#[test]
fn maestro_recipes_quote_apostrophes_in_checkout_paths() {
    let mut workspace = Workspace::new();
    let renamed = workspace.path().with_file_name(format!(
        "Maestro's-{}",
        workspace.path().file_name().unwrap().to_str().unwrap()
    ));
    workspace.rename(renamed.file_name().unwrap()).unwrap();
    let outside = workspace.path().join("outside");
    fs::create_dir(&outside).unwrap();
    for recipe in ["run-source", "run-source-windows", "test-offline"] {
        let output = workspace
            .command("--justfile")
            .arg(workspace.path().join("justfile"))
            .arg(recipe)
            .current_dir(&outside)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{recipe}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // The recipes join the manifest onto the justfile directory with `/` on every host.
    assert!(workspace.log().contains(&format!(
        "--manifest-path|{}/Cargo.toml",
        workspace.path().display()
    )));
}

#[test]
fn catalog_build_recipes_generate_once_before_compilation() {
    for recipe in [
        "models-generate",
        "models-build",
        "models-prepublish",
        "build",
        "prepublish",
        "build-binary",
    ] {
        let workspace = Workspace::new();
        assert!(workspace.run(recipe).status.success(), "{recipe}");
        let log = workspace.log();
        assert_eq!(
            log.matches("--example|generate_catalog_data").count(),
            1,
            "{recipe}: {log}"
        );
        if recipe != "models-generate" {
            let generation = log.find("--example|generate_catalog_data").unwrap();
            let compilation = log
                .find(if matches!(recipe, "build" | "prepublish") {
                    "cargo build|--workspace"
                } else {
                    "cargo build|-p|maestro-models"
                })
                .unwrap();
            assert!(generation < compilation, "{recipe}: {log}");
        }
        fs::remove_file(workspace.path().join("log")).unwrap();
        let result = workspace
            .command(recipe)
            .env("MAESTRO_FAIL", "run")
            .output()
            .unwrap();
        assert!(!result.status.success(), "{recipe}");
        let log = workspace.log();
        assert!(!log.contains("cargo build|--workspace"));
        assert!(!log.contains("cargo build|-p|maestro-models"));
    }
}

#[test]
fn catalog_cargo_tests_and_watch_routes_stay_offline() {
    let workspace = Workspace::new();
    for recipe in [
        "check",
        "test",
        "models-test",
        "agent-test",
        "tui-test",
        "app-test",
        "tui-test-ansi",
        "models-dev",
        "models-dev-compile",
        "dev-compile",
        "dev",
        "agent-dev",
        "tui-dev",
        "app-dev",
    ] {
        let result = workspace.run(recipe);
        assert!(
            result.status.success(),
            "{recipe}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let log = workspace.log();
    assert!(!log.contains("generate_catalog_data"));
    assert!(!log.contains("models-generate"));
}

/// A killed test leaves child output on disk without losing channel delivery.
#[cfg(unix)]
#[test]
fn watch_child_transcript_survives_forced_kill() {
    use std::io::{Read, Write};
    if std::env::var_os("MAESTRO_CAPTURE_CONTROL").is_some() {
        let (reader, mut writer) = std::os::unix::net::UnixStream::pair().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let _reader = recipe_support::capture_lines(move || reader, send);
        writer
            .write_all(b"watch phase: compiling\nwatch phase: ready\n")
            .unwrap();
        assert_eq!(receive.recv().unwrap(), "watch phase: compiling");
        assert_eq!(receive.recv().unwrap(), "watch phase: ready");
        recipe_support::record("phase: blocked waiting for controller");
        // Separate readiness from libtest's unterminated test-name prefix.
        println!("\nREADY {}", std::process::id());
        std::io::stdout().flush().unwrap();
        let _ = std::io::stdin().read(&mut [0]).unwrap();
        panic!("controller must kill the blocked test");
    }
    assert_killed_transcript(None);
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/browser-author-diagnostics/override-{}",
        std::process::id()
    ));
    assert_killed_transcript(Some(&directory));
}

/// Exercise the configured transcript location in a fresh blocked test process.
#[cfg(unix)]
fn assert_killed_transcript(directory: Option<&Path>) {
    use std::io::{BufRead, Read};
    let process =
        maestro_test_process::TestProcess::current("watch_child_transcript_survives_forced_kill")
            .unwrap();
    let name = process.name().replace("::", "_");
    let process = process.env_remove("MAESTRO_TEST_DIAGNOSTICS_DIR");
    let process = if let Some(directory) = directory {
        process.env("MAESTRO_TEST_DIAGNOSTICS_DIR", directory)
    } else {
        process
    };
    let mut child = process
        .nocapture()
        .single_threaded()
        .env("MAESTRO_CAPTURE_CONTROL", "1")
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
    let pid = output
        .by_ref()
        .lines()
        .map(Result::unwrap)
        .find_map(|line| {
            assert!(
                !line.contains("READY ") || line.starts_with("READY "),
                "readiness marker must start its own line: {line:?}"
            );
            line.strip_prefix("READY ").map(str::to_owned)
        })
        .unwrap();
    assert!(
        Command::new("kill")
            .args(["-KILL", "--", &pid])
            .status()
            .unwrap()
            .success()
    );
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("watch phase: compiling\nwatch phase: ready\n"),
        "{stderr}"
    );
    let fallback =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/browser-author-diagnostics");
    let path = directory
        .unwrap_or(&fallback)
        .join(format!("{pid}-{name}.log"));
    let transcript =
        fs::read_to_string(&path).expect("killed test must leave a durable transcript");
    assert!(transcript.contains("watch phase: compiling\n"));
    assert!(transcript.contains("watch phase: ready\n"));
    assert!(transcript.contains("phase: blocked waiting for controller\n"));
}
