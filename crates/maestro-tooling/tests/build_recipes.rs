#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "maestro-recipes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../justfile"),
            root.join("justfile"),
        )
        .unwrap();
        let source = root.join("tool.rs");
        fs::write(&source, include_str!("fixtures/recipe_command.rs")).unwrap();
        let tool = root.join("bin/cargo");
        assert!(
            Command::new("rustc")
                .arg(source)
                .arg("-o")
                .arg(&tool)
                .status()
                .unwrap()
                .success()
        );
        for name in ["mise", "prek", "watchexec"] {
            fs::copy(&tool, root.join("bin").join(name)).unwrap();
        }
        Self(root)
    }

    fn command(&self, recipe: &str) -> Command {
        let mut command = Command::new("just");
        command
            .current_dir(&self.0)
            .arg(recipe)
            .env("MAESTRO_LOG", self.0.join("log"))
            .env("MAESTRO_CWD", self.0.join("cwd"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.0.join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            );
        command
    }

    fn run(&self, recipe: &str) -> Output {
        self.command(recipe).output().unwrap()
    }
    fn log(&self) -> String {
        fs::read_to_string(self.0.join("log")).unwrap()
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

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
        "cargo clean\ncargo build|--workspace|--locked\ncargo fmt|--all\ncargo clippy|--workspace|--all-targets|--locked|--|-D|warnings\ncargo doc|--workspace|--no-deps|--locked\ncargo test|-p|maestro-test-conventions|--locked\n"
    );
    fs::remove_file(workspace.0.join("log")).unwrap();
    let result = workspace
        .command("prepublish")
        .env("MAESTRO_FAIL", "build")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(
        workspace.log(),
        "cargo clean\ncargo build|--workspace|--locked\n"
    );
    for phase in ["fmt", "clippy", "doc", "test"] {
        fs::remove_file(workspace.0.join("log")).unwrap();
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
        assert!(!log.contains("cargo test|--workspace"));
    }
    fs::remove_file(workspace.0.join("log")).unwrap();
    assert!(workspace.run("ci").status.success());
    assert!(
        workspace
            .log()
            .ends_with("cargo test|--workspace|--locked\n")
    );
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
        assert_eq!(workspace.log(), format!("cargo test|-p|{owner}|--locked\n"));
        assert_package_build(&workspace, prefix, owner);
    }
    assert_binary_build_order();
}

