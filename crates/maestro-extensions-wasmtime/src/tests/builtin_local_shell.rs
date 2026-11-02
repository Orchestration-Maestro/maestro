//! Compiled author execution through the original native local shell backend.
#![cfg(unix)]
use maestro_extensions::{ExtensionRunner, Local};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::RwLock};

/// A real component runner whose raw-output diagnostics remain externally observable.
async fn loaded() -> (ExtensionRunner<Local>, Rc<RefCell<Vec<String>>>) {
    let path = std::env::var("MAESTRO_LOCAL_SHELL_COMPONENT").unwrap();
    let runtime = maestro_extensions::create_extension_runtime::<Local>();
    runtime.set_tool_construction(maestro_extensions::ToolConstruction {
        tools_manager: std::sync::Arc::new(maestro_extensions::ToolsManager::new(
            "/unused".into(),
            None,
        )),
        bin_dir: "/unused".into(),
    });
    let errors = Rc::new(RefCell::new(Vec::new()));
    let reported = errors.clone();
    let events = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(drop),
        Rc::new(move |error| {
            reported.borrow_mut().push(error.to_string());
        }),
    );
    let owner = maestro_extensions::load_extension_from_factory(
        crate::artifact_factory(
            path.clone(),
            crate::tests::support::widgets(),
            crate::Installation {
                version: "local-shell-test".into(),
                agent_dir: "/controlled-agent".into(),
            },
        ),
        "/".into(),
        events.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session("/local-shell-caller").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    (runner, errors)
}

/// Direct execution keeps raw bytes and optional status, including observer rejection.
#[test]
fn builtin_local_shell_retains_exit_timeout_and_abort_outcomes() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let (runner, errors) = loaded().await;
        for timeout in [None, Some(0.0), Some(-1.0)] {
            for status in [0, 7] {
                let request = json!({"cwd":scratch.path(),"command":format!("exec 2>&1; printf 'out\\000\\377'; printf '\\033[31merr\\033[0m\\r\\n' >&2; exit {status}"),"timeout":timeout});
                let observed = execute(&runner, request.clone()).await;
                assert_eq!(observed, json!({"bytes":b"out\0\xff\x1b[31merr\x1b[0m\r\n","result":{"Ok":f64::from(status)}}));
                let mut rejected = request;
                rejected["reject_output"] = true.into();
                assert_eq!(execute(&runner, rejected).await, observed);
            }
        }
        assert!(!errors.borrow().is_empty());
        assert!(errors.borrow().iter().all(|error| error == "Event handler error (bash-output): raw observer rejected é\n"));
        local_failures(&runner, scratch.path()).await;
        local_cancellation(&runner, scratch.path()).await;
        local_streams_and_environment(&runner, scratch.path()).await;
        local_artifacts(&runner, scratch.path()).await;

    });
}

/// Invokes the registered author's direct shell command and decodes its complete observation.
async fn execute(runner: &ExtensionRunner<Local>, request: Value) -> Value {
    let command = runner.get_command("local-shell").unwrap().command;
    let result = (command.handler)(request.to_string(), runner.create_command_context()).await;
    serde_json::from_str(&result.unwrap_err().to_string()).unwrap()
}

/// Missing status, optional seconds and cancellation retain the native driver's outcomes.
async fn local_cancellation(runner: &ExtensionRunner<Local>, cwd: &std::path::Path) {
    let killed = execute(
        runner,
        json!({"cwd":cwd,"command":"printf killed; kill -TERM $$"}),
    )
    .await;
    assert_eq!(killed, json!({"bytes":b"killed","result":{"Ok":null}}));
    let request = json!({"cwd":cwd,"command":"printf ready; exec /bin/sleep 60","timeout":1.5});
    assert_eq!(
        execute(runner, request.clone()).await,
        json!({"bytes":b"ready","result":{"Err":"timeout:1.5"}})
    );
    for timeout in [None, Some(1.5)] {
        let signal = bind_signal(runner);
        let mut request = request.clone();
        request["signal"] = true.into();
        request["abort"] = true.into();
        request["timeout"] = json!(timeout);
        assert_eq!(
            execute(runner, request).await,
            json!({"bytes":b"ready","result":{"Err":"aborted"}})
        );
        assert!(signal.is_aborted());
    }
    let mut ready_timeout = json!({"cwd":cwd,"command":"exec /bin/sleep 60","timeout":1e-10});
    assert_eq!(
        execute(runner, ready_timeout.clone()).await,
        json!({"bytes":[],"result":{"Err":"timeout:0.0000000001"}})
    );
    let signal = bind_signal(runner);
    signal.abort();
    ready_timeout["signal"] = true.into();
    // Both abort and the positive timeout rounded to zero are ready on the first native poll.
    assert_eq!(
        execute(runner, ready_timeout).await,
        json!({"bytes":[],"result":{"Err":"aborted"}})
    );
}

