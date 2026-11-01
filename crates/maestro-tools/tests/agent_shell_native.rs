//! Native shell consumption, artifacts and cleanup through the agent factories.
#![cfg(test)]
#![cfg(unix)]
use crate::agent_shell_support;
use crate::process_support;
use crate::read_support;
use crate::runtime;

use crate::runtime::run;
use agent_shell_support::{Immediate, result_value};
use maestro_tools::{BashExecResult, BashSpawnContext, BashToolOptions, create_bash_tool};
use read_support::{Fixture, Scratch};
use serde_json::json;
use std::sync::{Arc, Mutex};

/// Invoke a fresh native tool where definition retention is not under test.
async fn invoke(
    cwd: String,
    command: &str,
    options: BashToolOptions,
) -> Result<maestro_agent::AgentToolResult, maestro_models::DiagnosticErrorInfo> {
    let tool = create_bash_tool(cwd, String::new(), options);
    (tool.execute)("call".into(), json!({"command":command}), None, None).await
}

#[test]
fn bash_native_uses_factory_shell_path_and_override_precedence() {
    let dir = Scratch::new().unwrap();
    run(async {
        let error = invoke(dir.text(), "exit 7", shell_path("/usr/bin/false".into()))
            .await
            .err()
            .unwrap();
        assert_eq!(error.message, "(no output)\n\nCommand exited with code 1");
        let missing = dir
            .path()
            .join("missing-shell")
            .to_str()
            .unwrap()
            .to_owned();
        assert!(
            invoke(dir.text(), "printf wrong", shell_path(missing.clone()))
                .await
                .err()
                .unwrap()
                .message
                .contains("Custom shell path not found")
        );
        let ops = Immediate(Arc::new(|_, _, options| {
            (options.on_data)(b"supplied");
            Ok(BashExecResult {
                exit_code: Some(0.0),
            })
        }));
        let result = invoke(
            dir.text(),
            "literal",
            BashToolOptions {
                shell_path: Some(missing.clone()),
                operations: Some(Arc::new(ops)),
                ..BashToolOptions::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(
            result_value(result),
            json!({"content":[{"type":"text","text":"supplied"}]})
        );
        assert!(
            invoke(missing, "true", BashToolOptions::default())
                .await
                .err()
                .unwrap()
                .message
                .contains("Working directory does not exist")
        );
        assert!(
            invoke(dir.text(), "true", shell_path(dir.text()))
                .await
                .is_err()
        );
    });
}

/// Expected native PATH, retaining the captured value if the tool directory is present.
fn expected_native_path() -> String {
    let initial = std::env::var("PATH").unwrap_or_default();
    if initial.split(':').any(|entry| entry == "/maestro/tool-bin") {
        initial
    } else if initial.is_empty() {
        "/maestro/tool-bin".into()
    } else {
        format!("/maestro/tool-bin:{initial}")
    }
}

/// Hook replacement observed through the local operation's real environment.
fn replacement_hook(cwd: String) -> maestro_tools::BashSpawnHook {
    let initial_hook_value = std::env::var_os("MAESTRO_HOOK_ONLY");
    let expected_path = expected_native_path();
    Arc::new(move |mut context: BashSpawnContext| {
        assert_eq!(
            context.command,
            "printf original-prefix\nprintf original-command"
        );
        assert_eq!(
            context.env.get(std::ffi::OsStr::new("MAESTRO_HOOK_ONLY")),
            initial_hook_value.as_ref().map(|_| &initial_hook_value)
        );
        assert_eq!(
            context
                .env
                .get(std::ffi::OsStr::new("PATH"))
                .unwrap()
                .as_ref()
                .unwrap(),
            std::ffi::OsStr::new(&expected_path)
        );
        context.env.clear();
        context
            .env
            .insert("MAESTRO_HOOK_ONLY".into(), Some("hook env".into()));
        context.cwd.clone_from(&cwd);
        context.command = "printf '%s' \"$MAESTRO_HOOK_ONLY\"".into();
        Ok(context)
    })
}

#[test]
fn bash_native_combines_prefix_output_and_environment_hook() {
    let dir = Scratch::new().unwrap();
    run(async {
        native_prefix(&dir).await;
        let tool = create_bash_tool(
            "/missing-original-directory".into(),
            "/maestro/tool-bin".into(),
            BashToolOptions {
                command_prefix: Some("printf original-prefix".into()),
                spawn_hook: Some(replacement_hook(dir.text())),
                ..BashToolOptions::default()
            },
        );
        for _ in 0..2 {
            let result = (tool.execute)(
                "call".into(),
                json!({"command":"printf original-command"}),
                None,
                None,
            )
            .await
            .unwrap();
            assert_eq!(
                result_value(result),
                json!({"content":[{"type":"text","text":"hook env"}]})
            );
        }
        let tool = create_bash_tool(
            dir.text(),
            "/maestro/tool-bin".into(),
            BashToolOptions::default(),
        );
        let value = result_value(
            (tool.execute)(
                "call".into(),
                json!({"command":"printf '%s' \"$PATH\""}),
                None,
                None,
            )
            .await
            .unwrap(),
        );
        assert_eq!(value["content"][0]["text"], expected_native_path());
    });
}

/// Read the complete artifact immediately after native settlement.
async fn native_artifact(tool: &maestro_agent::AgentTool, status: i32) {
    let updates = Arc::new(Mutex::new(Vec::new()));
    let seen = updates.clone();
    let update = Arc::new(move |value| seen.lock().unwrap().push(result_value(value)));
    let command = format!("for ((i=1;i<=3000;i++)); do printf '%s\\n' \"$i\"; done; exit {status}");
    let result = (tool.execute)(
        "call".into(),
        json!({"command":command}),
        None,
        Some(update),
    )
    .await;
    let mut observed = match result {
        Ok(value) => json!({"value":result_value(value)}),
        Err(error) => json!({"error":error.message}),
    };
    let bytes = agent_shell_support::normalize(&mut observed, &mut updates.lock().unwrap());
    assert_eq!(bytes, [agent_shell_support::numbered_lines(3000)]);
    let text = observed
        .pointer("/value/content/0/text")
        .or_else(|| observed.get("error"))
        .unwrap()
        .as_str()
        .unwrap();
    assert!(text.contains("[Showing lines 1001-3000 of 3000. Full output: <artifact>]"));
    if status == 7 {
        assert!(text.ends_with("\n\nCommand exited with code 7"));
    }
}

#[test]
fn bash_native_returns_raw_success_and_chatty_failure() {
    let dir = Scratch::new().unwrap();
    run(async {
        for (command, expected) in [
            ("printf hello", "hello"),
            ("true", "(no output)"),
            (
                r"printf '\033[31mred\033[0m\r\000\007\n'",
                "\u{1b}[31mred\u{1b}[0m\r\0\u{7}\n",
            ),
            ("printf stderr >&2", "stderr"),
        ] {
            let result = invoke(dir.text(), command, BashToolOptions::default())
                .await
                .unwrap();
            assert_eq!(
                result_value(result),
                json!({"content":[{"type":"text","text":expected}]})
            );
        }
        let error = invoke(
            dir.text(),
            "printf partial; exit 7",
            BashToolOptions::default(),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.message, "partial\n\nCommand exited with code 7");
        let tool = create_bash_tool(dir.text(), String::new(), BashToolOptions::default());
        for status in [0, 7] {
            native_artifact(&tool, status).await;
        }
    });
}

/// Readiness output precedes the requested abort or native fractional timeout.
async fn native_cancel(cwd: String, timeout: Option<f64>) {
    let tool = create_bash_tool(cwd, String::new(), BashToolOptions::default());
    let signal = maestro_models::Cancellation::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let update = Arc::new(move |value| {
        tx.send(result_value(value)).unwrap();
    });
    let mut params = json!({"command":"printf ready; exec /bin/sleep 60"});
    if let Some(seconds) = timeout {
        params["timeout"] = json!(seconds);
    }
    let mut execution = (tool.execute)("call".into(), params, Some(signal.clone()), Some(update));
    assert_eq!(rx.recv().await.unwrap(), json!({"content":[]}));
    loop {
        let value = tokio::select! { value = rx.recv() => value.unwrap(), _ = &mut execution => panic!("shell settled before readiness output") };
        if value["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("ready")
        {
            break;
        }
    }
    if timeout.is_none() {
        signal.abort();
    }
    let error = execution.await.err().unwrap();
    assert_eq!(
        error.message,
        if timeout.is_some() {
            "ready\n\nCommand timed out after 1.5 seconds"
        } else {
            "ready\n\nCommand aborted"
        }
    );
}

#[test]
fn bash_native_timeout_and_abort_report_output_after_cleanup() {
    let dir = Scratch::new().unwrap();
    run(async {
        for timeout in [None, Some(1.5)] {
            native_cancel(dir.text(), timeout).await;
        }
    });
}

#[test]
#[cfg(target_os = "linux")]
fn bash_tool_finishes_after_inherited_pipe_grace() {
    use tokio::io::AsyncWriteExt;
    let dir = Scratch::new().unwrap();
    run(async {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let tool = create_bash_tool(dir.text(), String::new(), BashToolOptions::default());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let update = Arc::new(move |value| {
            tx.send(result_value(value)).unwrap();
        });
        let command = format!(
            "printf '%s' \"$$\"; exec 3<>/dev/tcp/127.0.0.1/{port}; read -u 3 finish; printf child-exiting"
        );
        let mut execution = (tool.execute)(
            "call".into(),
            json!({"command":command}),
            None,
            Some(update),
        );
        assert_eq!(rx.recv().await.unwrap(), json!({"content":[]}));
        let pid = loop {
            let value = tokio::select! { value = rx.recv() => value.unwrap(), _ = &mut execution => panic!("shell exited before holder readiness") };
            if let Some(text) = value["content"][0]["text"].as_str()
                && !text.is_empty()
            {
                break text.parse::<u32>().unwrap();
            }
        };
        let (mut root, _) = listener.accept().await.unwrap();
        let mut holder = read_support::pipe_holder(pid).await;
        root.write_all(b"exit\n").await.unwrap();
        let result = execution.await.unwrap();
        assert_eq!(
            result_value(result),
            json!({"content":[{"type":"text","text":format!("{pid}child-exiting")}]})
        );
        assert!(
            holder.try_wait().unwrap().is_none(),
            "holder must still own inherited stdout when result returns"
        );
        holder.start_kill().unwrap();
        assert!(!holder.wait().await.unwrap().success());
    });
}

/// Keeps the failed data callback's operation alive until explicit settlement.
struct ArtifactPending {
    release: Arc<tokio::sync::Notify>,
    settled: Arc<std::sync::atomic::AtomicBool>,
}
impl maestro_tools::BashOperations for ArtifactPending {
    fn exec<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        options: maestro_tools::BashExecOptions<'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<maestro_tools::BashExecResult, maestro_tools::BashError>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            (options.on_data)(&vec![b'A'; 51201]);
            std::fs::create_dir(std::env::temp_dir()).unwrap();
            (options.on_data)(b"ignored after failure");
            self.release.notified().await;
            self.settled
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(maestro_tools::BashExecResult {
                exit_code: Some(0.0),
            })
        })
    }
}

