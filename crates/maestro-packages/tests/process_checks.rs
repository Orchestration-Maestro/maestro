//! Real children launched through the native package operations.
#![cfg(unix)]
#![cfg(test)]
use crate::runtime::runtime;
use crate::scratch;
use maestro_packages::{
    DefaultPackageManager, NativePackageOperations, PackageManager, PackageManagerOptions,
    PackageOperations,
};
use maestro_settings::{Settings, SettingsManager};
use maestro_test_process::TestProcess;
use scratch::Scratch;
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    io::Read,
    path::Path,
    process::Command,
    rc::Rc,
};

/// A path below the directory.
fn at(root: &Scratch, relative: &str) -> String {
    root.path().join(relative).to_string_lossy().into_owned()
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
            cwd: at(root, "project"),
            agent_dir: at(root, "agent"),
            settings_manager: Rc::new(RefCell::new(settings)),
        },
        NativePackageOperations::new(
            Rc::new(|| false),
            &std::rc::Rc::new(tokio::task::LocalSet::new()),
        ),
    )
}

/// Native operations that never use the shell and never take over stdout.
fn direct() -> NativePackageOperations {
    NativePackageOperations::new(
        Rc::new(|| false),
        &std::rc::Rc::new(tokio::task::LocalSet::new()),
    )
}

#[test]
fn parallel_scratch_directories_are_all_created_and_distinct() {
    let threads: Vec<_> = (0..32)
        .map(|_| std::thread::spawn(|| Scratch::new().unwrap()))
        .collect();
    let scratches: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    let paths: std::collections::HashSet<_> = scratches.iter().map(Scratch::path).collect();
    assert_eq!(paths.len(), 32);
    assert!(scratches.iter().all(|s| s.path().is_dir()));
}

