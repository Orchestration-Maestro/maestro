//! The shared browser build runs once per nextest run, before the tests that reuse it.
#![cfg(unix)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use maestro_test_directory::Scratch;
use maestro_test_process::{TestProcess, selection};
use maestro_tooling as tooling;

use crate::{
    browser_author_build::browser_support, recipe_support::install_executable, toolchain_support,
};

/// Tests the fixture crate records, as nextest names them inside its `maestro-tooling` package.
const BROWSER_TESTS: [&str; 2] = ["first", "second"];

/// The test that builds from empty state and so reuses nothing the setup prepares.
const COLD_TEST: &str = "browser_author_bootstraps_from_empty_build_state";

/// A shard prepares the browser build exactly when it runs a test that reuses it.
#[test]
fn browser_build_is_prepared_once_where_reusing_tests_run() {
    let root = Scratch::new().unwrap();
    write_fixture(root.path());
    let configured = root.path().join("configured.toml");
    fs::write(&configured, recording_config()).unwrap();
    let mut prepared = 0;
    let mut cold = 0;
    for shard in 1..=16 {
        let events = run_shard(root.path(), &configured, shard);
        let reuses = events
            .iter()
            .any(|event| BROWSER_TESTS.contains(&event.as_str()));
        let builds = events.iter().filter(|event| *event == "setup").count();
        assert_eq!(builds, usize::from(reuses), "shard {shard}: {events:?}");
        if reuses {
            assert_eq!(events[0], "setup", "shard {shard}: {events:?}");
            prepared += 1;
        }
        cold += usize::from(events.iter().any(|event| event == COLD_TEST));
    }
    assert!(prepared > 0, "no shard ran a test that reuses the build");
    assert_eq!(cold, 1, "the cold-start test must run once, unprepared");
}

/// Write a package named like the tooling crate whose tests only record that they ran.
fn write_fixture(root: &Path) {
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"maestro-tooling\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n",
    )
    .unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        format!(
            "#[cfg(test)]\nfn record(event: &str) {{\n\
             use std::io::Write;\n\
             let path = concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/events.log\");\n\
             let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();\n\
             writeln!(file, \"{{event}}\").unwrap();\n}}\n\
             #[cfg(test)]\nmod browser_author_build {{\n\
             #[test] fn first() {{ super::record(\"first\"); }}\n\
             #[test] fn second() {{ super::record(\"second\"); }}\n\
             #[test] fn {COLD_TEST}() {{ super::record(\"{COLD_TEST}\"); }}\n}}\n\
             #[cfg(test)]\nmod elsewhere {{\n\
             #[test] fn unrelated() {{ super::record(\"unrelated\"); }}\n}}\n"
        ),
    )
    .unwrap();
}

/// The repository's browser setup wiring, with the build replaced by a recorded `setup` event.
fn recording_config() -> String {
    let mut config: toml::Table =
        toml::from_str(include_str!("../../../.config/nextest.toml")).unwrap();
    let mut script = config["scripts"]["setup"]
        .as_table_mut()
        .unwrap()
        .remove("browser-author")
        .expect("the configuration defines a browser-author setup script")
        .try_into::<toml::Table>()
        .unwrap();
    script.insert(
        "command".into(),
        ["sh", "-c", "echo setup >> events.log"]
            .map(str::to_owned)
            .to_vec()
            .into(),
    );
    let setup = toml::Table::from_iter([("browser-author".to_owned(), script.into())]);
    config.insert(
        "scripts".into(),
        toml::Table::from_iter([("setup".to_owned(), setup.into())]).into(),
    );
    let profile = config["profile"]["default"].as_table_mut().unwrap();
    let rules = profile["scripts"].as_array_mut().unwrap();
    rules.retain(|rule| rule["setup"].as_str() == Some("browser-author"));
    assert_eq!(rules.len(), 1, "one rule selects the browser setup");
    profile.remove("overrides");
    toml::to_string(&config).unwrap()
}