#[test]
fn bash_artifact_creation_failure_is_reported() {
    if !maestro_test_process::selection().is_child() {
        let dir = Scratch::new().unwrap();
        let missing = dir.path().join("missing");
        let output = maestro_test_process::TestProcess::current(
            "bash_artifact_creation_failure_is_reported",
        )
        .unwrap()
        .nocapture()
        .case("*")
        .temp_dir(&missing)
        .output()
        .unwrap();
        assert!(output.status.success());

        return;
    }
    runtime::run_paused(async {
        let release = Arc::new(tokio::sync::Notify::new());
        let settled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let tool = create_bash_tool(
            "/work".into(),
            String::new(),
            BashToolOptions {
                operations: Some(Arc::new(ArtifactPending {
                    release: release.clone(),
                    settled: settled.clone(),
                })),
                ..BashToolOptions::default()
            },
        );
        let mut execution = Box::pin((tool.execute)(
            "call".into(),
            json!({"command":"literal"}),
            None,
            None,
        ));
        assert!(
            process_support::poll_once(&mut execution.as_mut())
                .await
                .is_pending(),
            "artifact failure must not drop the pending operation"
        );
        assert!(!settled.load(std::sync::atomic::Ordering::SeqCst));
        release.notify_one();
        let error = execution
            .await
            .expect_err("artifact I/O failure must not become success");
        assert!(settled.load(std::sync::atomic::Ordering::SeqCst));
        assert!(error.code.is_some(), "original native I/O code survives");
        assert_eq!(error.name.as_deref(), Some("Error"));
    });
}

