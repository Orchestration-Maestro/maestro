//! Standalone component preparation and archived nextest setup.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use maestro_test_directory::Scratch;

/// Reusable outputs survive a move to another directory and never start a shard build.
#[test]
fn archived_author_setup_reuses_relocated_components() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    prepare(root);
    assert_eq!(fs::read_to_string(root.join("builds")).unwrap(), "build\n");
    let moved = root.join("relocated setup");
    fs::rename(root.join("target/test-setup"), &moved).unwrap();
    fs::remove_file(root.join("component.wasm")).unwrap();
    fs::remove_dir_all(root.join("target/test-prepare")).unwrap();
    for archived in [false, true] {
        assert_success(&export(root, &moved, archived));
    }
    let author = moved.join("MAESTRO_AUTHOR_COMPONENT.wasm");
    assert_eq!(
        fs::read_to_string(root.join("seen")).unwrap(),
        format!("unset\n{0}\n{0}\n", author.display())
    );
    let environment = fs::read_to_string(root.join("nextest-env")).unwrap();
    for variable in [
        "MAESTRO_AUTHOR_COMPONENT",
        "MAESTRO_CONTEXT_COMPONENT",
        "MAESTRO_COMPONENT",
    ] {
        let path = environment
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{variable}=")))
            .unwrap();
        assert!(Path::new(path).starts_with(&moved));
        assert_eq!(fs::read(path).unwrap(), b"component bytes");
    }
    assert_eq!(fs::read_to_string(root.join("builds")).unwrap(), "build\n");
}

/// Missing, edited or incomplete reusable outputs, including the staged setup example, fail
/// without starting Cargo.
#[test]
fn archived_author_setup_rejects_missing_and_stale_outputs() {
    for change in [
        "directory",
        "component",
        "example",
        "edited example",
        "source",
        "added",
        "removed",
        "lock",
        "corrupt",
        "multiline directory",
    ] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path();
        fixture(root);
        prepare(root);
        let mut prepared = root.join("target/test-setup");
        match change {
            "directory" => fs::rename(&prepared, root.join("unused")).unwrap(),
            "component" => fs::remove_file(prepared.join("MAESTRO_AUTHOR_COMPONENT.wasm")).unwrap(),
            "example" => fs::remove_file(prepared.join("prepare_author_component")).unwrap(),
            "edited example" => {
                fs::write(prepared.join("prepare_author_component"), "#!/bin/sh\n").unwrap();
            }
            "source" => fs::write(root.join("crates/example/src/lib.rs"), "edited source").unwrap(),
            "added" => fs::write(root.join("crates/example/src/new.rs"), "new source").unwrap(),
            "removed" => fs::remove_file(root.join("crates/example/src/lib.rs")).unwrap(),
            "lock" => fs::write(root.join("Cargo.lock"), "new dependencies").unwrap(),
            "corrupt" => {
                fs::write(prepared.join("MAESTRO_AUTHOR_COMPONENT.wasm"), "broken").unwrap();
            }
            "multiline directory" => {
                let moved = root.join("new\nline");
                fs::rename(&prepared, &moved).unwrap();
                prepared = moved;
            }
            _ => unreachable!(),
        }
        let output = export(root, &prepared, true);
        assert!(!output.status.success(), "accepted {change}");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains(prepared.to_str().unwrap()),
            "{diagnostic}"
        );
        assert!(!root.join("nextest-env").exists());
        assert!(!root.join("target/debug/examples").exists());
        assert_eq!(fs::read_to_string(root.join("seen")).unwrap(), "unset\n");
        assert_eq!(fs::read_to_string(root.join("builds")).unwrap(), "build\n");
    }
}

/// The bundle carries the command that prepared it, so a shard needs neither Cargo nor
/// maestro-tooling in its test archive to validate the bundle.
#[test]
fn prepared_setup_stages_the_preparing_command() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    assert_eq!(run_prepare(root).unwrap(), ExitCode::SUCCESS);
    assert_eq!(
        fs::read(root.join("target/test-setup/development")).unwrap(),
        fs::read(std::env::current_exe().unwrap()).unwrap()
    );
}

/// Archived runs give every extracted executable, and the installed setup example, one
/// modification time so test processes share a compile-cache namespace with the warm-up.
#[test]
fn archived_author_setup_aligns_executable_times() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    prepare(root);
    assert_success(&export(root, &root.join("target/test-setup"), true));
    let modified = |name: &str| {
        fs::metadata(root.join("target/debug").join(name))
            .unwrap()
            .modified()
            .unwrap()
    };
    let example = modified("examples/prepare_author_component");
    assert!(example > at(200), "{example:?}");
    for name in ["deps/older", "deps/newer"] {
        assert_eq!(modified(name), example, "{name}");
    }
}