/// Run one partition and return the events its processes recorded, in order.
fn run_shard(root: &Path, config: &Path, shard: u32) -> Vec<String> {
    let log = root.join("events.log");
    let _ = fs::remove_file(&log);
    let output = toolchain_support::fixture_command("cargo", root, None)
        .unwrap()
        .args([
            "nextest",
            "run",
            "--offline",
            "--color",
            "never",
            "--no-tests",
            "pass",
            "--config-file",
        ])
        .arg(config)
        .args(["--partition", &format!("count:{shard}/16")])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Stand-in for `just` and `cargo` that records its arguments and the variables that decide
/// Cargo's output, then fails the build recipe to show that one failure does not end the setup.
const RECORDING_TOOL: &str = "#!/bin/sh\n\
    { echo \"call $*\"; env | grep -E '^(CARGO_TARGET_DIR|RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|LLVM_PROFILE_FILE|CARGO_LLVM_COV|__CARGO_LLVM_COV|RUSTC_WRAPPER)' | sort; } >> \"$CALLS\"\n\
    [ \"$1\" != browser-library-build ]\n";

/// The library lint `browser-check` runs after repairing formatting.
const LIBRARY_LINT: &str =
    "clippy --no-deps -p maestro-web --target wasm32-unknown-unknown --locked -- -D warnings";

/// Each build runs in one dedicated target directory with the runner's instrumentation
/// removed, and the tests are told where that directory is.
#[test]
fn browser_setup_builds_in_its_own_target_without_instrumentation() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    let target = root.join("target/browser-author");
    install_stubs(root);
    for wrapper in [Some("sccache"), None] {
        let calls = run_setup(root, wrapper);
        let mut seen = Vec::new();
        for call in calls.split("call ").skip(1) {
            let (arguments, environment) = call.split_once('\n').unwrap();
            seen.push(arguments);
            let mut expected = vec![format!("CARGO_TARGET_DIR={}", target.display())];
            expected.extend(wrapper.map(|wrapper| format!("RUSTC_WRAPPER={wrapper}")));
            assert_eq!(
                environment.lines().collect::<Vec<_>>(),
                expected,
                "{arguments}"
            );
        }
        seen.sort_unstable();
        assert_eq!(
            seen,
            [
                "browser-debug-build",
                "browser-example-check",
                "browser-library-build",
                LIBRARY_LINT
            ]
        );
        assert_eq!(
            fs::read_to_string(root.join("nextest-env")).unwrap(),
            format!("MAESTRO_BROWSER_TARGET_DIR={}\n", target.display())
        );
    }
}

/// The setup lints the library exactly as `browser-check` does, so it warms what that recipe uses.
#[test]
fn browser_setup_lints_the_library_as_browser_check_does() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    install_stubs(root);
    let calls = root.join("calls.log");
    let _ = tooling::run(
        &[
            "browser-author".into(),
            "check".into(),
            "brand.json".into(),
            "dark".into(),
        ],
        &root.join("bin/cargo"),
        root,
    );
    let recipe_lints: Vec<_> = fs::read_to_string(&calls)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("call "))
        .filter(|arguments| arguments.starts_with("clippy"))
        .map(str::to_owned)
        .collect();
    assert_eq!(recipe_lints, [LIBRARY_LINT]);
}

/// Put recording `just` and `cargo` in `root/bin`; both append to `root/calls.log`.
fn install_stubs(root: &Path) {
    fs::create_dir(root.join("bin")).unwrap();
    let log = format!("\"{}\"", root.join("calls.log").display());
    fs::write(
        root.join("stub.sh"),
        RECORDING_TOOL.replace("\"$CALLS\"", &log),
    )
    .unwrap();
    for name in ["just", "cargo"] {
        install_executable(&root.join("stub.sh"), &root.join("bin").join(name));
    }
}

/// Run the setup under coverage instrumentation, where `wrapper` is the compiler wrapper that
/// preceded coverage, and return what the recorded calls saw.
fn run_setup(root: &Path, wrapper: Option<&str>) -> String {
    let calls = root.join("calls.log");
    let _ = fs::remove_file(&calls);
    let _ = fs::remove_file(root.join("nextest-env"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_development"));
    command
        .args(["nextest-setup", "browser-author"])
        .current_dir(root)
        .env_clear()
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("bin").display()),
        )
        .env("NEXTEST_ENV", root.join("nextest-env"))
        .env("RUSTFLAGS", "-C instrument-coverage")
        .env("CARGO_ENCODED_RUSTFLAGS", "-Cinstrument-coverage")
        .env("LLVM_PROFILE_FILE", root.join("profile-%p.profraw"))
        .env("CARGO_LLVM_COV", "1")
        .env(
            "CARGO_LLVM_COV_TARGET_DIR",
            root.join("target/llvm-cov-target"),
        )
        .env("RUSTC_WRAPPER", "cargo-llvm-cov")
        .env("__CARGO_LLVM_COV_RUSTC_WRAPPER", "1");
    if let Some(wrapper) = wrapper {
        command.env("__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING", wrapper);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(calls).unwrap()
}

/// Browser builds use the target directory the setup announces, and the test build's own without it.
#[test]
fn browser_builds_follow_the_announced_target_directory() {
    let selection = selection();
    if !selection.is_child() {
        for case in ["announced", "unannounced"] {
            let process =
                TestProcess::current("browser_builds_follow_the_announced_target_directory")
                    .unwrap()
                    .case(case)
                    .nocapture();
            let process = match case {
                "announced" => process.env("MAESTRO_BROWSER_TARGET_DIR", "/announced/target"),
                _ => process.env_remove("MAESTRO_BROWSER_TARGET_DIR"),
            };
            let output = process.output().unwrap();
            assert!(
                output.status.success(),
                "{case}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }
    let expected = if selection.runs_case("announced") {
        PathBuf::from("/announced/target")
    } else {
        Path::new(env!("CARGO_BIN_EXE_development"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    };
    assert_eq!(browser_support::target_directory(), expected);
}
