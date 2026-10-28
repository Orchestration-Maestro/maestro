//! Project rules through the shipped component, the real runner and a disposable directory.
#![cfg(test)]
use maestro_extensions::{
    BuildSystemPromptOptions, ExtensionRunner, ExtensionUIContext, Local, Presence,
    SessionStartEvent, SessionStartReason, create_event_bus_for, create_extension_runtime,
    load_extension_from_factory,
    types::{ExtensionUIContextPort, NotificationType},
};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::RwLock};

/// Notifications in arrival order.
#[derive(Default)]
struct Ui {
    /// Each notification's kind and text.
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
        self.calls.borrow_mut().push(json!([message, kind]));
        Ok(())
    }
}

/// One disposable working directory and the real runner over the compiled example.
struct Harness {
    /// Deletes the directory on completion.
    root: Scratch,
    /// Production runner over the compiled shipped example.
    runner: ExtensionRunner<Local>,
    /// Caller-controlled UI.
    ui: Rc<Ui>,
    /// Events and messages of handler failures the runner reported.
    errors: Rc<RefCell<Vec<(String, String)>>>,
}
impl Harness {
    /// Loads the compiled example with the working directory set to a new empty directory.
    async fn new() -> Self {
        let root = Scratch::new().unwrap();
        let cwd = root.path().to_str().unwrap();
        let runtime = create_extension_runtime::<Local>();
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let path =
            std::env::var("MAESTRO_PROJECT_RULES_COMPONENT").expect("nextest component setup");
        let owner = load_extension_from_factory(
            crate::artifact(path.clone()),
            cwd.into(),
            bus.for_extension(),
            runtime.clone(),
            Some(path),
        )
        .await
        .unwrap();
        let (session, _history) = super::session::runner_session(cwd);
        let runner = ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(vec![owner])),
            runtime,
            session,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        );
        super::support::bind_prompt(&runner, "rules test");
        let errors = Rc::new(RefCell::new(Vec::new()));
        let reported = errors.clone();
        let _remove = runner.on_error(Rc::new(move |error| {
            reported
                .borrow_mut()
                .push((error.event.clone(), error.error.clone()));
            Ok(())
        }));
        let ui = Rc::new(Ui::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        Self {
            root,
            runner,
            ui,
            errors,
        }
    }

    /// The directory the example scans.
    fn rules(&self) -> std::path::PathBuf {
        self.root.path().join(".claude").join("rules")
    }

    /// Writes one file below the rules directory, creating the folders it needs.
    fn write(&self, relative: &str, text: &str) {
        let path = self.rules().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// Starts a session through the production runner and returns the new notifications.
    async fn start(&self) -> Vec<Value> {
        self.ui.calls.borrow_mut().clear();
        let mut event = SessionStartEvent {
            reason: SessionStartReason::Startup,
            previous_session_file: Presence::Missing,
        };
        assert!(self.runner.emit(&mut event).await.unwrap().is_none());
        self.ui.calls.borrow().clone()
    }

    /// The prompt the example replaces `base` with, or `None` when it adds nothing.
    async fn prompt(&self, base: &str) -> Option<String> {
        let options: BuildSystemPromptOptions =
            serde_json::from_value(json!({"cwd": self.root.path().to_str().unwrap()})).unwrap();
        let mut images = Presence::Missing;
        self.runner
            .emit_before_agent_start("task".into(), &mut images, base.into(), options)
            .await
            .unwrap()
            .and_then(|result| result.system_prompt)
    }
}

/// Runs native process futures on their caller-owned local executor.
fn run(action: impl Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, action);
}

/// The prompt with the rule section, written out in full rather than built from the example.
fn with_rules(base: &str, rules: &[&str]) -> String {
    let list: Vec<_> = rules
        .iter()
        .map(|rule| format!("- .claude/rules/{rule}"))
        .collect();
    format!(
        "{base}\n\n## Project Rules\n\nProject rule files in .claude/rules/:\n\n{}\n\nUse the read tool to load any rule files that apply to your task for guidance.\n",
        list.join("\n")
    )
}

