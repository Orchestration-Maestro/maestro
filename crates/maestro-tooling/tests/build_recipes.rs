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
        fs::write(&source, r#"
use std::{env, fs, io::Write, path::Path};
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if let Some(path) = env::var_os("MAESTRO_CWD") { fs::write(path, env::current_dir().unwrap().as_os_str().as_encoded_bytes()).unwrap(); }
    let name = Path::new(&env::args().next().unwrap()).file_stem().unwrap().to_str().unwrap().to_owned();
    let mut log = fs::OpenOptions::new().create(true).append(true).open(env::var_os("MAESTRO_LOG").unwrap()).unwrap();
    writeln!(log, "{name} {}", args.join("|")).unwrap();
    if let Some(path) = env::var_os("MAESTRO_ENV_RESULT") {
        let auth = Path::new(&env::var_os("HOME").unwrap()).join(".maestro/agent/auth.json");
        fs::write(path, format!("key={} auth={} flag={}\n", env::var_os("OPENAI_API_KEY").is_some(), auth.is_file(), env::var("MAESTRO_NO_LOCAL_LLM").unwrap_or_default())).unwrap();
    }
    if args.first().map(String::as_str) == Some("run") {
        if let Some(binary) = env::var_os("MAESTRO_DEVELOPMENT") {
            let start = args.iter().position(|arg| arg == "--").unwrap() + 1;
            std::process::exit(std::process::Command::new(binary).args(&args[start..]).status().unwrap().code().unwrap());
        }
    }
    if env::var("MAESTRO_FAIL").ok().as_deref() == args.first().map(String::as_str) { std::process::exit(17); }
}
"#).unwrap();
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
    assert!(workspace.run("build").status.success());
    assert!(
        workspace
            .0
            .join("target/debug/libmaestro_models.rlib")
            .exists()
    );
    assert!(workspace.run("models-clean").status.success());
    assert!(
        !workspace
            .0
            .join("target/debug/libmaestro_models.rlib")
            .exists()
    );
    assert!(
        workspace
            .0
            .join("target/debug/libmaestro_agent.rlib")
            .exists()
    );
    assert!(
        workspace
            .0
            .join("crates/maestro-models/src/lib.rs")
            .exists()
    );
    assert!(workspace.run("clean").status.success());
    assert!(
        !workspace
            .0
            .join("target/debug/libmaestro_agent.rlib")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn maestro_dev_rebuilds_and_retains_output() {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::sync::mpsc;
    use std::time::Duration;
    let workspace = Workspace::new();
    native_workspace(&workspace);
    fs::remove_file(workspace.0.join("bin/watchexec")).unwrap();
    let mut command = workspace.command("dev");
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command.spawn().unwrap();
    let (send, receive) = mpsc::channel();
    let output = child.stdout.take().unwrap();
    let error = child.stderr.take().unwrap();
    let threads = [
        capture_lines(output, send.clone()),
        capture_lines(error, send.clone()),
    ];
    drop(send);
    let diagnostics = watcher_diagnostics(&workspace.0);
    let mut transcript = String::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        wait_for_build(&receive, &mut transcript, "initial build", &diagnostics);
        let source = workspace.0.join("crates/maestro-models/src/lib.rs");
        fs::write(&source, "pub fn changed() {}\n").unwrap();
        wait_for_build(&receive, &mut transcript, "source rebuild", &diagnostics);
        assert_eq!(transcript.matches("Finished `dev`").count(), 2);
        assert!(!transcript.contains("\u{1b}[2J"));
        fs::write(workspace.0.join("target/asset"), "ignored").unwrap();
        while let Ok(line) = receive.recv_timeout(Duration::from_millis(300)) {
            assert!(
                !line.contains("Finished `dev`"),
                "unexpected rebuild: {line}"
            );
        }
    }));
    assert!(
        Command::new("kill")
            .args(["-TERM", "--", &format!("-{}", child.id())])
            .status()
            .unwrap()
            .success()
    );
    child.wait().unwrap();
    drop(receive);
    for thread in threads {
        thread.join().unwrap();
    }
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

fn capture_lines(
    reader: impl std::io::Read + Send + 'static,
    send: std::sync::mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    use std::io::{BufRead, BufReader};
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            if send.send(line.unwrap()).is_err() {
                break;
            }
        }
    })
}

fn watcher_diagnostics(root: &Path) -> String {
    let path = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|directory| directory.join("watchexec"))
        .find(|path| path.is_file())
        .unwrap();
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
        if line.contains("Finished `dev`") {
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
        assert!(log.contains(&format!("--|cargo|build|-p|{owner}|--locked")));
    }
    assert!(!log.contains("generate"));
    assert!(!log.contains("copy"));
    assert!(!log.contains("clear"));
    assert!(log.contains("--on-busy-update=queue|--shell=none"));
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
