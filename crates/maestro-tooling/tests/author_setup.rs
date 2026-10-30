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
        "missing pair",
        "invalid variable",
        "corrupt cache",
        "missing cache",
        "escaping cache name",
    ] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path();
        fixture(root);
        prepare(root);
        let prepared = tamper(root, change);
        let output = export(root, &prepared, true);
        assert!(!output.status.success(), "accepted {change}");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains(prepared.to_str().unwrap()),
            "{diagnostic}"
        );
        assert!(!root.join("nextest-env").exists());
        assert!(!root.join("target/debug/examples").exists());
        assert!(!root.join("cache-home").exists());
        assert_eq!(fs::read_to_string(root.join("seen")).unwrap(), "unset\n");
        assert_eq!(fs::read_to_string(root.join("builds")).unwrap(), "build\n");
    }
}

/// Preparation refuses a bad request, a malformed environment entry and inputs that change
/// while it runs, and writes no manifest.
#[test]
fn prepare_rejects_bad_requests_and_unstable_inputs() {
    let malformed = "#!/bin/sh\necho invalid > \"$NEXTEST_ENV\"\n";
    let unstable = format!("{EXAMPLE}echo edited > crates/example/src/lib.rs\n");
    for (name, example, command, output, reason) in [
        ("checkout", EXAMPLE, "prepare", ".", "needs a path"),
        (
            "command",
            EXAMPLE,
            "other",
            "target/test-setup",
            "unknown test setup",
        ),
        (
            "entry",
            malformed,
            "prepare",
            "target/test-setup",
            "invalid component entry",
        ),
        (
            "inputs",
            &unstable,
            "prepare",
            "target/test-setup",
            "inputs changed",
        ),
        (
            "cache time",
            MISPLACED_CACHE_EXAMPLE,
            "prepare",
            "target/test-setup",
            "does not follow",
        ),
        (
            "two caches",
            SPLIT_CACHE_EXAMPLE,
            "prepare",
            "target/test-setup",
            "more than one",
        ),
    ] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path();
        fixture_with(root, example);
        let error = maestro_tooling::run(
            &[
                "prepare-test-setup".into(),
                command.into(),
                output.into(),
                root.join("target/test-prepare").into(),
            ],
            &root.join("cargo"),
            root,
        )
        .unwrap_err();
        assert!(error.to_string().contains(reason), "{name}: {error}");
        assert!(!root.join("target/test-setup/components.json").exists());
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

/// The bundle carries the code the setup example compiled, and a shard installs it for the
/// modification time its executables share, in whichever directory the user cache is found,
/// a relative `HOME` included, as the test-process owner finds it.
#[test]
fn archived_author_setup_installs_the_prepared_compiled_code() {
    for home in [None, Some("absolute"), Some("relative")] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path();
        fixture(root);
        prepare(root);
        let staged = root.join("target/test-setup/cache");
        assert_eq!(fs::read_to_string(staged.join("key")).unwrap(), "compiled");
        assert!(!staged.join("key.stats").exists());
        let mut command = export_command(root, &root.join("target/test-setup"), true);
        if let Some(kind) = home {
            let value = if kind == "absolute" {
                root.join("home")
            } else {
                PathBuf::from("home")
            };
            command.env_remove("XDG_CACHE_HOME").env("HOME", value);
        }
        assert_success(&command.output().unwrap());
        let cache = if home.is_some() {
            root.join("home/.cache")
        } else {
            root.join("cache-home")
        };
        let milliseconds =
            fs::metadata(root.join("target/debug/examples/prepare_author_component"))
                .unwrap()
                .modified()
                .unwrap()
                .duration_since(at(0))
                .unwrap()
                .as_millis();
        let installed = cache.join(format!("wasmtime/modules/wasmtime-fixture-{milliseconds}"));
        assert_eq!(
            fs::read_to_string(installed.join("key")).unwrap(),
            "compiled"
        );
        assert!(!installed.join("key.stats").exists());
    }
}

/// A host whose compiled code the preparation cannot collect still prepares and exports, and
/// installs no cache.
#[test]
fn archived_author_setup_without_compiled_code_installs_none() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture_with(root, PLAIN_EXAMPLE);
    prepare(root);
    assert!(!root.join("target/test-setup/cache").exists());
    assert_success(&export(root, &root.join("target/test-setup"), true));
    assert!(!root.join("cache-home").exists());
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
if [ -n "${XDG_CACHE_HOME-}" ]; then
    seconds=$(stat -c %Y "$0" 2>/dev/null || stat -f %m "$0")
    modules="$XDG_CACHE_HOME/wasmtime/modules/wasmtime-fixture-${seconds}000"
    mkdir -p "$modules"
    entry=compiled
    if [ -n "${MAESTRO_AUTHOR_COMPONENT-}" ]; then entry=recompiled; fi
    [ -e "$modules/key" ] || { printf %s "$entry" > "$modules/key"; printf stats > "$modules/key.stats"; }
fi
"#;

/// A setup example that compiles nothing into a user cache, as on a host whose cache the
/// preparation cannot move.
const PLAIN_EXAMPLE: &str = r#"#!/bin/sh
printf 'MAESTRO_AUTHOR_COMPONENT=%s\nMAESTRO_CONTEXT_COMPONENT=%s\n' \
    "$PWD/component.wasm" "$PWD/component.wasm" > "$NEXTEST_ENV"