fn assert_package_build(workspace: &Workspace, prefix: &str, owner: &str) {
    for recipe in ["build", "prepublish", "clean"] {
        fs::remove_file(workspace.0.join("log")).unwrap();
        assert!(
            workspace
                .run(&format!("{prefix}-{recipe}"))
                .status
                .success()
        );
        let log = workspace.log();
        assert!(!log.contains("clippy"));
        if recipe == "build" {
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
        workspace.0.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    for owner in [
        "maestro-models",
        "maestro-agent",
        "maestro-tui",
        "maestro-app",
    ] {
        let root = workspace.0.join("crates").join(owner);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!("[package]\nname = '{owner}'\nversion = '0.1.0'\nedition = '2024'\n"),
        )
        .unwrap();
        fs::write(root.join("src/lib.rs"), "/// Adds one.\n/// ```\n/// assert_eq!(maestro_models::increment(1), 2);\n/// ```\npub fn increment(n: i32) -> i32 { n + 1 }\n#[test] fn selected_behavior() {}\n#[test] #[ignore] fn ignored_behavior() {}\n#[test] fn failing_behavior() { if std::env::var_os(\"MAESTRO_TEST_FAIL\").is_some() { panic!(\"controlled failure\"); } }\n".replace("maestro_models", &owner.replace('-', "_"))).unwrap();
    }
    fs::rename(
        workspace.0.join("bin/cargo"),
        workspace.0.join("bin/cargo-proxy"),
    )
    .unwrap();
    fs::copy(env!("CARGO"), workspace.0.join("bin/cargo")).unwrap();
    assert!(
        Command::new(env!("CARGO"))
            .current_dir(&workspace.0)
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
    fs::create_dir_all(workspace.0.join(".cargo")).unwrap();
    fs::write(
        workspace.0.join(".cargo/config.toml"),
        "[build]\ntarget-dir = 'compiler-output'\n",
    )
    .unwrap();
    assert!(workspace.run("build").status.success());
    let development = prepare_configured_assets(&workspace);
    assert!(
        workspace
            .0
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
            .0
            .join("compiler-output/debug/libmaestro_models.rlib")
            .exists()
    );
    assert!(
        workspace
            .0
            .join("compiler-output/debug/libmaestro_agent.rlib")
            .exists()
    );
    assert!(
        workspace
            .0
            .join("crates/maestro-models/src/lib.rs")
            .exists()
    );
    assert_asset_cleanup(&workspace, &development);
}

fn assert_asset_cleanup(workspace: &Workspace, development: &Path) {
    for tree in ["maestro-app-assets", "maestro-binary-assets"] {
        assert!(workspace.0.join("compiler-output").join(tree).is_dir());
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
            .0
            .join("compiler-output/debug/libmaestro_agent.rlib")
            .exists()
    );
    assert!(workspace.0.join("crates/maestro-app/src/lib.rs").exists());
    for tree in ["maestro-app-assets", "maestro-binary-assets"] {
        assert!(!workspace.0.join("compiler-output").join(tree).exists());
        fs::create_dir_all(workspace.0.join("compiler-output").join(tree)).unwrap();
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
        assert!(!workspace.0.join("compiler-output").join(tree).exists());
    }
    assert!(
        !workspace
            .0
            .join("compiler-output/debug/libmaestro_agent.rlib")
            .exists()
    );
}

fn prepare_configured_assets(workspace: &Workspace) -> PathBuf {
    fs::rename(
        workspace.0.join("bin/cargo-proxy"),
        workspace.0.join("bin/cargo"),
    )
    .unwrap();
    create_prepared_assets(workspace);
    let binary_root = workspace.0.join("crates/maestro");
    fs::create_dir_all(binary_root.join("src")).unwrap();
    fs::write(
        binary_root.join("Cargo.toml"),
        "[package]\nname = 'maestro'\nversion = '0.1.0'\nedition = '2024'\n",
    )
    .unwrap();
    fs::write(binary_root.join("src/main.rs"), "fn main() {}\n").unwrap();
    prepare_tooling_owner(workspace);
    let development = workspace.0.join("compiler-output/debug/development");
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
            workspace.0.join("compiler-output").join(tree).is_dir(),
            "missing configured asset tree: {tree}"
        );
    }
    development
}

fn create_prepared_assets(workspace: &Workspace) {
    let root = workspace.0.join("crates/maestro-app");
    for (name, value) in [
        ("src/modes/interactive/theme/theme.json", "{}"),
        ("src/modes/interactive/assets/logo.png", "image"),
        ("src/core/export-html/template.html", "html"),
        ("src/core/export-html/template.css", "css"),
        ("src/core/export-html/template.js", "js"),
        ("src/core/export-html/vendor/vendor.js", "vendor"),
        ("README.md", "readme"),
        ("CHANGELOG.md", "history"),
        ("docs/guide.md", "guide"),
        ("examples/example.txt", "example"),
    ] {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }
    fs::create_dir_all(workspace.0.join("target/maestro-viewer")).unwrap();
    fs::write(
        workspace.0.join("target/maestro-viewer/index.html"),
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

/// Owns the recipe's process group, including watcher commands and their children.
#[cfg(unix)]
struct ProcessGroup(std::process::Child);

#[cfg(unix)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        // The fixture disables nested watcher groups so every child inherits this group.
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{}", self.0.id())])
            .status();
        let _ = self.0.wait();
    }
}

#[cfg(unix)]
fn assert_watch_rebuild(recipe: &str, changed: &str) {
    let workspace = Workspace::new();
    prepare_watch_workspace(&workspace);
    prepare_tooling_owner(&workspace);
    let binary = workspace.0.join("target/debug/development");
    let (child, receive, threads) = start_watch(&workspace, recipe, &binary);
    let diagnostics = watcher_diagnostics(&workspace.0);
    assert_watch_events(&workspace, changed, &receive, &diagnostics);
    drop(child);
    drop(receive);
    for thread in threads {
        thread.join().unwrap();
    }
}

