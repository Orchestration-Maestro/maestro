//! The shipped read override through the production component and registered callbacks.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, ExtensionUIContext, Local, create_extension_runtime,
    load_extension_from_factory,
    types::{ExtensionUIContextPort, NotificationType},
};
use maestro_extensions_wasmtime::{Installation, artifact_factory};
use serde_json::{Value, json};
use std::{cell::RefCell, path::Path, rc::Rc, sync::RwLock};

/// Only the notifications delivered to the supplied UI.
#[derive(Default)]
struct Ui(RefCell<Vec<Value>>);
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(
        &self,
        message: String,
        kind: Option<NotificationType>,
    ) -> Result<(), maestro_extensions::EventError> {
        self.0.borrow_mut().push(json!([message, kind]));
        Ok(())
    }
}

/// Loads the single shipped component with an isolated agent directory and supplied UI.
async fn loaded(root: &Path) -> (Rc<ExtensionRunner<Local>>, Rc<Ui>) {
    let path = std::env::var("MAESTRO_TOOL_OVERRIDE_COMPONENT").unwrap();
    let bus = super::components::TracedBus::new();
    let runtime = create_extension_runtime::<Local>();
    let owner = load_extension_from_factory(
        artifact_factory(
            path.clone(),
            super::support::widgets(),
            Installation {
                version: "controlled-host".into(),
                agent_dir: root.to_str().unwrap().into(),
            },
        ),
        root.to_str().unwrap().into(),
        bus.controller.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    let runner = Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session(root.to_str().unwrap()).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ));
    let ui = Rc::new(Ui::default());
    runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
    (runner, ui)
}

/// Calls the real registered read through the host's standard tool wrapper.
async fn read(runner: &Rc<ExtensionRunner<Local>>, params: Value) -> Value {
    let registered = runner.get_all_registered_tools();
    assert_eq!(registered.len(), 1);
    let metadata = registered[0].metadata();
    assert_eq!(metadata.definition.name, "read");
    assert_eq!(metadata.label, "read (audited)");
    assert_eq!(
        serde_json::to_value(&metadata.definition.parameters).unwrap()["properties"]["offset"]["type"],
        "number"
    );
    let tool = maestro_extensions::wrap_registered_tool(&registered[0], runner.clone());
    let value = (tool.execute)("controlled-read".into(), params, None, None)
        .await
        .unwrap();
    json!({"content":value.content,"details":value.details})
}

/// The host's resolved spelling of a fixture path below `root`, with native separators.
fn native(root: &Path, path: &str) -> String {
    root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR))
        .display()
        .to_string()
}

/// Executes the registered log-display command without a prompt or provider.
async fn show_log(runner: &ExtensionRunner<Local>) {
    let command = runner.get_command("read-log").unwrap();
    (command.command.handler)(String::new(), runner.create_command_context())
        .await
        .unwrap();
}

#[test]
fn tool_override_reads_paths_and_numeric_line_ranges() {
    super::wasi_support::run(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let root = scratch.path();
        std::fs::write(root.join("allowed.txt"), "one\ntwo Ω\nthree\n").unwrap();
        let (runner, _) = loaded(root).await;
        for (params, expected) in [
            (json!({"path":"allowed.txt"}), "one\ntwo Ω\nthree\n"),
            (
                json!({"path":root.join("allowed.txt"),"offset":2,"limit":1}),
                "two Ω",
            ),
            (
                json!({"path":"sub/../allowed.txt","offset":-2,"limit":2}),
                "one\ntwo Ω",
            ),
            (
                json!({"path":"allowed.txt","offset":2.8,"limit":1.9}),
                "two Ω\nthree",
            ),
            (
                json!({"path":"allowed.txt","offset":0,"limit":0}),
                "one\ntwo Ω\nthree\n",
            ),
            (
                json!({"path":"allowed.txt","limit":-1}),
                "one\ntwo Ω\nthree",
            ),
            (json!({"path":"allowed.txt","offset":99}), ""),
        ] {
            assert_eq!(
                read(&runner, params).await,
                json!({"content":[{"type":"text","text":expected}],"details":{"lines":4}})
            );
        }
        std::fs::write(root.join("bytes.txt"), b"bad \xff\n").unwrap();
        assert_eq!(
            read(&runner, json!({"path":"bytes.txt"})).await["content"][0]["text"],
            "bad �\n"
        );
        for (contents, selected) in [
            ("x".repeat(51_200), "x".repeat(51_200)),
            (
                "雪".repeat(20_000),
                format!("{}\n\n[Output truncated at 50KB]", "雪".repeat(17_066)),
            ),
        ] {
            std::fs::write(root.join("large.txt"), contents).unwrap();
            assert_eq!(
                read(&runner, json!({"path":"large.txt"})).await["content"][0]["text"],
                selected
            );
        }
    });
}

#[test]
fn tool_override_preserves_authored_file_identity() {
    super::wasi_support::run(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let root = scratch.path();
        std::fs::write(root.join("report Q1.txt"), "plain space").unwrap();
        std::fs::write(root.join("report\u{a0}Q1.txt"), "no-break space").unwrap();
        std::fs::create_dir(root.join("~")).unwrap();
        std::fs::write(root.join("~/x"), "literal tilde").unwrap();
        let (runner, _) = loaded(root).await;
        for (path, expected) in [
            ("report\u{a0}Q1.txt", "no-break space"),
            ("report Q1.txt", "plain space"),
            ("~/x", "literal tilde"),
        ] {
            assert_eq!(
                read(&runner, json!({"path":path})).await,
                json!({"content":[{"type":"text","text":expected}],"details":{"lines":1}})
            );
            assert!(
                std::fs::read_to_string(root.join("read-access.log"))
                    .unwrap()
                    .ends_with(&format!("ALLOWED: {}\n", native(root, path)))
            );
        }
    });
}

