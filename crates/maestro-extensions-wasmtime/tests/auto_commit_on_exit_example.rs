//! Shutdown commits through the shipped component, real runner and disposable repositories.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, ExtensionUIContext, Local, create_event_bus_for, create_extension_runtime,
    load_extension_from_factory,
    runner::emit_session_shutdown_event,
    types::{ExtensionUIContextPort, NotificationType, SessionShutdownEvent},
};
use maestro_session::SessionManager;
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{cell::RefCell, path::Path, process::Command, rc::Rc, sync::RwLock};

/// Notifications in arrival order.
#[derive(Default)]
struct Ui {
    /// Each notification's text and kind.
    calls: RefCell<Vec<Value>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(
        &self,
        message: String,
        kind: Option<NotificationType>,
    ) -> Result<(), maestro_extensions::EventError> {
        self.calls
            .borrow_mut()
            .push(json!(["notify", message, kind]));
        Ok(())
    }
}

/// One disposable directory and the real history its runner reads.
struct Harness {
    /// Deletes the directory on completion.
    root: Scratch,
    /// Production runner over the compiled shipped example.
    runner: ExtensionRunner<Local>,
    /// Mutable session history supplied to callbacks.
    history: Rc<RefCell<SessionManager>>,
    /// Caller-controlled UI.
    ui: Rc<Ui>,
}
impl Harness {
    /// Loads the compiled example over a fresh directory, a Git repository when `repository`.
    async fn new(repository: bool) -> Self {
        let root = Scratch::new().unwrap();
        if repository {
            git(root.path(), &["init", "-q"]);
            // Git for Windows converts line endings system-wide, and the developer's settings
            // may sign commits; the example's own Git commands read these repository settings.
            for (key, value) in [
                ("user.email", "commit@example.invalid"),
                ("user.name", "Commit test"),
                ("commit.gpgsign", "false"),
                ("core.autocrlf", "false"),
            ] {
                git(root.path(), &["config", key, value]);
            }
            std::fs::write(root.path().join("tracked.txt"), "committed\n").unwrap();
            git(root.path(), &["add", "tracked.txt"]);
            git(root.path(), &["commit", "-qm", "initial"]);
        }
        let cwd = root.path().to_str().unwrap();
        let runtime = create_extension_runtime::<Local>();
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let path = std::env::var("MAESTRO_AUTO_COMMIT_ON_EXIT_COMPONENT")
            .expect("nextest component setup");
        let owner = load_extension_from_factory(
            crate::artifact(path.clone()),
            cwd.into(),
            bus.for_extension(),
            runtime.clone(),
            Some(path),
        )
        .await
        .unwrap();
        let (session, history) = super::session::runner_session(cwd);
        let runner = ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(vec![owner])),
            runtime,
            session,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        );
        super::support::bind_prompt(&runner, "commit test");
        let _remove = runner.on_error(Rc::new(|error| {
            panic!("unexpected callback error: {error:?}")
        }));
        let ui = Rc::new(Ui::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        Self {
            root,
            runner,
            history,
            ui,
        }
    }

    /// Changes the tracked file and adds a new one, leaving the index alone.
    fn change(&self, text: &str) {
        std::fs::write(self.root.path().join("tracked.txt"), text).unwrap();
        std::fs::write(self.root.path().join(format!("{}.txt", text.len())), text).unwrap();
    }

    /// Appends an assistant reply with the given content blocks.
    fn reply(&self, content: &Value) {
        self.history
            .borrow_mut()
            .append_message(json!({
                "role":"assistant", "content":content, "api":"fixture", "provider":"fixture",
                "model":"fixture", "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,
                "totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
                "stopReason":"stop", "timestamp":1
            }))
            .unwrap();
    }

    /// Appends a user prompt.
    fn prompt(&self, text: &str) {
        self.history
            .borrow_mut()
            .append_message(json!({"role":"user","content":text,"timestamp":1}))
            .unwrap();
    }

    /// Shuts the session down through the production emitter and returns the new UI calls.
    async fn shutdown(&self) -> Vec<Value> {
        self.ui.calls.borrow_mut().clear();
        let mut event =
            serde_json::from_value::<SessionShutdownEvent>(json!({"reason":"quit"})).unwrap();
        assert!(
            emit_session_shutdown_event(&self.runner, &mut event)
                .await
                .unwrap()
        );
        self.ui.calls.borrow().clone()
    }

    /// The newest commit's full message.
    fn head_message(&self) -> String {
        git(self.root.path(), &["log", "-1", "--format=%B"])
    }

    /// The number of commits on the current branch.
    fn commits(&self) -> String {
        git(self.root.path(), &["rev-list", "--count", "HEAD"])
    }
}

/// Runs Git only inside an exclusively owned disposable directory.
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Runs native process futures on their caller-owned local executor.
fn run(action: impl Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, action);
}

/// The notification for one successful commit.
fn committed(message: &str) -> Vec<Value> {
    vec![json!([
        "notify",
        format!("Committed on exit: {message}"),
        "info"
    ])]
}

