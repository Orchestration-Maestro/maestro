//! Git checkpoints through the shipped component, real runner and disposable repositories.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, ExtensionUIContext, Local, create_event_bus_for, create_extension_runtime,
    load_extension_from_factory,
    types::{ExtensionUIContextPort, ExtensionUIFuture, NotificationType},
};
use maestro_session::SessionManager;
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{cell::RefCell, path::Path, process::Command, rc::Rc, sync::RwLock};

/// Scripted selector with exact observations of requests and notifications.
#[derive(Default)]
struct Ui {
    /// Next choice, including dismissal.
    choice: RefCell<Option<String>>,
    /// Requests in arrival order.
    calls: RefCell<Vec<Value>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn select(
        &self,
        title: String,
        options: Vec<String>,
        opts: Option<maestro_extensions::ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        assert!(opts.is_none_or(|options| options.timeout.is_none()));
        self.calls
            .borrow_mut()
            .push(json!(["select", title, options]));
        let answer = self.choice.borrow().clone();
        Box::pin(async move { Ok(answer) })
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

/// One isolated checkout and the real history its runner reads.
struct Harness {
    /// Deletes the repository on completion.
    root: Scratch,
    /// Production runner over the compiled shipped example.
    runner: ExtensionRunner<Local>,
    /// Mutable session history supplied to callbacks.
    history: Rc<RefCell<SessionManager>>,
    /// Caller-controlled UI.
    ui: Rc<Ui>,
}
impl Harness {
    /// Initializes a committed tracked file and loads the compiled example.
    async fn new() -> Self {
        let root = Scratch::new().unwrap();
        git(root.path(), &["init", "-q"]);
        git(
            root.path(),
            &["config", "user.email", "checkpoint@example.invalid"],
        );
        git(root.path(), &["config", "user.name", "Checkpoint test"]);
        std::fs::write(root.path().join("tracked.txt"), "committed\n").unwrap();
        git(root.path(), &["add", "tracked.txt"]);
        git(root.path(), &["commit", "-qm", "initial"]);
        let cwd = root.path().to_str().unwrap();
        let runtime = create_extension_runtime::<Local>();
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let path =
            std::env::var("MAESTRO_GIT_CHECKPOINT_COMPONENT").expect("nextest component setup");
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
        super::support::bind_prompt(&runner, "checkpoint test");
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

    /// Writes the sole tracked file without touching the index.
    fn write(&self, text: &str) {
        std::fs::write(self.root.path().join("tracked.txt"), text).unwrap();
    }

    /// Reads the actual working-tree contents.
    fn text(&self) -> String {
        std::fs::read_to_string(self.root.path().join("tracked.txt")).unwrap()
    }

    /// Saves a session entry, then lets the registered tool-result hook track it.
    async fn track(&self) -> String {
        let id = self
            .history
            .borrow_mut()
            .append_custom_entry("checkpoint-test", maestro_session::Presence::Missing)
            .unwrap();
        self.tool_result().await;
        id
    }

    /// Delivers an ordinary result through the owning runner interface.
    async fn tool_result(&self) {
        let event = serde_json::from_value(json!({"toolCallId":"call", "toolName":"read", "input":{}, "content":[], "isError":false})).unwrap();
        assert!(
            self.runner
                .emit_tool_result(&event)
                .await
                .unwrap()
                .is_none()
        );
    }

    /// Captures a checkpoint through the registered turn-start hook.
    async fn turn(&self) {
        let mut event = serde_json::from_value::<maestro_extensions::types::TurnStartEvent>(
            json!({"turnIndex":0,"timestamp":0}),
        )
        .unwrap();
        assert!(self.runner.emit(&mut event).await.unwrap().is_none());
    }

    /// Requests a fork with a caller-selected answer and returns only its new UI effects.
    async fn fork(&self, id: &str, choice: Option<&str>) -> Vec<Value> {
        *self.ui.choice.borrow_mut() = choice.map(str::to_owned);
        self.ui.calls.borrow_mut().clear();
        let mut event =
            serde_json::from_value::<maestro_extensions::types::SessionBeforeForkEvent>(
                json!({"entryId":id,"position":"at"}),
            )
            .unwrap();
        assert!(self.runner.emit(&mut event).await.unwrap().is_none());
        self.ui.calls.borrow().clone()
    }
}

/// Runs Git only inside an exclusively owned disposable repository, with the environment
/// the example's own Git commands inherit.
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
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

/// Runs the calling test again in a child whose Git commands, the example's included, read no
/// system or global configuration; returns `true` in the parent once that child passed.
fn isolated(test: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return false;
    }
    let status = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .case("*")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .nocapture()
        .status()
        .unwrap();
    assert!(status.success(), "{test}: {status}");
    true
}

/// Runs native process futures on their caller-owned local executor.
fn run(action: impl Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, action);
}

/// Literal selector request authored by the example.
fn selection() -> Value {
    json!([
        "select",
        "Restore code state?",
        ["Yes, restore code to that point", "No, keep current code"]
    ])
}

/// The example's Git commands see the disposable repository's own configuration alone.
#[test]
fn git_checkpoint_component_reads_only_repository_configuration() {
    if isolated("git_checkpoint_component_reads_only_repository_configuration") {
        return;
    }
    run(async {
        let h = Harness::new().await;
        let scopes = git(
            h.root.path(),
            &["config", "--list", "--show-scope", "--show-origin"],
        );
        eprintln!("{scopes}");
        assert!(
            scopes.lines().all(|line| line.starts_with("local\t")),
            "{scopes}"
        );
    });
}

/// Captures replace per-entry objects and restore the selected tracked-file contents.
#[test]
fn git_checkpoint_component_tracks_entries_and_applies_latest_stash() {
    if isolated("git_checkpoint_component_tracks_entries_and_applies_latest_stash") {
        return;
    }
    run(async {
        let h = Harness::new().await;
        let first = h.track().await;
        h.write("first checkpoint\n");
        h.turn().await;
        assert_eq!(
            h.text(),
            "first checkpoint\n",
            "stash create keeps working files"
        );
        h.write("replacement checkpoint\n");
        h.turn().await;
        let second = h.track().await;
        h.write("second entry\n");
        h.turn().await;
        git(h.root.path(), &["restore", "tracked.txt"]);
        assert_eq!(
            h.fork(&first, Some("Yes, restore code to that point"))
                .await,
            [
                selection(),
                json!(["notify", "Code restored to checkpoint", "info"])
            ]
        );
        assert_eq!(h.text(), "replacement checkpoint\n");
        git(h.root.path(), &["restore", "tracked.txt"]);
        assert_eq!(h.fork(&second, Some("Yes indeed")).await.len(), 2);
        assert_eq!(h.text(), "second entry\n");
        assert_eq!(
            git(h.root.path(), &["stash", "list"]),
            "",
            "create never pushes a stash entry"
        );
    });
}

/// Missing checkpoints, headless operation and nonaffirmative choices never restore files.
#[test]
fn git_checkpoint_component_requires_checkpoint_ui_and_yes() {
    if isolated("git_checkpoint_component_requires_checkpoint_ui_and_yes") {
        return;
    }
    run(async {
        let h = Harness::new().await;
        h.write("untracked entry\n");
        h.turn().await;
        let id = h.track().await;
        assert!(
            h.fork(&id, Some("Yes")).await.is_empty(),
            "no entry at capture time"
        );
        git(h.root.path(), &["restore", "tracked.txt"]);
        h.turn().await;
        assert!(
            h.fork(&id, Some("Yes")).await.is_empty(),
            "clean tree creates no checkpoint"
        );
        h.write("saved\n");
        h.turn().await;
        h.write("keep current\n");
        for choice in [None, Some("No, keep current code"), Some("yes")] {
            assert_eq!(h.fork(&id, choice).await, [selection()]);
            assert_eq!(h.text(), "keep current\n");
        }
        assert!(h.fork("missing", Some("Yes")).await.is_empty());
        h.runner.set_ui_context(None);
        assert!(h.fork(&id, Some("Yes")).await.is_empty());
        assert_eq!(h.text(), "keep current\n");
    });
}

/// Agent completion clears references while leaving the remembered entry available.
#[test]
fn git_checkpoint_component_clears_checkpoints_but_keeps_current_entry() {
    if isolated("git_checkpoint_component_clears_checkpoints_but_keeps_current_entry") {
        return;
    }
    run(async {
        let h = Harness::new().await;
        let id = h.track().await;
        h.write("saved\n");
        h.turn().await;
        let mut event = serde_json::from_value::<maestro_extensions::types::AgentEndEvent>(
            json!({"messages":[]}),
        )
        .unwrap();
        h.runner.emit(&mut event).await.unwrap();
        assert!(h.fork(&id, Some("Yes")).await.is_empty());
        h.turn().await;
        git(h.root.path(), &["restore", "tracked.txt"]);
        assert_eq!(h.fork(&id, Some("Yes")).await.len(), 2);
        assert_eq!(
            h.text(),
            "saved\n",
            "agent end clears refs, not current entry"
        );
    });
}