/// Starts a recipe with group ownership before reading any output.
#[cfg(unix)]
fn start_watch(
    workspace: &Workspace,
    recipe: &str,
    binary: &Path,
) -> (
    ProcessGroup,
    std::sync::mpsc::Receiver<String>,
    [std::thread::JoinHandle<()>; 1],
) {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::sync::mpsc;
    let mut command = workspace.command(recipe);
    command
        .env("MAESTRO_REAL_CARGO", env!("CARGO"))
        .env("MAESTRO_DEVELOPMENT", binary)
        .env("MAESTRO_REAL_WATCHEXEC", watcher_path());
    // Both output streams share one pipe so completion cannot overtake step output.
    let (output, writer) = std::os::unix::net::UnixStream::pair().unwrap();
    let stdout: std::os::fd::OwnedFd = writer.try_clone().unwrap().into();
    let stderr: std::os::fd::OwnedFd = writer.into();
    command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .process_group(0);
    let child = ProcessGroup(command.spawn().unwrap());
    drop(command);
    let (send, receive) = mpsc::channel();
    let threads = [capture_lines(move || output, send.clone())];
    drop(send);
    (child, receive, threads)
}

/// Completion follows retained stdout and stderr from the same command.
#[cfg(unix)]
#[test]
fn maestro_watch_completion_retains_both_output_streams() {
    use std::io::Write;
    let workspace = Workspace::new();
    writeln!(
        fs::OpenOptions::new().append(true).open(workspace.0.join("justfile")).unwrap(),
        "\nwatch-output:\n    @printf 'source output\\n'\n    @printf 'compiler output\\nFinished `dev`\\n[Command was successful]\\n' >&2"
    ).unwrap();
    let (child, receive, threads) = start_watch(
        &workspace,
        "watch-output",
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
    for thread in threads {
        thread.join().unwrap();
    }
}

/// Panic cleanup closes every descendant's output pipe, not just the recipe leader's.
#[cfg(unix)]
#[test]
fn maestro_watch_stops_its_children_on_panic() {
    let workspace = Workspace::new();
    prepare_watch_workspace(&workspace);
    let (child, receive, threads) = start_watch(
        &workspace,
        "models-dev",
        Path::new(env!("CARGO_BIN_EXE_development")),
    );
    let group = child.0.id();
    wait_for_build(
        &receive,
        &mut String::new(),
        "panic cleanup readiness",
        &watcher_diagnostics(&workspace.0),
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _child = child;
        panic!("controlled panic after starting the watch");
    }));
    let disconnected = loop {
        match receive.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(_) => {}
            Err(error) => break error == std::sync::mpsc::RecvTimeoutError::Disconnected,
        }
    };
    // Rescue descendants when checking an incomplete cleanup implementation.
    if !disconnected {
        Command::new("kill")
            .args(["-KILL", "--", &format!("-{group}")])
            .status()
            .unwrap();
    }
    drop(receive);
    for thread in threads {
        thread.join().unwrap();
    }
    assert!(panic.is_err());
    assert!(
        disconnected,
        "watch descendants retained their output pipes after panic"
    );
}

#[cfg(unix)]
fn prepare_watch_workspace(workspace: &Workspace) {
    native_workspace(workspace);
    for (owner, marker) in [("maestro-models", "source"), ("maestro-agent", "sibling")] {
        let path = workspace.0.join(format!("crates/{owner}/src/lib.rs"));
        let contents = fs::read_to_string(&path).unwrap();
        fs::write(
            path,
            format!("{contents}\npub const MARKER: &str = \"maestro-initial-{marker}\";\n"),
        )
        .unwrap();
    }
    fs::write(
        workspace.0.join("crates/maestro-models/src/main.rs"),
        "fn main() { println!(\"{}\\n{}\\n{}\", maestro_models::MARKER, maestro_agent::MARKER, env!(\"MAESTRO_REBUILD_MARKER\")); }\n",
    ).unwrap();
    fs::write(workspace.0.join("crates/maestro-models/Cargo.toml"), "[package]\nname = 'maestro-models'\nversion = '0.1.0'\nedition = '2024'\n[dependencies]\nmaestro-agent = { path = '../maestro-agent' }\n").unwrap();
    assert!(
        Command::new(env!("CARGO"))
            .current_dir(&workspace.0)
            .arg("generate-lockfile")
            .status()
            .unwrap()
            .success()
    );
    fs::create_dir(workspace.0.join(".cargo")).unwrap();
    fs::write(
        workspace.0.join(".cargo/config.toml"),
        "[build]\ntarget-dir = 'target'\n[env]\nMAESTRO_REBUILD_MARKER = 'maestro-initial-config'\n",
    )
    .unwrap();
    fs::create_dir_all(workspace.0.join("target")).unwrap();
    fs::rename(
        workspace.0.join("bin/cargo-proxy"),
        workspace.0.join("bin/cargo"),
    )
    .unwrap();
}