/// A directory outside any repository and a clean checkout are left as they are.
#[test]
fn auto_commit_on_exit_component_skips_clean_and_non_repository_exits() {
    run(async {
        let outside = Harness::new(false).await;
        assert!(
            !Command::new("git")
                .current_dir(outside.root.path())
                .args(["rev-parse", "--git-dir"])
                .output()
                .unwrap()
                .status
                .success(),
            "the temporary root must lie outside any repository"
        );
        outside.reply(&json!([{"type":"text","text":"reply"}]));
        assert!(outside.shutdown().await.is_empty());
        assert!(!outside.root.path().join(".git").exists());

        let clean = Harness::new(true).await;
        clean.reply(&json!([{"type":"text","text":"reply"}]));
        assert!(clean.shutdown().await.is_empty());
        assert_eq!(clean.commits(), "1\n");
    });
}

/// The newest reply's first text line titles the commit, cut to fifty characters.
#[test]
fn auto_commit_on_exit_component_titles_commit_with_newest_reply() {
    run(async {
        let h = Harness::new(true).await;
        h.reply(&json!([{"type":"text","text":"An older reply"}]));
        h.prompt("next");
        let long = "Rename the session loader and update every caller of it";
        h.reply(&json!([
            {"type":"thinking","thinking":"plan"},
            {"type":"text","text":format!("{long}\nsecond line")},
            {"type":"text","text":"another block"}
        ]));
        h.change("first change\n");
        let message = format!("[maestro] {}...", &long[..50]);
        assert_eq!(h.shutdown().await, committed(&message));
        assert_eq!(h.head_message(), format!("{message}\n\n"));
        assert_eq!(git(h.root.path(), &["status", "--porcelain"]), "");

        let exact = "x".repeat(50);
        h.reply(&json!([{"type":"text","text":exact}]));
        h.change("second change\n");
        assert_eq!(h.shutdown().await, committed(&format!("[maestro] {exact}")));

        // Fifty characters, not fifty UTF-16 units, so no character is cut in half.
        let wide = format!("a{}", "\u{1F3BB}".repeat(50));
        h.reply(&json!([{"type":"text","text":wide}]));
        h.change("third change, wide\n");
        let message = format!("[maestro] a{}...", "\u{1F3BB}".repeat(49));
        assert_eq!(h.shutdown().await, committed(&message));
        assert_eq!(h.head_message(), format!("{message}\n\n"));
        assert_eq!(h.commits(), "4\n");
    });
}

/// Without reply text the commit takes the fallback title, and a headless exit stays silent.
#[test]
fn auto_commit_on_exit_component_falls_back_without_reply_text() {
    run(async {
        let h = Harness::new(true).await;
        let fallback = "[maestro] Session changes";
        h.prompt("no reply yet");
        h.change("no reply\n");
        assert_eq!(h.shutdown().await, committed(fallback));
        assert_eq!(h.head_message(), format!("{fallback}\n\n"));

        h.reply(&json!([{"type":"text","text":"older text"}]));
        h.reply(&json!([{"type":"toolCall","id":"call","name":"read","arguments":{}}]));
        h.change("newest reply has no text\n");
        assert_eq!(h.shutdown().await, committed(fallback));

        h.reply(&json!([{"type":"text","text":"\nsecond line only"}]));
        h.change("empty first line\n");
        assert_eq!(h.shutdown().await, committed(fallback));

        h.runner.set_ui_context(None);
        h.change("headless\n");
        assert!(h.shutdown().await.is_empty());
        assert_eq!(h.head_message(), format!("{fallback}\n\n"));
        assert_eq!(h.commits(), "5\n");
    });
}

/// A failed commit announces nothing and leaves the staged changes for the reader.
#[test]
fn auto_commit_on_exit_component_stays_silent_when_commit_fails() {
    run(async {
        let h = Harness::new(true).await;
        for (key, value) in [
            ("commit.gpgsign", "true"),
            ("gpg.format", "openpgp"),
            ("gpg.program", "/nonexistent/signing-program"),
        ] {
            git(h.root.path(), &["config", key, value]);
        }
        h.reply(&json!([{"type":"text","text":"reply"}]));
        h.change("unsigned\n");
        assert!(h.shutdown().await.is_empty());
        assert_eq!(h.commits(), "1\n");
        assert_eq!(
            git(h.root.path(), &["diff", "--cached", "--name-only"]),
            "9.txt\ntracked.txt\n"
        );
    });
}

/// Global commit hooks cannot change the result of the component's disposable Git tests.
#[cfg(unix)]
#[test]
fn auto_commit_on_exit_component_ignores_global_hooks() {
    use std::os::unix::fs::PermissionsExt;

    let root = Scratch::new().unwrap();
    let hooks = root.path().join("hooks");
    std::fs::create_dir(&hooks).unwrap();
    let hook = hooks.join("pre-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = root.path().join("global.gitconfig");
    git(
        root.path(),
        &[
            "config",
            "--file",
            config.to_str().unwrap(),
            "core.hooksPath",
            hooks.to_str().unwrap(),
        ],
    );
    let output = maestro_test_process::TestProcess::named(
        maestro_test_process::test_name(
            auto_commit_on_exit_component_titles_commit_with_newest_reply,
        )
        .unwrap(),
    )
    .unwrap()
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_CONFIG_GLOBAL", &config)
    .nocapture()
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
