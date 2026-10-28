//! Shipped SSH operations through the component runner and original coding tools.
#![cfg(test)]
use maestro_app::{ExtensionRunner, Local};
use serde_json::{Value, json};
use std::{path::Path, rc::Rc};

mod resolution_failure;

/// Runs on the same local executor the application's native resources retain.
fn run<F: Future<Output = ()>>(action: impl FnOnce(Rc<tokio::task::LocalSet>) -> F) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, action(local.clone()));
}

/// Loads the shipped example through the existing embedding composition.
async fn loaded(
    root: &super::support::Root,
    local: Rc<tokio::task::LocalSet>,
) -> Rc<maestro_app::AgentSession> {
    super::support::session::component_session(
        root,
        local,
        std::env::var("MAESTRO_SSH_COMPONENT").unwrap(),
        None,
        super::support::widgets(),
    )
    .await
}

/// Invokes the registered override through the application's ordinary wrapper.
async fn call(runner: &Rc<ExtensionRunner<Local>>, name: &str, params: Value) -> Value {
    let tools = runner.get_all_registered_tools();
    let registered = tools
        .iter()
        .find(|tool| tool.metadata().definition.name == name)
        .unwrap();
    let tool = maestro_app::wrap_registered_tool(registered, runner.clone());
    let result = (tool.execute)("ssh-call".into(), params, None, None)
        .await
        .unwrap();
    serde_json::to_value(result.content).unwrap()
}

#[test]
fn ssh_unset_keeps_original_local_coding_tools() {
    run(|executor| async move {
        let root = super::support::Root::new();
        let session = loaded(&root, executor).await;
        let runner = session.extension_runner();
        assert_eq!(runner.get_all_registered_tools().len(), 4);
        assert!(runner.get_flags().contains_key("ssh"));
        call(
            &runner,
            "write",
            json!({"path":"note.txt","content":"before\n"}),
        )
        .await;
        assert_eq!(
            call(&runner, "read", json!({"path":"note.txt"})).await[0]["text"],
            "before\n"
        );
        call(
            &runner,
            "edit",
            json!({"path":"note.txt","edits":[{"oldText":"before","newText":"after"}]}),
        )
        .await;
        assert_eq!(
            std::fs::read_to_string(root.path("project/note.txt")).unwrap(),
            "after\n"
        );
        assert_eq!(
            call(&runner, "bash", json!({"command":"printf local"})).await[0]["text"],
            "local"
        );
    });
}

/// A checked child replaces only SSH; its remote shell works in disposable files.
#[cfg(unix)]
fn isolated(name: &str) -> bool {
    use std::{collections::BTreeMap, os::unix::fs::PermissionsExt as _};
    if maestro_test_process::selection().is_child() {
        return false;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    std::fs::create_dir(root.path().join("remote")).unwrap();
    let ssh = root.path().join("ssh");
    std::fs::write(
        &ssh,
        r"#!/usr/bin/python3
import json, os, sys
from pathlib import Path
root = Path(__file__).parent
with (root / 'calls.jsonl').open('a') as log:
    log.write(json.dumps(sys.argv[1:]) + '\n')
assert sys.argv[1] == '--', sys.argv
if sys.argv[2] == 'user@broken':
    sys.stderr.write('controlled connection failure')
    sys.exit(37)
if sys.argv[3].startswith('cat > ') and 'early-write.txt' in sys.argv[3]:
    sys.stderr.write('controlled remote write rejection')
    sys.exit(38)
assert sys.argv[2] == 'user@controlled', sys.argv
os.chdir(root / 'remote')
os.execv('/bin/bash', ['bash', '-c', sys.argv[3]])
",
    )
    .unwrap();
    std::fs::set_permissions(ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let environment = BTreeMap::from([
        ("PATH", format!("{}:/usr/bin:/bin", root.path().display())),
        (
            "MAESTRO_SSH_COMPONENT",
            std::env::var("MAESTRO_SSH_COMPONENT").unwrap(),
        ),
    ]);
    maestro_test_process::run_per_environment(name, [()], |()| environment.clone())
        .unwrap()
        .is_none()
}

/// The fixture root is the first directory of the checked child's supplied PATH.
#[cfg(unix)]
fn fixture(name: &str) -> std::path::PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .next()
        .unwrap()
        .join(name)
}

/// Records lifecycle UI requests without requiring a terminal.
#[derive(Default)]
struct Ui(std::cell::RefCell<Vec<Value>>);
impl maestro_app::ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        let theme = maestro_theme::load_theme_from_path(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../maestro-theme/assets/theme/dark.json"
            ),
            Some(maestro_theme::ColorMode::Truecolor),
        )
        .unwrap();
        maestro_theme::LiveTheme::from_theme(Rc::new(theme))
    }
    fn set_status(&self, key: String, text: Option<String>) -> Result<(), maestro_app::EventError> {
        self.0.borrow_mut().push(json!(["status", key, text]));
        Ok(())
    }
    fn notify(
        &self,
        text: String,
        kind: Option<maestro_app::NotificationType>,
    ) -> Result<(), maestro_app::EventError> {
        self.0.borrow_mut().push(json!(["notify", text, kind]));
        Ok(())
    }
}

