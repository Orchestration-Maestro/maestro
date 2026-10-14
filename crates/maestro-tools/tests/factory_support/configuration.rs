//! Observable native environment, progress and pending-operation consumption.
#![cfg(test)]
use super::{execution, factories, select};
use crate::{agent_shell_support, read_support, write_support};
use maestro_models::Cancellation;
use maestro_tools::*;
use serde_json::json;
use std::sync::{Arc, atomic::Ordering};

/// Exercise operation settlement with the existing controlled clocks and readiness probes.
pub fn check() {
    for factory in factories("bash") {
        agent_shell_support::cases_with(
            &[
                "immediate_first",
                "dirty_abort",
                "signal_live_abort_success",
                "output_aborted",
            ],
            &|bash| {
                let root = read_support::TempDir::new();
                select(
                    factory,
                    "bash",
                    "/work".into(),
                    ToolsOptions {
                        bash,
                        ..ToolsOptions::default()
                    },
                    execution::manager(&root),
                )
                .unwrap()
                .into_iter()
                .find(|t| t.definition.name == "bash")
                .unwrap()
            },
        );
    }
    pending_read();
    #[cfg(unix)]
    native_shell_configuration();
}

/// Cancellation drops an admitted pending operation after its first poll.
fn pending_read() {
    for factory in factories("read") {
        let root = read_support::TempDir::new();
        let ops = Arc::new(read_support::ScriptedOps::new(b"unreachable").at(
            read_support::Stage::Read,
            read_support::Behavior::Pend,
            None,
        ));
        let options = ToolsOptions {
            read: ReadToolOptions {
                operations: Some(ops.clone()),
                ..ReadToolOptions::default()
            },
            ..ToolsOptions::default()
        };
        let tool = select(
            factory,
            "read",
            "/work".into(),
            options,
            execution::manager(&root),
        )
        .unwrap()
        .into_iter()
        .find(|t| t.definition.name == "read")
        .unwrap();
        let signal = Cancellation::new();
        let mut future = (tool.execute)(
            "call".into(),
            json!({"path":"f"}),
            Some(signal.clone()),
            None,
        );
        assert!(write_support::poll_once(&mut future).is_pending());
        assert!(ops.polled.load(Ordering::SeqCst) > 0);
        signal.abort();
        assert_eq!(
            read_support::run(future).unwrap_err().message,
            "Operation aborted"
        );
        assert_eq!(ops.dropped.load(Ordering::SeqCst), 1);
    }
}

/// Native shell selection consumes its own path and binary-directory environment.
#[cfg(unix)]
fn native_shell_configuration() {
    use crate::find_support;
    let root = read_support::TempDir::new();
    let search = read_support::TempDir::new();
    let shell = root.0.join("chosen-shell");
    find_support::executable(
        &root,
        "chosen-shell",
        "#!/bin/sh\nprintf 'chosen-shell\\n'\nexec /bin/bash \"$@\"\n",
    );
    for name in [
        "createTool",
        "createToolDefinition",
        "createCodingTools",
        "createCodingToolDefinitions",
        "createAllTools",
        "createAllToolDefinitions",
    ] {
        let options = || ToolsOptions {
            bash: BashToolOptions {
                shell_path: Some(shell.to_str().unwrap().into()),
                command_prefix: Some("printf 'prefix\\n'".into()),
                ..BashToolOptions::default()
            },
            ..ToolsOptions::default()
        };
        let manager = execution::manager(&search);
        let tools =
            super::select_with_bin(name, "bash", options(), (root.text(), root.text()), manager)
                .unwrap();
        let bash = tools
            .into_iter()
            .find(|t| t.definition.name == "bash")
            .unwrap();
        let result =
            execution::execute(&bash, json!({"command":"printf '%s' \"$PATH\""}), None).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("chosen-shell\nprefix\n"));
        let path = text.strip_prefix("chosen-shell\nprefix\n").unwrap();
        assert!(path.split(':').any(|entry| entry == root.text()));
        assert!(!path.split(':').any(|entry| entry == search.text()));
    }
}