"#;

/// A setup example whose compiled code sits in a namespace that does not follow its own time.
const MISPLACED_CACHE_EXAMPLE: &str = r#"#!/bin/sh
printf 'MAESTRO_AUTHOR_COMPONENT=%s\nMAESTRO_CONTEXT_COMPONENT=%s\n' \
    "$PWD/component.wasm" "$PWD/component.wasm" > "$NEXTEST_ENV"
mkdir -p "$XDG_CACHE_HOME/wasmtime/modules/wasmtime-fixture-1"
echo x > "$XDG_CACHE_HOME/wasmtime/modules/wasmtime-fixture-1/key"
"#;

/// A setup example whose compiled code sits in two namespaces.
const SPLIT_CACHE_EXAMPLE: &str = r#"#!/bin/sh
printf 'MAESTRO_AUTHOR_COMPONENT=%s\nMAESTRO_CONTEXT_COMPONENT=%s\n' \
    "$PWD/component.wasm" "$PWD/component.wasm" > "$NEXTEST_ENV"
seconds=$(stat -c %Y "$0" 2>/dev/null || stat -f %m "$0")
mkdir -p "$XDG_CACHE_HOME/wasmtime/modules/wasmtime-fixture-${seconds}000" \
    "$XDG_CACHE_HOME/wasmtime/modules/wasmtime-other-${seconds}000"
echo x > "$XDG_CACHE_HOME/wasmtime/modules/wasmtime-fixture-${seconds}000/key"
echo y > "$XDG_CACHE_HOME/wasmtime/modules/wasmtime-other-${seconds}000/key"
"#;

/// The fixture Cargo: it counts its builds and builds the setup example `@example@`, and a
/// `run` also starts that example.
const CARGO: &str = r#"#!/bin/sh
set -eu
echo build >> builds
mode=$1
while [ $# -gt 0 ]; do
    if [ "$1" = --target-dir ]; then target=$2; fi
    shift
done
example=$target/debug/examples/prepare_author_component
mkdir -p "${example%/*}"
cat > "$example" <<'EXAMPLE'
@example@
EXAMPLE
chmod +x "$example"
touch -t 202001010000.00 "$example"
if [ "$mode" = run ]; then "$example"; fi
"#;

/// Give the standalone command a controlled Cargo boundary and source checkout.
fn fixture(root: &Path) {
    fixture_with(root, EXAMPLE);
}

/// Like `fixture`, with the setup example script the fixture Cargo builds.
fn fixture_with(root: &Path, example: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(root.join("crates/example/src")).unwrap();
    fs::write(root.join("crates/example/src/lib.rs"), "original source").unwrap();
    fs::write(root.join("Cargo.lock"), "locked dependencies").unwrap();
    fs::write(root.join("component.wasm"), "component bytes").unwrap();
    executable(&root.join("target/debug/deps/older"), 100);
    executable(&root.join("target/debug/deps/newer"), 200);
    fs::write(root.join("cargo"), CARGO.replace("@example@", example)).unwrap();
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
    export_command(root, prepared, archived).output().unwrap()
}

/// The command `export` runs, with the user cache inside the fixture.
fn export_command(root: &Path, prepared: &Path, archived: bool) -> Command {
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
        .env("XDG_CACHE_HOME", root.join("cache-home"));
    command
}

/// Damage the prepared bundle or the checkout as `change` names, and return the bundle's
/// directory afterwards.
fn tamper(root: &Path, change: &str) -> PathBuf {
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
        "corrupt cache" => fs::write(prepared.join("cache/key"), "broken").unwrap(),
        "missing cache" => fs::remove_file(prepared.join("cache/key")).unwrap(),
        "escaping cache name" => {
            // The file the name reaches carries the right digest, so only the guard refuses it.
            fs::copy(prepared.join("cache/key"), prepared.join("key")).unwrap();
            edit_manifest_value(&prepared, |manifest| {
                let files = manifest["compiled"]["files"].as_object_mut().unwrap();
                let digest = files.shift_remove("key").unwrap();
                files.insert("../key".to_owned(), digest);
            });
        }
        "missing pair" => edit_manifest(&prepared, |components| {
            components.retain(|variable, _| variable != "MAESTRO_CONTEXT_COMPONENT");
        }),
        "invalid variable" => {
            // The renamed component keeps its file and digest, so only the name is wrong.
            let renamed = "MAESTRO_lower_COMPONENT";
            fs::rename(
                prepared.join("MAESTRO_COMPONENT.wasm"),
                prepared.join(format!("{renamed}.wasm")),
            )
            .unwrap();
            edit_manifest(&prepared, |components| {
                let digest = components["MAESTRO_COMPONENT"].clone();
                components.retain(|variable, _| variable != "MAESTRO_COMPONENT");
                components.insert(renamed.to_owned(), digest);
            });
        }
        _ => unreachable!(),
    }
    prepared
}

/// Rewrite the component digests in the bundle's manifest.
fn edit_manifest(
    prepared: &Path,
    edit: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>),
) {
    edit_manifest_value(prepared, |manifest| {
        edit(manifest["components"].as_object_mut().unwrap());
    });
}

/// Rewrite the bundle's manifest as a whole.
fn edit_manifest_value(prepared: &Path, edit: impl FnOnce(&mut serde_json::Value)) {
    let path = prepared.join("components.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    edit(&mut manifest);
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
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