/// Exercise absent and present prefix through native execution.
async fn native_prefix(dir: &Scratch) {
    for prefix in [None, Some("printf prefix".into())] {
        let expected = if prefix.is_some() {
            "prefixcommand"
        } else {
            "command"
        };
        let result = invoke(
            dir.text(),
            "printf command",
            BashToolOptions {
                command_prefix: prefix,
                ..BashToolOptions::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(
            result_value(result),
            json!({"content":[{"type":"text","text":expected}]})
        );
    }
}

/// Select an explicit local executable for native constructor witnesses.
fn shell_path(path: String) -> BashToolOptions {
    BashToolOptions {
        shell_path: Some(path),
        ..BashToolOptions::default()
    }
}

/// Cancellation before execution never enters either backend, including observer-triggered abort.
#[test]
fn bash_native_preabort_never_starts_operations() {
    let dir = Scratch::new().unwrap();
    run(async {
        for supplied in [false, true] {
            check_native_preabort(&dir, supplied).await;
        }
    });
}

/// Both preexisting cancellation and a synchronous initial observer stop backend entry.
async fn check_native_preabort(dir: &Scratch, supplied: bool) {
    for abort_in_update in [false, true] {
        let signal = maestro_models::Cancellation::new();
        if !abort_in_update {
            signal.abort();
        }
        let entered = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called = entered.clone();
        let operations = Immediate(Arc::new(move |_, _, _| {
            called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(BashExecResult {
                exit_code: Some(0.0),
            })
        }));
        let tool = create_bash_tool(
            dir.text(),
            String::new(),
            BashToolOptions {
                operations: supplied.then(|| {
                    Arc::new(operations) as Arc<dyn maestro_tools::BashOperations + Send + Sync>
                }),
                // An aborted call must not even resolve the local shell configuration.
                shell_path: Some(dir.path().join("missing-shell").to_str().unwrap().into()),
                ..BashToolOptions::default()
            },
        );
        let selected = signal.clone();
        let result = (tool.execute)(
            "cancelled".into(),
            json!({"command":"printf must-not-run"}),
            Some(signal),
            Some(Arc::new(move |value| {
                assert!(value.content.is_empty());
                if abort_in_update {
                    selected.abort();
                }
            })),
        )
        .await;
        assert_eq!(
            result.unwrap_err().message,
            "Command aborted",
            "{supplied}/{abort_in_update}"
        );
        assert!(!entered.load(std::sync::atomic::Ordering::SeqCst));
    }
}