#[test]
fn ordinary_commands_finish_at_exit_with_exact_failure_text() {
    let root = Scratch::new().unwrap();
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

    let (held, done) = (at(&root, "held.fifo"), at(&root, "done.fifo"));
    for fifo in [&held, &done] {
        assert!(Command::new("mkfifo").arg(fifo).status().unwrap().success());
    }
    let script = format!(
        r#"HELD='{held}'; DONE='{done}'; export HELD DONE; ( cat < "$HELD" > /dev/null; echo done > "$DONE" ) & exit 0 #"#
    );
    let (finished, outcome) = std::sync::mpsc::channel();
    let mut text = String::new();
    let completion = std::thread::scope(|scope| {
        scope.spawn(|| {
            let manager = manager_running(&script, &root);
            let result = self::runtime().block_on(manager.install("npm:x", None));
            finished.send(result.is_ok()).unwrap();
        });
        let verdict = outcome.recv();
        // Release the background holder after observing install completion, then drain it.
        drop(std::fs::OpenOptions::new().write(true).open(&held).unwrap());
        std::fs::File::open(&done)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        verdict
    });
    assert_eq!(
        completion,
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
fn stdio_launches(root: &Path, supplied: bool) {
    let taken = Rc::new(Cell::new(false));
    let query = taken.clone();
    let query: Rc<dyn Fn() -> bool> = Rc::new(move || query.get());
    let operations: Box<dyn PackageOperations> = Box::new(if supplied {
        NativePackageOperations::with_spawn(query, |_| panic!("stdio must not admit workers"))
    } else {
        NativePackageOperations::new(query, &Rc::new(tokio::task::LocalSet::new()))
    });
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
    stdio_scenario(
        "native_stdio_obeys_live_takeover_and_literal_arguments",
        false,
    );
}

/// Drives the same native stream routes through either admission constructor.
fn stdio_scenario(test: &str, supplied: bool) {
    if let Some(root) = std::env::var_os(STDIO_CHILD) {
        stdio_launches(Path::new(&root), supplied);
        return;
    }
    let root = Scratch::new().unwrap();
    for directory in ["ambient", "target"] {
        std::fs::create_dir(at(&root, directory)).unwrap();
    }
    std::fs::write(at(&root, "input"), "line1\nline2\nline3\n").unwrap();
    let (out, err) = (at(&root, "out"), at(&root, "err"));
    let stdout = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&out)
        .unwrap();
    let status = TestProcess::current(test)
        .unwrap()
        .nocapture()
        .env(STDIO_CHILD, root.path())
        .current_dir(at(&root, "ambient"))
        .stdin(std::fs::File::open(at(&root, "input")).unwrap())
        .stderr(std::fs::File::create(&err).unwrap())
        .status_with_stdout(stdout)
        .unwrap();
    assert!(status.success());
    let (out, err) = (
        std::fs::read_to_string(out).unwrap(),
        std::fs::read_to_string(err).unwrap(),
    );
    let canonical = |name: &str| {
        root.path()
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
    let root = Scratch::new().unwrap();
    let repository = at(&root, "origin");
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
    let output = maestro_test_process::TestProcess::current(
        "native_local_repository_installs_checks_out_and_removes",
    )
    .unwrap()
    .env(REPOSITORY_CHILD, root.path())
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

    let root = Scratch::new().unwrap();
    let settings = Rc::new(RefCell::new(SettingsManager::in_memory(Settings(
        json!({"npmCommand": ["/nonexistent/maestro-binary"]})
            .as_object()
            .cloned()
            .unwrap(),
    ))));
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: at(&root, "project"),
            agent_dir: at(&root, "agent"),
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
fn native_package_spawn_keeps_admission_and_output_routes() {
    stdio_scenario(
        "native_package_spawn_keeps_admission_and_output_routes",
        true,
    );
    let admitted = Rc::new(Cell::new(0));
    let seen = admitted.clone();
    let local = Rc::new(tokio::task::LocalSet::new());
    let target = local.clone();
    let operations = NativePackageOperations::with_spawn(Rc::new(|| false), move |work| {
        seen.set(seen.get() + 1);
        drop(target.spawn_local(work));
        Ok(())
    });
    let (send, receive) = tokio::sync::oneshot::channel();
    operations
        .spawn(Box::pin(async move {
            send.send(()).unwrap();
        }))
        .unwrap();
    assert_eq!(admitted.get(), 1);
    runtime().block_on(local.run_until(async {
        receive.await.unwrap();
    }));
    assert_supplied_capture_routes();
}

#[test]
fn native_package_spawn_preserves_local_set_release_error() {
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    let (send, receive) = tokio::sync::oneshot::channel();
    operations
        .spawn(Box::pin(async move {
            send.send(()).unwrap();
        }))
        .unwrap();
    runtime().block_on(local.run_until(async {
        receive.await.unwrap();
    }));
    assert_eq!(Rc::strong_count(&local), 1);
    drop(local);
    assert_eq!(
        operations
            .spawn(Box::pin(async {}))
            .unwrap_err()
            .to_string(),
        "package runtime is no longer available"
    );
    let local = Rc::new(tokio::task::LocalSet::new());
    let weak = Rc::downgrade(&local);
    let supplied = NativePackageOperations::with_spawn(Rc::new(|| false), move |work| {
        let local = weak
            .upgrade()
            .ok_or_else(|| std::io::Error::other("package runtime is no longer available"))?;
        drop(local.spawn_local(work));
        Ok(())
    });
    let (send, receive) = tokio::sync::oneshot::channel();
    supplied
        .spawn(Box::pin(async move {
            send.send(()).unwrap();
        }))
        .unwrap();
    runtime().block_on(local.run_until(async {
        receive.await.unwrap();
    }));
    drop(local);
    assert_eq!(
        supplied.spawn(Box::pin(async {})).unwrap_err().to_string(),
        "package runtime is no longer available"
    );
}

/// Checks unchanged native capture operands beside an admission error.
fn assert_supplied_capture_routes() {
    let denied = NativePackageOperations::with_spawn(
        Rc::new(|| panic!("capture must not query output routing")),
        |_| Err(std::io::Error::from_raw_os_error(13)),
    );
    assert_eq!(
        denied.spawn(Box::pin(async {})).unwrap_err().raw_os_error(),
        Some(13)
    );
    let output = denied
        .run_command_sync(
            "/bin/sh",
            &["-c".into(), "printf sync; printf err >&2; exit 7".into()],
        )
        .unwrap();
    assert_eq!(
        (
            output.status,
            output.stdout.as_str(),
            output.stderr.as_str()
        ),
        (Some(7), "sync", "err")
    );
    let root = Scratch::new().unwrap();
    let cwd = root.path().to_str().unwrap();
    let output = runtime()
        .block_on(denied.run_command_capture(
            "/bin/sh",
            &[
                "-c".into(),
                "printf '%s|%s|' \"$MAESTRO_CAPTURE_VALUE\" \"$1\"; pwd -P".into(),
                "sh".into(),
                "literal argument".into(),
            ],
            maestro_packages::CommandCaptureOptions {
                cwd: Some(cwd),
                timeout: None,
                env: &[("MAESTRO_CAPTURE_VALUE", "overlaid")],
            },
        ))
        .unwrap();
    assert_eq!(
        output,
        format!(
            "overlaid|literal argument|{}",
            root.path().canonicalize().unwrap().display()
        )
    );
}
