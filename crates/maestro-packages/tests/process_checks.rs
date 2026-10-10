//! Real children launched through the native package operations.
#![cfg(unix)]
#![cfg(test)]
use maestro_packages::{
    DefaultPackageManager, NativePackageOperations, PackageManager, PackageManagerOptions,
    PackageOperations,
};
use maestro_settings::{Settings, SettingsManager};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
};

/// A disposable directory removed on drop.
struct Scratch(PathBuf);
impl Scratch {
    /// Creates a fresh directory with an unused name.
    fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("maestro-process-{}-{nanos}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    /// A path below the directory.
    fn at(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().into_owned()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        assert!(std::fs::remove_dir_all(&self.0).is_ok());
    }
}

/// A runtime with process support for driving native futures.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// A native manager running the given shell script as its package command.
fn manager_running(script: &str, root: &Scratch) -> DefaultPackageManager<NativePackageOperations> {
    let settings = SettingsManager::in_memory(Settings(
        json!({"npmCommand": ["/bin/sh", "-c", script]})
            .as_object()
            .cloned()
            .unwrap(),
    ));
    DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: root.at("project"),
            agent_dir: root.at("agent"),
            settings_manager: Rc::new(RefCell::new(settings)),
        },
        NativePackageOperations::new(|_| false, Rc::new(|| false)),
    )
}

/// Native operations that never use the shell and never take over stdout.
fn direct() -> NativePackageOperations {
    NativePackageOperations::new(|_| false, Rc::new(|| false))
}

#[test]
fn ordinary_commands_finish_at_exit_with_exact_failure_text() {
    let root = Scratch::new();
    let runtime = runtime();
    let install = |script: &str| {
        let manager = manager_running(script, &root);
        runtime.block_on(manager.install("npm:x", None))
    };
    install("exit 0 #").unwrap();
    assert_eq!(
        install("exit 3 #").unwrap_err().to_string(),
        "/bin/sh -c exit 3 # install -g x failed with code 3"
    );
    assert_eq!(
        install("kill -9 $$ #").unwrap_err().to_string(),
        "/bin/sh -c kill -9 $$ # install -g x failed with code null"
    );

    let (held, done) = (root.at("held.fifo"), root.at("done.fifo"));
    for fifo in [&held, &done] {
        assert!(Command::new("mkfifo").arg(fifo).status().unwrap().success());
    }
    let script = format!(
        r#"HELD='{held}'; DONE='{done}'; export HELD DONE; ( cat < "$HELD" > /dev/null; echo done > "$DONE" ) & exit 0 #"#
    );
    let (finished, outcome) = std::sync::mpsc::channel();
    let mut text = String::new();
    let finished_in_time = std::thread::scope(|scope| {
        scope.spawn(|| {
            let manager = manager_running(&script, &root);
            let result = self::runtime().block_on(manager.install("npm:x", None));
            finished.send(result.is_ok()).unwrap();
        });
        let verdict = outcome.recv_timeout(std::time::Duration::from_secs(20));
        // Release and drain the background holder so the scope can join even when the wait failed.
        drop(std::fs::OpenOptions::new().write(true).open(&held).unwrap());
        std::fs::File::open(&done)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        verdict
    });
    assert_eq!(
        finished_in_time,
        Ok(true),
        "install must settle at child exit while a background process holds its streams"
    );
    assert_eq!(text, "done\n");
}

/// Marks the child process of the stdio test and carries the scratch directory.
const STDIO_CHILD: &str = "MAESTRO_STDIO_CHILD";
/// The shell script run by each launch of the stdio test.
const STDIO_SCRIPT: &str =
    r#"printf '<%s>' "$@"; echo out-$0; echo err-$0 >&2; read -r line; echo "stdin:$line"; pwd -P"#;

/// Runs the three launches of the stdio test inside the child process.
fn stdio_launches(root: &Path) {
    let taken = Rc::new(Cell::new(false));
    let query = taken.clone();
    let operations: Box<dyn PackageOperations> = Box::new(NativePackageOperations::new(
        |_| false,
        Rc::new(move || query.get()),
    ));
    let runtime = runtime();
    let launch = |name: &str, cwd: Option<&str>| {
        let args = ["-c", STDIO_SCRIPT, name, "x  y", "z"].map(str::to_owned);
        let code = runtime.block_on(operations.run_command("/bin/sh", &args, cwd));
        assert_eq!(code.unwrap(), Some(0));
    };
    launch("A", None);
    taken.set(true);
    launch("B", None);
    taken.set(false);
    launch("C", Some(&root.join("target").to_string_lossy()));
}

#[test]
fn native_stdio_obeys_live_takeover_and_literal_arguments() {
    if let Some(root) = std::env::var_os(STDIO_CHILD) {
        stdio_launches(Path::new(&root));
        return;
    }
    let root = Scratch::new();
    for directory in ["ambient", "target"] {
        std::fs::create_dir(root.at(directory)).unwrap();
    }
    std::fs::write(root.at("input"), "line1\nline2\nline3\n").unwrap();
    let (out, err) = (root.at("out"), root.at("err"));
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_stdio_obeys_live_takeover_and_literal_arguments",
            "--nocapture",
        ])
        .env(STDIO_CHILD, &root.0)
        .current_dir(root.at("ambient"))
        .stdin(std::fs::File::open(root.at("input")).unwrap())
        .stdout(std::fs::File::create(&out).unwrap())
        .stderr(std::fs::File::create(&err).unwrap())
        .status()
        .unwrap();
    assert!(status.success());
    let (out, err) = (
        std::fs::read_to_string(out).unwrap(),
        std::fs::read_to_string(err).unwrap(),
    );
    let canonical = |name: &str| {
        root.0
            .join(name)
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    };
    assert!(out.contains(&format!(
        "<x  y><z>out-A\nstdin:line1\n{}\n",
        canonical("ambient")
    )));
    assert!(err.contains("err-A\n"));
    assert!(out.contains(&format!(
        "<x  y><z>out-C\nstdin:line2\n{}\n",
        canonical("target")
    )));
    assert!(err.contains("<x  y><z>out-B\nerr-B\nstdin:\n"));
    assert!(!out.contains("out-B"));
}