/// Resolves the flag only when the real session-start callback runs.
async fn start(runner: &ExtensionRunner<Local>, value: &str) {
    runner.set_flag_value("ssh".into(), maestro_app::FlagValue::String(value.into()));
    runner
        .emit(
            &mut serde_json::from_value::<maestro_app::SessionStartEvent>(json!({"reason":"new"}))
                .unwrap(),
        )
        .await
        .unwrap();
}

/// Exercises original write/edit/read semantics over literal paths and raw input pipes.
#[cfg(unix)]
async fn remote_files(runner: &Rc<ExtensionRunner<Local>>, local: &Path) {
    let name = "note'$(touch ESCAPED).txt";
    let content = "remote Ω\n\0end\n";
    call(runner, "write", json!({"path":name,"content":content})).await;
    assert_eq!(
        std::fs::read(fixture("remote").join(name)).unwrap(),
        content.as_bytes()
    );
    assert!(!local.join(name).exists());
    assert!(!fixture("remote/ESCAPED").exists());
    assert_eq!(
        call(runner, "read", json!({"path":name})).await[0]["text"],
        content
    );
    call(
        runner,
        "edit",
        json!({"path":name,"edits":[{"oldText":"remote","newText":"edited"}]}),
    )
    .await;
    assert_eq!(
        std::fs::read_to_string(fixture("remote").join(name)).unwrap(),
        "edited Ω\n\0end\n"
    );
    let large = "雪".repeat(70_000);
    call(
        runner,
        "write",
        json!({"path":"nested/large.txt","content":large}),
    )
    .await;
    assert_eq!(
        std::fs::read_to_string(fixture("remote/nested/large.txt")).unwrap(),
        large
    );
    std::fs::write(fixture("remote/raw.txt"), b"raw \xff\n").unwrap();
    assert_eq!(
        call(runner, "read", json!({"path":"raw.txt"})).await[0]["text"],
        "raw �\n"
    );
    let tools = runner.get_all_registered_tools();
    let read = tools
        .iter()
        .find(|tool| tool.metadata().definition.name == "read")
        .unwrap();
    let tool = maestro_app::wrap_registered_tool(read, runner.clone());
    let error = (tool.execute)("missing".into(), json!({"path":"absent.txt"}), None, None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("SSH failed (1)"), "{error}");
}