#[test]
fn tool_override_reduces_paths_before_access_checks() {
    super::wasi_support::run(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let root = scratch.path();
        std::fs::write(root.join("public.txt"), "public").unwrap();
        let (runner, _) = loaded(root).await;
        for path in [
            root.join(".ssh/../public.txt"),
            root.join("public.txt"),
            "a/../public.txt".into(),
            "public.txt/".into(),
        ] {
            assert_eq!(
                read(&runner, json!({"path":path})).await,
                json!({"content":[{"type":"text","text":"public"}],"details":{"lines":1}})
            );
            assert!(
                std::fs::read_to_string(root.join("read-access.log"))
                    .unwrap()
                    .ends_with(&format!("ALLOWED: {}\n", root.join("public.txt").display()))
            );
        }
        let value = read(&runner, json!({"path":root.join("public/../.ssh/key")})).await;
        assert_eq!(value["details"], json!({"blocked":true}));
        assert!(
            std::fs::read_to_string(root.join("read-access.log"))
                .unwrap()
                .ends_with(&format!(
                    "BLOCKED: {} (matches blocked pattern)\n",
                    native(root, ".ssh/key")
                ))
        );
    });
}

#[test]
fn tool_override_blocks_sensitive_paths_and_records_every_attempt() {
    super::wasi_support::run(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let root = scratch.path();
        let (runner, _) = loaded(root).await;
        for path in [
            ".env",
            ".env.local",
            "appsecrets.JSON",
            "secret.yaml",
            "secrets.yml",
            "secret.toml",
            "credential.json",
            "credentials.YAML",
            ".ssh/key",
            ".aws/config",
            ".gnupg/key",
        ] {
            let value = read(&runner, json!({"path":path})).await;
            assert_eq!(value["details"], json!({"blocked":true}));
            assert_eq!(
                value["content"][0]["text"],
                format!(
                    "Access denied: \"{path}\" matches a blocked pattern (sensitive file). This tool blocks access to .env files, secrets, credentials, and SSH/AWS/GPG directories."
                )
            );
            let log = std::fs::read_to_string(root.join("read-access.log")).unwrap();
            assert!(log.ends_with(&format!(
                "BLOCKED: {} (matches blocked pattern)\n",
                native(root, path)
            )));
        }
        for path in [".ENV", ".env.", "secret.txt", ".SSH/key", "missing.txt"] {
            let value = read(&runner, json!({"path":path})).await;
            assert_eq!(value["details"], json!({"error":true}));
            assert!(
                value["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .starts_with("Error reading file: ")
            );
            assert!(
                std::fs::read_to_string(root.join("read-access.log"))
                    .unwrap()
                    .ends_with(&format!("ALLOWED: {}\n", native(root, path)))
            );
        }
        let log = std::fs::read_to_string(root.join("read-access.log")).unwrap();
        assert_eq!(log.lines().count(), 16);
        for line in log.lines() {
            assert!(line[21..24].bytes().all(|byte| byte.is_ascii_digit()));
            assert_eq!(&line[20..21], ".");
            assert_eq!(&line[24..25], "Z");
            assert_eq!(&line[25..27], "] ");
        }
        observe_read_failures(&runner, root).await;
    });
}

/// Checks OS refusal and a failed audit append without hiding an allowed read.
async fn observe_read_failures(runner: &Rc<ExtensionRunner<Local>>, root: &Path) {
    std::fs::write(root.join("unreadable.txt"), "private").unwrap();
    #[cfg(unix)]
    if let Some(_guard) = maestro_test_directory::deny(root.join("unreadable.txt"), 0).unwrap() {
        let value = read(runner, json!({"path":"unreadable.txt"})).await;
        assert_eq!(value["details"], json!({"error":true}));
    }
    std::fs::remove_file(root.join("read-access.log")).unwrap();
    std::fs::create_dir(root.join("read-access.log")).unwrap();
    assert_eq!(
        read(runner, json!({"path":"unreadable.txt"})).await["content"][0]["text"],
        "private",
        "logging failure must not refuse the read"
    );
}

#[test]
fn tool_override_displays_only_recent_access_records() {
    super::wasi_support::run(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let root = scratch.path();
        let (runner, ui) = loaded(root).await;
        show_log(&runner).await;
        assert_eq!(ui.0.take(), [json!(["No access log found", "info"])]);
        for (contents, expected) in [
            (String::new(), "Recent file access:\n".to_owned()),
            (
                " \u{feff}\nfirst\nsecond\n\u{feff} ".into(),
                "Recent file access:\nfirst\nsecond".into(),
            ),
            (
                "\u{85}kept\u{85}".into(),
                "Recent file access:\n\u{85}kept\u{85}".into(),
            ),
            (
                (1..=25)
                    .map(|n| format!("record {n}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                format!(
                    "Recent file access:\n{}",
                    (6..=25)
                        .map(|n| format!("record {n}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            ),
        ] {
            std::fs::write(root.join("read-access.log"), contents).unwrap();
            show_log(&runner).await;
            assert_eq!(ui.0.take(), [json!([expected, "info"])]);
        }
    });
}