/// The notification a session start shows for `count` rules.
fn found(count: usize) -> Vec<Value> {
    vec![json!([
        format!("Project rules: {count} file(s) in .claude/rules/"),
        "info"
    ])]
}

/// Links are not followed: a symbolic link to a file or a folder is neither listed nor entered.
#[cfg(unix)]
fn link(harness: &Harness, target: &str, name: &str) {
    std::os::unix::fs::symlink(harness.rules().join(target), harness.rules().join(name)).unwrap();
}

/// Markdown files at any depth are listed, folder by folder in name order, whatever the order
/// the file system returns them in.
#[test]
fn project_rules_component_lists_nested_markdown_in_name_order() {
    run(async {
        let files = [
            "README.MD",
            "api-design.md",
            "frontend/components.md",
            "frontend/deep/state.md",
            "legacy.md/x.md",
            "notes.txt",
            "testing.md",
        ];
        let listed = [
            "api-design.md",
            "frontend/components.md",
            "frontend/deep/state.md",
            "legacy.md/x.md",
            "testing.md",
        ];
        for reversed in [false, true] {
            let harness = Harness::new().await;
            let mut order = files.to_vec();
            if reversed {
                order.reverse();
            }
            for name in order {
                harness.write(name, "rule\n");
            }
            #[cfg(unix)]
            {
                link(&harness, "api-design.md", "linked.md");
                link(&harness, "frontend", "linked");
            }
            assert_eq!(harness.start().await, found(listed.len()), "{reversed}");
            assert_eq!(
                harness.prompt("Base prompt").await,
                Some(with_rules("Base prompt", &listed)),
                "{reversed}"
            );
        }
    });
}

/// A missing, empty or Markdown-free folder adds nothing, and every start scans again.
#[test]
fn project_rules_component_adds_nothing_without_rules_and_rescans_on_start() {
    run(async {
        let harness = Harness::new().await;
        assert!(harness.start().await.is_empty());
        assert_eq!(harness.prompt("Base").await, None);
        harness.write("notes.txt", "not a rule\n");
        std::fs::create_dir(harness.rules().join("empty")).unwrap();
        assert!(harness.start().await.is_empty());
        assert_eq!(harness.prompt("Base").await, None);
        harness.write("one.md", "rule\n");
        assert_eq!(harness.start().await, found(1));
        assert_eq!(harness.prompt("").await, Some(with_rules("", &["one.md"])));
        std::fs::remove_file(harness.rules().join("one.md")).unwrap();
        assert!(harness.start().await.is_empty());
        assert_eq!(harness.prompt("Base").await, None);
        assert!(harness.errors.borrow().is_empty());
    });
}

/// A rules path that cannot be read is reported as a failure of the start handler.
#[test]
fn project_rules_component_reports_an_unreadable_rules_path() {
    run(async {
        let harness = Harness::new().await;
        std::fs::create_dir(harness.root.path().join(".claude")).unwrap();
        std::fs::write(harness.rules(), "a file, not a folder").unwrap();
        assert!(harness.start().await.is_empty());
        assert_eq!(harness.prompt("Base").await, None);
        let errors = harness.errors.borrow();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].0, "session_start");
        assert!(!errors[0].1.is_empty());
    });
}

/// A regular file above the rules path makes it missing and clears the previous scan.
#[test]
fn project_rules_component_clears_rules_when_parent_is_a_file() {
    run(async {
        let harness = Harness::new().await;
        harness.write("one.md", "rule\n");
        assert_eq!(harness.start().await.len(), 1);
        assert!(harness.prompt("Base").await.is_some());
        std::fs::remove_dir_all(harness.root.path().join(".claude")).unwrap();
        std::fs::write(harness.root.path().join(".claude"), "not a folder").unwrap();
        assert!(harness.start().await.is_empty());
        assert_eq!(harness.prompt("Base").await, None);
        assert!(harness.errors.borrow().is_empty());
    });
}