/// Marks the child process of the repository test and carries the scratch directory.
const REPOSITORY_CHILD: &str = "MAESTRO_REPOSITORY_CHILD";
/// The source whose clone address the child's Git configuration rewrites to a local path.
const REPOSITORY_SOURCE: &str = "https://example.test/team/repo@v1";

/// Runs a Git command in `directory` with a fixed identity.
fn git(directory: &str, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.test",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
        ])
        .args(args)
        .current_dir(directory)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// Installs, inspects and removes the repository inside the child process.
fn repository_scenario(root: &Path) {
    let scratch = |name: &str| root.join(name).to_string_lossy().into_owned();
    let settings = SettingsManager::in_memory(Settings::default());
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: scratch("project"),
            agent_dir: scratch("agent"),
            settings_manager: Rc::new(RefCell::new(settings)),
        },
        direct(),
    );
    let runtime = runtime();
    runtime
        .block_on(manager.install(REPOSITORY_SOURCE, None))
        .unwrap();
    let target = scratch("agent/git/example.test/team/repo");
    assert_eq!(
        std::fs::read_to_string(format!("{target}/marker")).unwrap(),
        "one"
    );
    assert_eq!(
        std::fs::read_to_string(scratch("agent/git/.gitignore")).unwrap(),
        "*\n!.gitignore\n"
    );
    let head = Command::new("git")
        .args(["-C", &target, "rev-parse", "HEAD"])
        .output()
        .unwrap();
    let tag = Command::new("git")
        .args(["-C", &target, "rev-parse", "v1^{commit}"])
        .output()
        .unwrap();
    assert_eq!(head.stdout, tag.stdout);
    runtime
        .block_on(manager.remove(REPOSITORY_SOURCE, None))
        .unwrap();
    assert!(!Path::new(&scratch("agent/git/example.test")).exists());
    assert!(Path::new(&scratch("agent/git/.gitignore")).exists());
}

#[test]
fn native_local_repository_installs_checks_out_and_removes() {
    if let Some(root) = std::env::var_os(REPOSITORY_CHILD) {
        repository_scenario(Path::new(&root));
        return;
    }
    let root = Scratch::new();
    let repository = root.at("origin");
    std::fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "-q"]);
    for (text, tag) in [("one", true), ("two", false)] {
        std::fs::write(format!("{repository}/marker"), text).unwrap();
        git(&repository, &["add", "marker"]);
        git(&repository, &["commit", "-q", "-m", text]);
        if tag {
            git(&repository, &["tag", "v1"]);
        }
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_local_repository_installs_checks_out_and_removes",
        ])
        .env(REPOSITORY_CHILD, &root.0)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", format!("url.{repository}.insteadOf"))
        .env("GIT_CONFIG_VALUE_0", "https://example.test/team/repo")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

#[test]
fn native_commands_preserve_spawn_failures_and_signal_status() {
    let runtime = runtime();
    let operations: &dyn PackageOperations = &direct();
    let missing =
        runtime.block_on(operations.run_command("/nonexistent/maestro-binary", &[], None));
    assert_eq!(missing.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    let sh = |script: &str| ["-c", script].map(str::to_owned);
    assert_eq!(
        runtime
            .block_on(operations.run_command("/bin/sh", &sh("exit 7"), None))
            .unwrap(),
        Some(7)
    );
    assert_eq!(
        runtime
            .block_on(operations.run_command("/bin/sh", &sh("kill -9 $$"), None))
            .unwrap(),
        None
    );

    let root = Scratch::new();
    let settings = Rc::new(RefCell::new(SettingsManager::in_memory(Settings(
        json!({"npmCommand": ["/nonexistent/maestro-binary"]})
            .as_object()
            .cloned()
            .unwrap(),
    ))));
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: root.at("project"),
            agent_dir: root.at("agent"),
            settings_manager: settings,
        },
        direct(),
    );
    let error = runtime
        .block_on(manager.install("npm:x", None))
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(!error.to_string().contains("failed with code"));
}

#[test]
fn supplied_shell_choice_is_observed_for_each_command() {
    let runtime = runtime();
    let operations: &dyn PackageOperations =
        &NativePackageOperations::new(|command| command == "true", Rc::new(|| false));
    let args = ["a;", "exit", "7"].map(str::to_owned);
    assert_eq!(
        runtime
            .block_on(operations.run_command("true", &args, None))
            .unwrap(),
        Some(7)
    );
    assert_eq!(
        runtime
            .block_on(operations.run_command("/usr/bin/true", &args, None))
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        operations.run_command_sync("true", &args).unwrap().status,
        Some(7)
    );
}