/// Host-dependent spawn diagnostics are compared with the original backend, not rewritten.
async fn local_failures(runner: &ExtensionRunner<Local>, cwd: &std::path::Path) {
    for (cwd, shell) in [
        (cwd.join("missing"), None),
        (cwd.to_owned(), Some("/missing/maestro-shell".to_owned())),
    ] {
        let operations = maestro_extensions::local::create_local_bash_operations(
            "/unused".into(),
            shell.clone(),
        );
        let mut output = Vec::new();
        let mut sink = |bytes: &[u8]| output.extend_from_slice(bytes);
        let expected = operations
            .exec(
                "printf unused",
                cwd.to_str().unwrap(),
                maestro_extensions::BashExecOptions {
                    on_data: &mut sink,
                    signal: None,
                    timeout: None,
                    env: None,
                },
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(output.is_empty());
        assert_eq!(
            execute(
                runner,
                json!({"cwd":cwd,"shell":shell,"command":"printf unused"})
            )
            .await,
            json!({"bytes":[],"result":{"Err":expected}})
        );
    }
}

/// Both native pipes remain raw, and an explicitly supplied environment keeps unset entries.
async fn local_streams_and_environment(runner: &ExtensionRunner<Local>, cwd: &std::path::Path) {
    let observed = execute(
        runner,
        json!({"cwd":cwd,"command":"printf stdout; printf stderr >&2"}),
    )
    .await;
    let bytes: Vec<u8> = serde_json::from_value(observed["bytes"].clone()).unwrap();
    assert!(
        bytes == b"stdoutstderr" || bytes == b"stderrstdout",
        "{bytes:?}"
    );
    assert_eq!(observed["result"], json!({"Ok":0.0}));
    assert_eq!(
        execute(
            runner,
            json!({"cwd":cwd,"shell":"/bin/bash",
        "command":"printf '%s:%s' \"$MAESTRO_LOCAL_ENV\" \"${MAESTRO_UNSET-unset}\"",
        "env":{"MAESTRO_LOCAL_ENV":"é","MAESTRO_UNSET":null}})
        )
        .await,
        json!({"bytes":"é:unset".as_bytes(),"result":{"Ok":0.0}})
    );
}

/// Truncated native timeout and abort errors retain the complete raw output artifact.
async fn local_artifacts(runner: &ExtensionRunner<Local>, cwd: &std::path::Path) {
    for abort in [false, true] {
        let signal = bind_signal(runner);
        let observed = execute(runner, json!({"mode":"tool","cwd":cwd,"signal":abort,"abort":abort,"timeout":1.5,
            "command":"head -c 60000 /dev/zero | tr '\\0' x; printf '\\nready'; exec /bin/sleep 60"})).await;
        let error = observed["tool"]["Err"].as_str().unwrap();
        let status = if abort {
            "Command aborted"
        } else {
            "Command timed out after 1.5 seconds"
        };
        assert!(error.ends_with(status), "{error}");
        let path = error
            .split_once("Full output: ")
            .unwrap()
            .1
            .split_once(']')
            .unwrap()
            .0;
        let bytes = std::fs::read(path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(bytes, format!("{}\nready", "x".repeat(60000)).as_bytes());
        assert_eq!(signal.is_aborted(), abort);
    }
}

/// Supplies one caller-owned cancellation independently of the native execution's mirror.
fn bind_signal(runner: &ExtensionRunner<Local>) -> maestro_extensions::Cancellation {
    let signal = maestro_extensions::Cancellation::new();
    let supplied = signal.clone();
    let aborted = signal.clone();
    runner
        .bind_core(
            <Local as maestro_extensions::ExtensionExecution>::unbound_actions(),
            maestro_extensions::ExtensionContextActions {
                get_signal: Rc::new(move || Ok(Some(supplied.clone()))),
                abort: Rc::new(move || {
                    aborted.abort();
                    Ok(())
                }),
                ..Default::default()
            },
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
    signal
}
