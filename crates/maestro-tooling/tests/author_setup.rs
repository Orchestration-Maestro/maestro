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

/// A change to a crate that nothing the preparation builds depends on leaves the bundle valid.
#[test]
fn archived_author_setup_ignores_crates_outside_the_built_closure() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    prepare(root);
    fs::write(root.join("crates/unrelated/src/lib.rs"), "edited source").unwrap();
    fs::write(root.join("crates/unrelated/src/new.rs"), "new source").unwrap();
    assert_success(&export(root, &root.join("target/test-setup"), true));
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
        "dependency",
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
    let unstable = format!("{EXAMPLE}echo edited > crates/maestro-extensions-wasm/src/lib.rs\n");
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

/// A run without a bundle installs this command alone, where the nextest setup entry looks for
/// it, and the run then prepares its components locally.
#[test]
fn install_stages_only_the_preparing_command() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);

    let code = maestro_tooling::run(
        &[
            "prepare-test-setup".into(),
            "install".into(),
            "target/test-setup".into(),
        ],
        &root.join("cargo"),
        root,
    )
    .unwrap();

    assert_eq!(code, ExitCode::SUCCESS);
    let staged: Vec<_> = fs::read_dir(root.join("target/test-setup"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(staged, ["development"]);
    assert_eq!(
        fs::read(root.join("target/test-setup/development")).unwrap(),
        fs::read(std::env::current_exe().unwrap()).unwrap()
    );
}

/// The recipes the browser graph is built with, as one `justfile` stands for them. The first
/// builds the graph and what Cargo regenerates beside it, or records that it found the graph
/// installed; the others do nothing.
const BROWSER_RECIPES: &str = "browser-example-check:\n\
    \tmkdir -p \"$CARGO_TARGET_DIR/debug/incremental\"\n\
    \tif [ -f \"$CARGO_TARGET_DIR/debug/graph\" ]; then echo installed >> \"$PWD/recipes.log\"; \
    else echo built >> \"$PWD/recipes.log\"; echo graph > \"$CARGO_TARGET_DIR/debug/graph\"; \
    echo regenerated > \"$CARGO_TARGET_DIR/debug/incremental/state\"; fi\n\
    browser-library-build:\n\ttrue\n\
    browser-debug-build:\n\ttrue\n";

/// The recipes with a first one that leaves a partial graph and fails.
const FAILING_BROWSER_RECIPES: &str = "browser-example-check:\n\
    \tmkdir -p \"$CARGO_TARGET_DIR/debug\"\n\
    \techo partial > \"$CARGO_TARGET_DIR/debug/graph\"\n\
    \tfalse\n\
    browser-library-build:\n\ttrue\n\
    browser-debug-build:\n\ttrue\n";

/// The bundle records the browser graph, stages it without what Cargo regenerates itself and
/// carries it beside the components.
#[test]
fn prepared_bundle_carries_the_browser_graph() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    fs::write(root.join("justfile"), BROWSER_RECIPES).unwrap();
    prepare(root);
    let staged = root.join("target/test-setup/browser-author");
    assert_eq!(
        fs::read_to_string(staged.join("debug/graph")).unwrap(),
        "graph\n"
    );
    assert!(!staged.join("debug/incremental").exists());
    assert_eq!(bundle_manifest(root)["browser"], true);
}

/// A graph whose build failed is not staged, so a shard builds it itself instead of installing
/// a partial one.
#[test]
fn prepared_bundle_leaves_out_a_failed_browser_graph() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    fs::write(root.join("justfile"), FAILING_BROWSER_RECIPES).unwrap();
    prepare(root);
    assert!(!root.join("target/test-setup/browser-author").exists());
    assert_eq!(bundle_manifest(root)["browser"], false);
}

/// A shard installs the bundle's graph before the recipes run, under one modification time that
/// is not older than any file of its checkout, so the recipes find it.
#[test]
fn browser_setup_installs_the_prepared_graph_before_the_recipes() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    fs::write(root.join("justfile"), BROWSER_RECIPES).unwrap();
    prepare(root);
    fs::remove_dir_all(root.join("target/browser-author")).unwrap();
    let source = root.join("crates/maestro-web/src/lib.rs");
    fs::File::open(&source)
        .unwrap()
        .set_modified(std::time::SystemTime::now())
        .unwrap();

    assert_success(&browser_command(root).output().unwrap());

    assert_eq!(
        fs::read_to_string(root.join("recipes.log")).unwrap(),
        "built\ninstalled\n"
    );
    let source_time = fs::metadata(&source).unwrap().modified().unwrap();
    let times = tree_times(&root.join("target/browser-author"));
    assert!(!times.is_empty());
    assert!(
        times
            .iter()
            .all(|time| *time == times[0] && *time >= source_time)
    );
    assert_eq!(
        fs::read_to_string(root.join("nextest-env")).unwrap(),
        format!(
            "MAESTRO_BROWSER_TARGET_DIR={}\n",
            root.join("target/browser-author").display()
        )
    );
}