/// Local and warm runs without a prepared bundle keep automatic preparation.
#[test]
fn author_setup_without_prepared_outputs_builds_locally() {
    for mode in ["local", "warm"] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path();
        fixture(root);
        let profile = root.join("target/debug");
        fs::create_dir_all(profile.join("deps")).unwrap();
        let mut command = Command::new("sh");
        command
            .arg(script("prepare-author-component.sh"))
            .current_dir(root)
            .env("CARGO", root.join("cargo"))
            .env("NEXTEST_ENV", root.join("nextest-env"))
            .env("NEXTEST_LD_LIBRARY_PATH", &profile)
            .env("NEXTEST_DYLD_FALLBACK_LIBRARY_PATH", &profile)
            .env_remove("TEST_ARCHIVE")
            .env_remove("TEST_PREPARE_OUTPUT");
        if mode == "warm" {
            command.env("TEST_PREPARE_OUTPUT", root.join("target/test-setup"));
        }
        assert_success(&command.output().unwrap());
        assert_eq!(fs::read_to_string(root.join("builds")).unwrap(), "build\n");
        assert!(root.join("nextest-env").is_file());
        assert!(!root.join("target/test-setup").exists());
    }
}

/// The fixture setup example: it records the author component it was given and writes
/// nextest's environment file from the components it was given, or from the checkout. It
/// also exports `MAESTRO_COMPONENT`, a name source discovery accepts.
const EXAMPLE: &str = r#"#!/bin/sh
echo "${MAESTRO_AUTHOR_COMPONENT-unset}" >> "$PWD/seen"
printf 'MAESTRO_AUTHOR_COMPONENT=%s\nMAESTRO_CONTEXT_COMPONENT=%s\nMAESTRO_COMPONENT=%s\n' \
    "${MAESTRO_AUTHOR_COMPONENT:-$PWD/component.wasm}" \
    "${MAESTRO_CONTEXT_COMPONENT:-$PWD/component.wasm}" \
    "${MAESTRO_COMPONENT:-$PWD/component.wasm}" > "$NEXTEST_ENV"
"#;

/// Give the standalone command a controlled Cargo boundary and source checkout.
fn fixture(root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    fs::write(root.join("crates/example/src/lib.rs"), "original source").unwrap();
    fs::write(root.join("Cargo.lock"), "locked dependencies").unwrap();
    fs::write(root.join("component.wasm"), "component bytes").unwrap();
    executable(&root.join("target/debug/deps/older"), 100);
    executable(&root.join("target/debug/deps/newer"), 200);
    fs::write(
        root.join("cargo"),
        format!(
            "#!/bin/sh\nset -eu\necho build >> builds\nmode=$1\n\
             while [ $# -gt 0 ]; do\n    if [ \"$1\" = --target-dir ]; then target=$2; fi\n    shift\ndone\n\
             example=$target/debug/examples/prepare_author_component\nmkdir -p \"${{example%/*}}\"\n\
             cat > \"$example\" <<'EXAMPLE'\n{EXAMPLE}EXAMPLE\nchmod +x \"$example\"\n\
             if [ \"$mode\" = run ]; then \"$example\"; fi\n"
        ),
    )
    .unwrap();
    fs::set_permissions(root.join("cargo"), fs::Permissions::from_mode(0o755)).unwrap();
    for args in [["init", "-q"].as_slice(), ["add", "."].as_slice()] {
        assert_success(
            &Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .unwrap(),
        );
    }
}

/// Write an executable test artifact with a fixed modification time.
fn executable(path: &Path, seconds: u64) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    fs::File::open(path)
        .unwrap()
        .set_modified(at(seconds))
        .unwrap();
}

/// Seconds after the epoch, as the timestamps these fixtures assign.
fn at(seconds: u64) -> std::time::SystemTime {
    std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds)
}

/// Invoke the preparation command with a controlled Cargo, as the preparation job does.
fn run_prepare(root: &Path) -> std::io::Result<ExitCode> {
    maestro_tooling::run(
        &[
            "prepare-test-setup".into(),
            "prepare".into(),
            root.join("target/test-setup").into(),
            root.join("target/test-prepare").into(),
        ],
        &root.join("cargo"),
        root,
    )
}

/// Prepare the bundle, then give it the development binary as its exporter: this test
/// executable cannot run the export command that the shard runs from the bundle.
fn prepare(root: &Path) {
    assert_eq!(run_prepare(root).unwrap(), ExitCode::SUCCESS);
    fs::copy(
        env!("CARGO_BIN_EXE_development"),
        root.join("target/test-setup/development"),
    )
    .unwrap();
}

/// Run the real nextest setup entry point with archived-shard environment. The development
/// binary takes its checkout from `CARGO_MANIFEST_DIR`, which points into the fixture.
fn export(root: &Path, prepared: &Path, archived: bool) -> Output {
    let mut command = Command::new("sh");
    if archived {
        command.env("TEST_ARCHIVE", "1");
    } else {
        command.env_remove("TEST_ARCHIVE");
    }
    command
        .arg(script("prepare-author-component.sh"))
        .current_dir(root)
        .env("CARGO", root.join("cargo"))
        .env("CARGO_MANIFEST_DIR", root.join("crates/example"))
        .env("TEST_PREPARE_OUTPUT", prepared)
        .env("NEXTEST_ENV", root.join("nextest-env"))
        .env("NEXTEST_LD_LIBRARY_PATH", root.join("target/debug"))
        .env(
            "NEXTEST_DYLD_FALLBACK_LIBRARY_PATH",
            root.join("target/debug"),
        )
        .output()
        .unwrap()
}

/// Locate a repository setup script without embedding its contents.
fn script(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.config")
        .join(name)
}

/// Display both output streams when a fixture command fails.
fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