fn prepare_tooling_owner(workspace: &Workspace) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = workspace.0.join("crates/maestro-tooling");
    copy_tree(&source.join("src"), &root.join("src"));
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
    fs::copy(source.join("Cargo.toml"), root.join("Cargo.toml")).unwrap();
    fs::copy(
        source.join("../../Cargo.toml"),
        workspace.0.join("Cargo.toml"),
    )
    .unwrap();
    assert!(
        Command::new(env!("CARGO"))
            .current_dir(&workspace.0)
            .args(["build", "-p", "maestro-tooling", "--bin", "development"])
            .status()
            .unwrap()
            .success()
    );
}

fn copy_tree(source: &Path, destination: &Path) {
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
        workspace.0.file_name().unwrap().to_str().unwrap()
    );
    let path = workspace.0.join(changed);
    let contents = fs::read_to_string(&path).unwrap();
    let contents = contents.replace(&format!("maestro-initial-{input}"), &marker);
    fs::write(path, contents).unwrap();
    marker
}

/// Waits for the output associated with this mutation, not a queued older build.
fn wait_for_marker(
    receive: &std::sync::mpsc::Receiver<String>,
    transcript: &mut String,
    marker: &str,
    diagnostics: &str,
) {
    while !has_rebuild_marker(transcript, marker) {
        let line = receive.recv_timeout(std::time::Duration::from_secs(10)).unwrap_or_else(|error| {
            panic!("marker {marker}: {error} (10 s wait); {diagnostics}; watcher output:\n{transcript}")
        });
        transcript.push_str(&line);
        transcript.push('\n');
    }
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

/// Captures complete lines from the reader supplied to the thread.
fn capture_lines<R: std::io::Read>(
    reader: impl FnOnce() -> R + Send + 'static,
    send: std::sync::mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    use std::io::{BufRead, BufReader};
    std::thread::spawn(move || {
        for line in BufReader::new(reader()).lines() {
            if send.send(line.unwrap()).is_err() {
                break;
            }
        }
    })
}

/// Finds the pinned external watcher before fixture commands extend PATH.
fn watcher_path() -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|directory| directory.join("watchexec"))
        .find(|path| path.is_file())
        .unwrap()
}

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