/// A bundle prepared from other sources than the checkout holds installs nothing.
#[test]
fn browser_setup_refuses_a_graph_prepared_from_other_sources() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    fs::write(root.join("justfile"), BROWSER_RECIPES).unwrap();
    prepare(root);
    fs::remove_dir_all(root.join("target/browser-author")).unwrap();
    fs::write(root.join("crates/maestro-web/src/lib.rs"), "edited source").unwrap();

    let output = browser_command(root).output().unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("stale prepared"));
    assert!(!root.join("target/browser-author").exists());
    assert_eq!(
        fs::read_to_string(root.join("recipes.log")).unwrap(),
        "built\n"
    );
}

/// A bundle without a graph leaves the shard to build it, as a run without a bundle does.
#[test]
fn browser_setup_builds_when_the_bundle_holds_no_graph() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    prepare(root);
    fs::write(root.join("justfile"), BROWSER_RECIPES).unwrap();

    assert_success(&browser_command(root).output().unwrap());

    assert_eq!(
        fs::read_to_string(root.join("recipes.log")).unwrap(),
        "built\n"
    );
}

/// Archived runs give every extracted executable, and the installed setup example, one
/// modification time so test processes share a compile-cache namespace with the installed
/// compiled code.
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

/// How a shard's `HOME` is set: left to the fixture's `XDG_CACHE_HOME`, absolute or relative.
#[derive(Clone, Copy)]
enum Home {
    Unset,
    Absolute,
    Relative,
}