/// User shell operations retain streaming, numeric exit, cancellation and source deadlines.
async fn remote_shell(runner: &Rc<ExtensionRunner<Local>>, cwd: &Path) {
    use maestro_app::{BashExecOptions, BashOperations, BashResult, UserBashEvent};
    assert_eq!(
        call(runner, "bash", json!({"command":"printf shell"})).await[0]["text"],
        "shell"
    );
    let mut event = UserBashEvent {
        command: "ignored".into(),
        cwd: cwd.to_str().unwrap().into(),
        exclude_from_context: false,
    };
    let operations = runner
        .emit_user_bash::<Rc<dyn BashOperations<Local>>, BashResult>(&mut event)
        .await
        .unwrap()
        .unwrap()
        .operations
        .unwrap();
    let mut bytes = Vec::new();
    let mut sink = |chunk: &[u8]| bytes.extend_from_slice(chunk);
    let result = operations
        .exec(
            "printf out; printf err >&2; exit 7",
            cwd.to_str().unwrap(),
            BashExecOptions {
                on_data: &mut sink,
                signal: None,
                timeout: None,
                env: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(result.exit_code, Some(7.0));
    assert!(bytes.windows(3).any(|text| text == b"out"));
    assert!(bytes.windows(3).any(|text| text == b"err"));
    shell_interruptions(operations.as_ref(), cwd.to_str().unwrap()).await;
    #[cfg(unix)]
    remote_only_shell(operations.as_ref(), cwd).await;
}

/// Runs a remote-only directory without making it a local launch prerequisite.
#[cfg(unix)]
async fn remote_only_shell(operations: &dyn maestro_app::BashOperations<Local>, cwd: &Path) {
    let local = cwd.join("remote-only");
    let remote = fixture("remote/remote-only");
    assert!(!local.exists());
    std::fs::create_dir(&remote).unwrap();
    let mut bytes = Vec::new();
    let mut sink = |chunk: &[u8]| bytes.extend_from_slice(chunk);
    let result = operations
        .exec(
            "pwd",
            local.to_str().unwrap(),
            maestro_app::BashExecOptions {
                on_data: &mut sink,
                signal: None,
                timeout: None,
                env: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(result.exit_code, Some(0.0));
    assert_eq!(
        String::from_utf8(bytes).unwrap().trim(),
        remote.to_str().unwrap()
    );
    assert!(!local.exists());
}

/// Cancels a live child on its output and supplies an immediately ready native deadline.
async fn shell_interruptions(operations: &dyn maestro_app::BashOperations<Local>, cwd: &str) {
    use maestro_app::{BashExecOptions, Cancellation};
    let signal = Cancellation::new();
    let mut abort = |_: &[u8]| signal.abort();
    let error = operations
        .exec(
            "printf ready; exec /bin/sleep 60",
            cwd,
            BashExecOptions {
                on_data: &mut abort,
                signal: Some(&signal),
                timeout: None,
                env: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "aborted");
    let mut sink = |_: &[u8]| {};
    let error = operations
        .exec(
            "true",
            cwd,
            BashExecOptions {
                on_data: &mut sink,
                signal: None,
                timeout: Some(1e-10),
                env: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "timeout:0.0000000001");
}

/// Checks the exact session notification and the themed status content.
fn assert_status(ui: &Ui, remote: &Path) {
    let observations = ui.0.borrow();
    assert_eq!(observations[0][1], "ssh");
    assert!(
        observations[0][2]
            .as_str()
            .unwrap()
            .contains(&format!("SSH: user@controlled:{}", remote.display()))
    );
    assert_eq!(
        observations[1],
        json!([
            "notify",
            format!("SSH mode: user@controlled:{}", remote.display()),
            "info"
        ])
    );
}

/// The assembled prompt changes only its first matching working-directory line.
async fn remote_prompt(runner: &ExtensionRunner<Local>, local: &Path, remote: &Path) {
    let original = format!("Current working directory: {}", local.display());
    let prompt = format!("{original}\n{original}");
    let options = serde_json::from_value(
        json!({"cwd":local,"selectedTools":[],"customPrompt":"","contextFiles":[],"skills":[]}),
    )
    .unwrap();
    let result = runner
        .emit_before_agent_start(
            "test".into(),
            &mut maestro_app::Presence::Missing,
            prompt,
            options,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result.system_prompt.unwrap(),
        format!(
            "Current working directory: {} (via SSH: user@controlled)\n{original}",
            remote.display()
        )
    );
}

#[cfg(unix)]
#[test]
fn ssh_enabled_routes_coding_tools_shell_and_session_context() {
    if isolated("ssh_enabled_routes_coding_tools_shell_and_session_context") {
        return;
    }
    run(|executor| async move {
        let root = super::support::Root::new();
        let session = loaded(&root, executor).await;
        let runner = session.extension_runner();
        let local = root.path("project");
        let ui = Rc::new(Ui::default());
        session
            .bind_extensions(maestro_app::ExtensionBindings {
                ui_context: Some(maestro_app::ExtensionUIContext::new(ui.clone()).into()),
                on_error: Some(Rc::new(|error| {
                    panic!("unexpected SSH callback error: {error:?}")
                })),
                ..maestro_app::ExtensionBindings::default()
            })
            .await
            .unwrap();
        let remote = fixture("remote");
        start(&runner, &format!("user@controlled:{}", remote.display())).await;
        remote_files(&runner, &local).await;
        remote_shell(&runner, &local).await;
        remote_prompt(&runner, &local, &remote).await;
        assert_status(&ui, &remote);
        start(&runner, "user@controlled").await;
        remote_prompt(&runner, &local, &remote).await;
        start(&runner, "user@broken:/unreachable").await;
        let tools = runner.get_all_registered_tools();
        let read = tools
            .iter()
            .find(|tool| tool.metadata().definition.name == "read")
            .unwrap();
        let tool = maestro_app::wrap_registered_tool(read, runner.clone());
        let error = (tool.execute)("error".into(), json!({"path":"note.txt"}), None, None)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("SSH failed (37): controlled connection failure"),
            "{error}"
        );
        start(&runner, "").await;
        call(
            &runner,
            "write",
            json!({"path":"local.txt","content":"local again"}),
        )
        .await;
        assert_eq!(
            std::fs::read_to_string(local.join("local.txt")).unwrap(),
            "local again"
        );
    });
}

/// A remote child that never consumes a large write retains its own diagnostic.
#[cfg(unix)]
#[test]
fn ssh_early_write_failure_retains_remote_exit_diagnostic() {
    if isolated("ssh_early_write_failure_retains_remote_exit_diagnostic") {
        return;
    }
    run(|executor| async move {
        let root = super::support::Root::new();
        let session = loaded(&root, executor).await;
        let runner = session.extension_runner();
        session
            .bind_extensions(maestro_app::ExtensionBindings {
                ui_context: Some(
                    maestro_app::ExtensionUIContext::new(Rc::new(Ui::default())).into(),
                ),
                ..maestro_app::ExtensionBindings::default()
            })
            .await
            .unwrap();
        start(
            &runner,
            &format!("user@controlled:{}", fixture("remote").display()),
        )
        .await;
        let tools = runner.get_all_registered_tools();
        let write = tools
            .iter()
            .find(|tool| tool.metadata().definition.name == "write")
            .unwrap();
        let tool = maestro_app::wrap_registered_tool(write, runner);
        let error = (tool.execute)(
            "early-write".into(),
            json!({"path":"early-write.txt","content":"x".repeat(2_000_000)}),
            None,
            None,
        )
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("SSH failed (38): controlled remote write rejection"),
            "{error}"
        );
    });
}