fn wait_for_build(
    receive: &std::sync::mpsc::Receiver<String>,
    transcript: &mut String,
    phase: &str,
    diagnostics: &str,
) {
    let started = std::time::Instant::now();
    let mut compiled = false;
    loop {
        let line = receive
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap_or_else(|error| {
                panic!(
                    "{phase}: {error} after {} ms (10 s wait); {diagnostics}; watcher output:\n{transcript}",
                    started.elapsed().as_millis()
                )
            });
        transcript.push_str(&line);
        transcript.push('\n');
        compiled |= line.contains("Finished `dev`");
        if compiled && line.contains("[Command was successful]") {
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
    fs::remove_dir_all(workspace.0.join("crates/maestro-models")).unwrap();
    assert!(!workspace.run("models-dev").status.success());
}

#[test]
fn maestro_native_tests_keep_filters_and_ignored_cases() {
    let workspace = Workspace::new();
    native_workspace(&workspace);
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
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed; 0 ignored"));
    let output = workspace
        .command("models-test")
        .args(["--", "--ignored"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("ignored_behavior ... ok"));
    let output = workspace
        .command("models-test")
        .arg("failing_behavior")
        .env("MAESTRO_TEST_FAIL", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("0 passed; 1 failed"));
    let output = workspace
        .command("models-test")
        .arg("--doc")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
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
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .matches("1 passed; 0 failed; 0 ignored")
            .count(),
        4
    );
}

fn assert_terminal_test_selection(workspace: &Workspace) {
    fs::write(workspace.0.join("crates/maestro-tui/src/lib.rs"), "#[test] fn terminal_behavior() {}\n#[test] fn wrap_ansi_selected() {}\n#[test] #[ignore] fn wrap_ansi_ignored() {}\n#[test] fn selected_behavior() {}\n").unwrap();
    let output = workspace
        .command("tui-test")
        .args(["--", "--nocapture"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("terminal_behavior ... ok"));
    assert!(stdout.contains("wrap_ansi_selected ... ok"));
    assert!(stdout.contains("3 passed; 0 failed; 1 ignored"));
    let output = workspace
        .command("tui-test-ansi")
        .args(["--", "--nocapture"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("terminal_behavior ..."));
    assert!(stdout.contains("wrap_ansi_selected ... ok"));
    assert!(stdout.contains("1 passed; 0 failed; 1 ignored"));
    let output = workspace
        .command("tui-test-ansi")
        .args(["--", "--ignored", "--exact", "wrap_ansi_ignored"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("wrap_ansi_ignored ... ok"));
    assert!(stdout.contains("1 passed; 0 failed; 0 ignored"));
}

#[test]
fn maestro_source_recipes_preserve_invocation_directory() {
    let workspace = Workspace::new();
    let outside = workspace.0.join("outside");
    fs::create_dir(&outside).unwrap();
    for recipe in ["run-source", "run-source-windows"] {
        let output = workspace
            .command("--justfile")
            .arg(workspace.0.join("justfile"))
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
        fs::read(workspace.0.join("cwd")).unwrap(),
        outside.as_os_str().as_encoded_bytes()
    );
    let manifest = workspace.0.join("Cargo.toml");
    let log = workspace.log();
    for recipe in ["run-source", "run-source-windows"] {
        assert!(log.contains(&format!("cargo run|--quiet|--locked|--manifest-path|{}|-p|maestro-tooling|--bin|development|--|{recipe}||a space|'quoted'|\u{feff}\u{85}\u{2003}", manifest.display())), "{log}");
    }
}

#[test]
fn maestro_offline_recipe_is_explicit_only() {
    let workspace = Workspace::new();
    let home = workspace.0.join("home");
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
            .env("MAESTRO_ENV_RESULT", workspace.0.join("env-result"))
            .env("MAESTRO_DEVELOPMENT", env!("CARGO_BIN_EXE_development"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(workspace.0.join("env-result")).unwrap(),
            expected
        );
        assert_eq!(fs::read(&auth).unwrap(), b"synthetic authentication");
    }
    let log = workspace.log();
    assert_eq!(log.matches("--bin|development|--|test-offline").count(), 1);
    assert!(log.contains("cargo test|--workspace|--locked"));
    assert!(log.contains("cargo doc|--workspace|--no-deps|--locked"));
}

fn hooks(workspace: &Workspace) {
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.pre-commit-config.yaml"),
        workspace.0.join(".pre-commit-config.yaml"),
    )
    .unwrap();
    assert!(
        Command::new("git")
            .current_dir(&workspace.0)
            .args(["init", "--quiet"])
            .status()
            .unwrap()
            .success()
    );
}

fn commit_message(workspace: &Workspace, message: &str) -> Output {
    fs::write(workspace.0.join("message"), message).unwrap();
    Command::new("prek")
        .current_dir(&workspace.0)
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
            .current_dir(&workspace.0)
            .arg("install")
            .status()
            .unwrap()
            .success()
    );
    for stage in ["pre-commit", "pre-merge-commit", "commit-msg"] {
        assert!(workspace.0.join(".git/hooks").join(stage).is_file());
    }
    fs::remove_file(workspace.0.join("bin/prek")).unwrap();
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
        fs::write(workspace.0.join(name), content).unwrap();
        let output = Command::new("prek")
            .current_dir(&workspace.0)
            .args(["run", hook, "--files", name])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{hook}");
        fs::write(workspace.0.join(name), "").unwrap();
        assert!(
            Command::new("prek")
                .current_dir(&workspace.0)
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
    let renamed = workspace.0.with_file_name(format!(
        "Maestro's-{}",
        workspace.0.file_name().unwrap().to_str().unwrap()
    ));
    fs::rename(&workspace.0, &renamed).unwrap();
    workspace.0 = renamed;
    let outside = workspace.0.join("outside");
    fs::create_dir(&outside).unwrap();
    for recipe in ["run-source", "run-source-windows", "test-offline"] {
        let output = workspace
            .command("--justfile")
            .arg(workspace.0.join("justfile"))
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
    assert!(workspace.log().contains(&format!(
        "--manifest-path|{}",
        workspace.0.join("Cargo.toml").display()
    )));
}