/// The bundle carries the code the setup example compiled, and a shard installs it for the
/// modification time its executables share, in whichever directory the user cache is found,
/// a relative `HOME` included, as the test-process owner finds it.
#[test]
fn archived_author_setup_installs_the_prepared_compiled_code() {
    for home in [Home::Unset, Home::Absolute, Home::Relative] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path();
        fixture(root);
        prepare(root);
        let staged = root.join("target/test-setup/cache");
        assert_eq!(fs::read_to_string(staged.join("key")).unwrap(), "compiled");
        assert!(!staged.join("key.stats").exists());
        let mut command = export_command(root, &root.join("target/test-setup"), true);
        let value = match home {
            Home::Unset => None,
            Home::Absolute => Some(root.join("home")),
            Home::Relative => Some(PathBuf::from("home")),
        };
        if let Some(value) = &value {
            command.env_remove("XDG_CACHE_HOME").env("HOME", value);
        }
        assert_success(&command.output().unwrap());
        let cache = if value.is_some() {
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

/// An archived shard that is given no output directory reads the bundle at `target/test-setup`.
#[test]
fn archived_author_setup_defaults_to_the_target_bundle() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    fixture(root);
    prepare(root);
    let mut command = export_command(root, &root.join("target/test-setup"), true);
    command.env_remove("TEST_PREPARE_OUTPUT");
    assert_success(&command.output().unwrap());
    let environment = fs::read_to_string(root.join("nextest-env")).unwrap();
    assert!(environment.contains(&format!(
        "MAESTRO_AUTHOR_COMPONENT={}",
        root.join("target/test-setup/MAESTRO_AUTHOR_COMPONENT.wasm")
            .display()
    )));
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

/// Runs without an archive and without a prepared bundle build the setup example themselves,
/// whether or not `TEST_PREPARE_OUTPUT` names an output directory.
#[test]
fn author_setup_without_prepared_outputs_builds_locally() {
    for names_output in [false, true] {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path();
        fixture(root);
        let profile = root.join("target/debug");
        fs::create_dir_all(profile.join("deps")).unwrap();
        let mut command = setup_command();
        command
            .current_dir(root)
            .env("CARGO", root.join("cargo"))
            .env("NEXTEST_ENV", root.join("nextest-env"))
            .env("NEXTEST_LD_LIBRARY_PATH", &profile)
            .env("NEXTEST_DYLD_FALLBACK_LIBRARY_PATH", &profile)
            .env_remove("TEST_ARCHIVE")
            .env_remove("TEST_PREPARE_OUTPUT");
        if names_output {
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

/// The fixture Cargo: it counts its builds and builds the setup example `@example@` in the `dev`
/// profile, which it requires, and a `run` also starts that example.
const CARGO: &str = r#"#!/bin/sh
set -eu
if [ "$1" = metadata ]; then exec "@cargo@" "$@"; fi
if [ "$1" = clippy ]; then exit 0; fi
case " $* " in *" --profile dev "*) ;; *) echo "build without the dev profile: $*" >&2; exit 1 ;; esac
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
    workspace(root);
    fs::write(root.join("component.wasm"), "component bytes").unwrap();
    executable(&root.join("target/debug/deps/older"), 100);
    executable(&root.join("target/debug/deps/newer"), 200);
    fs::write(
        root.join("cargo"),
        CARGO
            .replace("@example@", example)
            .replace("@cargo@", env!("CARGO")),
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

/// Write the fixture workspace: the two packages the preparation builds, the crate one of them
/// depends on and a crate that nothing it builds depends on.
fn workspace(root: &Path) {
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"maestro-extensions-wasm\"\nversion = \"0.0.0\"\ndependencies = [\n \"shared\",\n]\n\n\
         [[package]]\nname = \"maestro-tooling\"\nversion = \"0.0.0\"\n\n\
         [[package]]\nname = \"maestro-web\"\nversion = \"0.0.0\"\n\n\
         [[package]]\nname = \"shared\"\nversion = \"0.0.0\"\n\n\
         [[package]]\nname = \"unrelated\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    for (name, dependency) in [
        (
            "maestro-extensions-wasm",
            "\n[dependencies]\nshared = { path = \"../shared\" }\n",
        ),
        ("maestro-tooling", ""),
        ("maestro-web", ""),
        ("shared", ""),
        ("unrelated", ""),
    ] {
        let crate_root = root.join("crates").join(name);
        fs::create_dir_all(crate_root.join("src")).unwrap();
        fs::write(crate_root.join("src/lib.rs"), "original source").unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n{dependency}"),
        )
        .unwrap();
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

/// Run the real nextest setup entry point with archived-shard environment, from the fixture
/// checkout where nextest starts it.
fn export(root: &Path, prepared: &Path, archived: bool) -> Output {
    export_command(root, prepared, archived).output().unwrap()
}

/// The command `export` runs, with the user cache inside the fixture.
fn export_command(root: &Path, prepared: &Path, archived: bool) -> Command {
    shard_command(setup_command(), root, prepared, archived)
}

/// `command` in the environment of a shard that runs from the fixture checkout.
fn shard_command(mut command: Command, root: &Path, prepared: &Path, archived: bool) -> Command {
    if archived {
        command.env("TEST_ARCHIVE", "1");
    } else {
        command.env_remove("TEST_ARCHIVE");
    }
    command
        .current_dir(root)
        .env("CARGO", root.join("cargo"))
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
        "source" => fs::write(
            root.join("crates/maestro-extensions-wasm/src/lib.rs"),
            "edited source",
        )
        .unwrap(),
        "dependency" => fs::write(root.join("crates/shared/src/lib.rs"), "edited source").unwrap(),
        "added" => fs::write(
            root.join("crates/maestro-extensions-wasm/src/new.rs"),
            "new source",
        )
        .unwrap(),
        "removed" => {
            fs::remove_file(root.join("crates/maestro-extensions-wasm/src/lib.rs")).unwrap();
        }
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

/// The bundle's manifest of the fixture checkout.
fn bundle_manifest(root: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(root.join("target/test-setup/components.json")).unwrap())
        .unwrap()
}

/// The modification time of every file under `directory`.
fn tree_times(directory: &Path) -> Vec<std::time::SystemTime> {
    let mut times = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            times.extend(tree_times(&path));
        } else {
            times.push(fs::metadata(&path).unwrap().modified().unwrap());
        }
    }
    times
}

/// The browser setup of an archived shard, run from the fixture checkout.
fn browser_command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_development"));
    command.args(["nextest-setup", "browser-author"]);
    shard_command(command, root, &root.join("target/test-setup"), true)
}

/// The author-component setup, as nextest's setup entry runs it.
fn setup_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_development"));
    command.args(["nextest-setup", "author-component"]);
    command
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
