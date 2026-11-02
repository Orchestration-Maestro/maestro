//! Direct shell policy through supplied and local operations.
#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]
use maestro_models::Cancellation;
use maestro_tools::{
    BashError, BashExecOptions, BashExecResult, BashExecutorOptions, BashOperations,
    execute_bash_with_operations,
};
use serde::Deserialize;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    #[serde(rename = "case")]
    label: String,
    test: String,
    input: Input,
    expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    chunks_hex: Vec<String>,
    exit_code: Option<f64>,
    signal: String,
    settle: String,
    #[serde(default)]
    no_options: bool,
    #[serde(default)]
    no_callback: bool,
    command: Option<String>,
    cwd: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    chunks: Vec<String>,
    result: Option<serde_json::Value>,
    error: Option<ExpectedError>,
    artifacts: Vec<Artifact>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedError {
    message: String,
    same: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    path: String,
    content: String,
}
struct Supplied<'a> {
    input: &'a Input,
    chunks: Arc<Mutex<Vec<String>>>,
    expected_chunks: &'a [String],
    signal: Option<&'a Cancellation>,
    marker: Arc<std::io::Error>,
}
impl BashOperations for Supplied<'_> {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        mut options: BashExecOptions<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<BashExecResult, BashError>> + Send + 'a>> {
        Box::pin(async move { self.produce(command, cwd, &mut options) })
    }
}
impl Supplied<'_> {
    fn produce(
        &self,
        command: &str,
        cwd: &str,
        options: &mut BashExecOptions<'_>,
    ) -> Result<BashExecResult, BashError> {
        assert_eq!(command, self.input.command.as_deref().unwrap_or("command"));
        assert_eq!(cwd, self.input.cwd.as_deref().unwrap_or("cwd"));
        assert!(options.timeout.is_none());
        assert!(options.env.is_none());
        assert_eq!(
            options.signal.map(std::ptr::from_ref),
            self.signal.map(std::ptr::from_ref)
        );
        for (index, hex) in self.input.chunks_hex.iter().enumerate() {
            let bytes: Vec<_> = hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            (options.on_data)(&bytes);
            if !self.input.no_options && !self.input.no_callback {
                assert_eq!(*self.chunks.lock().unwrap(), self.expected_chunks[..=index]);
            }
        }
        if self.input.signal == "late" {
            self.signal.unwrap().abort();
        }
        if self.input.settle == "error" {
            Err(BashError::Other(Box::new(self.marker.clone())))
        } else {
            Ok(BashExecResult {
                exit_code: self.input.exit_code,
            })
        }
    }
}
use crate::runtime::run;

fn corpus(name: &str) {
    if !maestro_test_process::selection().is_child() {
        let dir = read_support::Scratch::new().unwrap();
        read_support::isolated_temp_dir(name, "direct-corpus", dir.path());
        return;
    }
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/direct_shell.json")).unwrap();
    let mut count = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        count += 1;
        replay(case);
    }
    assert!(count > 0);
}
fn replay(case: Case) {
    let signal = Cancellation::new();
    if case.input.signal == "pre" {
        signal.abort();
    }
    let signal = (case.input.signal != "none").then_some(&signal);
    let chunks = Arc::new(Mutex::new(Vec::new()));
    let mut sink = |text: &str| chunks.lock().unwrap().push(text.to_owned());
    let marker = Arc::new(std::io::Error::other("operation-marker"));
    let operations = Supplied {
        input: &case.input,
        chunks: chunks.clone(),
        expected_chunks: &case.expected.chunks,
        signal,
        marker: marker.clone(),
    };
    let options = if case.input.no_options {
        BashExecutorOptions::default()
    } else {
        BashExecutorOptions {
            signal,
            on_chunk: (!case.input.no_callback).then_some(&mut sink),
        }
    };
    let result = run(execute_bash_with_operations(
        case.input.command.as_deref().unwrap_or("command"),
        case.input.cwd.as_deref().unwrap_or("cwd"),
        &operations,
        options,
    ));
    assert_eq!(
        *chunks.lock().unwrap(),
        case.expected.chunks,
        "{}",
        case.label
    );
    observe(case, result, &marker);
}
fn observe(
    case: Case,
    result: Result<maestro_tools::BashResult, BashError>,
    marker: &Arc<std::io::Error>,
) {
    match result {
        Ok(result) => {
            observe_success(case, &result);
        }
        Err(BashError::Other(error)) => {
            observe_error(case, error.as_ref(), marker);
        }
        Err(error) => panic!("unexpected error category: {error}"),
    }
}

fn observe_success(case: Case, result: &maestro_tools::BashResult) {
    let mut observed = serde_json::to_value(result).unwrap();
    if let Some(path) = &result.full_output_path {
        assert_eq!(case.expected.artifacts.len(), 1);
        let artifact = &case.expected.artifacts[0];
        assert_eq!(artifact.path, "<artifact-0>");
        assert_eq!(std::fs::read_to_string(path).unwrap(), artifact.content);
        std::fs::remove_file(path).unwrap();
        observed["fullOutputPath"] = "<artifact-0>".into();
    } else {
        assert!(case.expected.artifacts.is_empty());
    }
    assert!(case.expected.error.is_none());
    let expected = case.expected.result.unwrap();
    assert_eq!(
        observed.get("exitCode").and_then(serde_json::Value::as_f64),
        expected.get("exitCode").and_then(serde_json::Value::as_f64)
    );
    if let Some(status) = expected.get("exitCode") {
        observed["exitCode"] = status.clone();
    }
    assert_eq!(observed, expected, "{}", case.label);
}
fn observe_error(
    case: Case,
    error: &(dyn std::error::Error + Send + Sync + 'static),
    marker: &Arc<std::io::Error>,
) {
    let expected = case.expected.error.unwrap();
    assert_eq!(error.to_string(), expected.message);
    assert!(expected.same);
    assert!(Arc::ptr_eq(
        error.downcast_ref::<Arc<std::io::Error>>().unwrap(),
        marker
    ));
    assert!(case.expected.result.is_none());
    let paths: Vec<_> = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(paths.len(), case.expected.artifacts.len(), "{}", case.label);
    for (path, artifact) in paths.iter().zip(&case.expected.artifacts) {
        assert_eq!(artifact.path, "<artifact-0>");
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("maestro-bash-")
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), artifact.content);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn direct_empty_output_is_not_a_tool_placeholder() {
    corpus("direct_empty_output_is_not_a_tool_placeholder");
}

#[test]
fn direct_chunk_callbacks_keep_empty_and_arrival_order() {
    corpus("direct_chunk_callbacks_keep_empty_and_arrival_order");
}

#[test]
fn direct_optional_callback_and_signal_are_independent() {
    corpus("direct_optional_callback_and_signal_are_independent");
}

#[test]
fn direct_forwards_only_command_cwd_data_and_signal() {
    corpus("direct_forwards_only_command_cwd_data_and_signal");
}

#[test]
fn direct_statuses_are_not_tool_errors() {
    corpus("direct_statuses_are_not_tool_errors");
}

#[test]
fn direct_cancellation_uses_signal_state_not_error_spelling() {
    corpus("direct_cancellation_uses_signal_state_not_error_spelling");
}

#[test]
fn direct_sanitizes_ansi_binary_and_carriage_returns() {
    corpus("direct_sanitizes_ansi_binary_and_carriage_returns");
}

#[test]
fn direct_escape_sequences_survive_chunk_boundaries() {
    corpus("direct_escape_sequences_survive_chunk_boundaries");
}

#[test]
fn direct_utf8_survives_every_multibyte_split() {
    corpus("direct_utf8_survives_every_multibyte_split");
}

#[test]
fn direct_drops_only_the_first_decoded_bom() {
    corpus("direct_drops_only_the_first_decoded_bom");
}

#[test]
fn direct_finalizes_utf8_on_success_and_cancel() {
    corpus("direct_finalizes_utf8_on_success_and_cancel");
}

#[test]
fn direct_byte_spill_starts_only_after_raw_limit() {
    corpus("direct_byte_spill_starts_only_after_raw_limit");
}

#[test]
fn direct_raw_spill_can_return_untruncated_clean_output() {
    corpus("direct_raw_spill_can_return_untruncated_clean_output");
}

#[test]
fn direct_line_only_truncation_persists_complete_sanitized_output() {
    corpus("direct_line_only_truncation_persists_complete_sanitized_output");
}

#[test]
fn direct_rolling_window_evicts_whole_chunks_at_unit_target() {
    corpus("direct_rolling_window_evicts_whole_chunks_at_unit_target");
}

#[test]
fn direct_rolling_window_keeps_a_sole_oversized_chunk() {
    corpus("direct_rolling_window_keeps_a_sole_oversized_chunk");
}

#[test]
fn direct_rolling_window_counts_utf16_units() {
    corpus("direct_rolling_window_counts_utf16_units");
}

#[test]
fn direct_spill_replays_prefix_once_in_sanitized_order() {
    corpus("direct_spill_replays_prefix_once_in_sanitized_order");
}

#[test]
fn direct_unexpected_failure_closes_artifact_and_keeps_error() {
    corpus("direct_unexpected_failure_closes_artifact_and_keeps_error");
}

#[test]
fn direct_unexpected_failure_does_not_create_final_line_artifact() {
    corpus("direct_unexpected_failure_does_not_create_final_line_artifact");
}

#[test]
fn direct_tail_selection_reuses_shared_limits() {
    corpus("direct_tail_selection_reuses_shared_limits");
}

#[test]
fn direct_unexpected_failure_does_not_finalize_decoder() {
    corpus("direct_unexpected_failure_does_not_finalize_decoder");
}

#[test]
fn direct_final_flush_can_cause_truncation_and_spill() {
    corpus("direct_final_flush_can_cause_truncation_and_spill");
}

#[test]
fn direct_finalizes_incomplete_escapes_like_whole_text() {
    corpus("direct_finalizes_incomplete_escapes_like_whole_text");
}

use crate::read_support;

#[test]
#[cfg(unix)]
fn direct_local_bash_returns_sanitized_output() {
    run(async {
        let operations =
            maestro_tools::create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let result = execute_bash_with_operations(
            r"printf '\033[31mred\033[0m\r\n'",
            "/",
            operations.as_ref(),
            BashExecutorOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(result.output, "red\n");
        assert_eq!(result.exit_code, Some(0.0));
        assert!(!result.cancelled);
        assert!(!result.truncated);
        assert!(result.full_output_path.is_none());
    });
}

#[test]
#[cfg(unix)]
fn direct_local_line_overflow_writes_complete_artifact() {
    run(async {
        let operations =
            maestro_tools::create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let result = execute_bash_with_operations(
            "seq 1 3000",
            "/",
            operations.as_ref(),
            BashExecutorOptions::default(),
        )
        .await
        .unwrap();
        let path = result.full_output_path.unwrap();
        let full = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            full,
            (1..=3000).fold(String::new(), |mut text, number| {
                use std::fmt::Write;
                writeln!(text, "{number}").unwrap();
                text
            })
        );
        assert!(result.output.starts_with("1001\n"));
        assert!(result.output.ends_with("3000"));
        assert!(result.truncated);
        assert_eq!(result.exit_code, Some(0.0));
        assert!(!result.cancelled);
        crate::agent_shell_support::assert_output_file_name(&path);
    });
}

#[test]
#[cfg(unix)]
fn direct_local_abort_retains_sanitized_prefix() {
    run(async {
        let operations =
            maestro_tools::create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let signal = Cancellation::new();
        let mut prefix = String::new();
        let mut sink = |text: &str| {
            prefix.push_str(text);
            if prefix.contains("ready") {
                signal.abort();
            }
        };
        let result = execute_bash_with_operations(
            r"printf '\033[31mready\033[0m'; exec /bin/sleep 60",
            "/",
            operations.as_ref(),
            BashExecutorOptions {
                signal: Some(&signal),
                on_chunk: Some(&mut sink),
            },
        )
        .await
        .unwrap();
        assert_eq!(prefix, "ready");
        assert_eq!(result.output, "ready");
        assert!(result.cancelled);
        assert_eq!(result.exit_code, None);
    });
}

#[test]
#[cfg(target_os = "linux")]
fn maestro_direct_bash_finishes_after_inherited_pipe_grace() {
    use tokio::io::AsyncWriteExt;
    run(async {
        let dir = read_support::Scratch::new().unwrap();
        let gate = dir.path().join("exit");
        let command = format!(
            "printf '%s\\n' \"$$\"; while [ ! -f '{}' ]; do :; done; printf child-exiting",
            gate.display()
        );
        let operations =
            maestro_tools::create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut sink = |text: &str| {
            sender.send(text.to_owned()).unwrap();
        };
        let operation = execute_bash_with_operations(
            &command,
            "/",
            operations.as_ref(),
            BashExecutorOptions {
                signal: None,
                on_chunk: Some(&mut sink),
            },
        );
        let setup = async {
            let mut prefix = String::new();
            while !prefix.contains('\n') {
                prefix.push_str(&receiver.recv().await.unwrap());
            }
            let pid = prefix.trim().parse().unwrap();
            let mut holder = read_support::pipe_holder(pid).await;
            holder
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"emit\n")
                .await
                .unwrap();
            while !prefix.ends_with("grace") {
                prefix.push_str(&receiver.recv().await.unwrap());
            }
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&gate)
                .unwrap();
            (holder, prefix)
        };
        let (result, (mut holder, prefix)) = tokio::join!(operation, setup);
        let held = holder.try_wait().unwrap().is_none();
        holder.stdin.take();
        let reaped = holder.wait().await.unwrap();
        let result = result.unwrap();
        assert!(held);
        assert!(reaped.success());
        assert_eq!(result.output, format!("{prefix}child-exiting"));
        assert_eq!(result.exit_code, Some(0.0));
        assert!(!result.cancelled);
    });
}

/// Cleans the detached Windows descendant even when result assertions fail.
#[cfg(windows)]
struct WindowsDescendant {
    pid_file: std::path::PathBuf,
    signal: Cancellation,
}
#[cfg(windows)]
impl Drop for WindowsDescendant {
    fn drop(&mut self) {
        self.signal.abort();
        if let Ok(text) = std::fs::read_to_string(&self.pid_file)
            && let Ok(pid) = text.trim().parse::<u32>()
            && pid > 0
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .output();
        }
    }
}

#[test]
#[cfg(windows)]
fn maestro_direct_bash_finishes_after_inherited_pipe_grace() {
    use crate::read_support::Fixture;
    run(async {
        let dir = read_support::Scratch::new().unwrap();
        let guard = WindowsDescendant {
            pid_file: dir.path().join("grandchild.pid"),
            signal: Cancellation::new(),
        };
        let operand = guard
            .pid_file
            .to_str()
            .unwrap()
            .replace('\\', "/")
            .replace('\'', "'\"'\"'");
        let command = format!(
            r#"node -e "const fs=require('fs');const {{spawn}}=require('child_process');const child=spawn(process.execPath,['-e','setTimeout(()=>{{}},60000)'],{{stdio:'inherit',detached:true}});fs.writeFileSync(process.argv[1],String(child.pid));child.unref();console.log('child-exiting');" '{operand}'"#
        );
        let operations = maestro_tools::create_local_bash_operations(dir.text(), None);
        let result = execute_bash_with_operations(
            &command,
            &dir.text(),
            operations.as_ref(),
            BashExecutorOptions {
                signal: Some(&guard.signal),
                on_chunk: None,
            },
        )
        .await;
        let pid = std::fs::read_to_string(&guard.pid_file)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        assert!(pid > 0);
        let alive = std::process::Command::new("node")
            .args([
                "-e",
                "process.kill(Number(process.argv[1]), 0)",
                &pid.to_string(),
            ])
            .status()
            .unwrap();
        assert!(alive.success(), "pipe-holding descendant must remain alive");
        drop(guard);
        let result = result.unwrap();
        assert!(result.output.contains("child-exiting"));
        assert_eq!(result.exit_code, Some(0.0));
        assert!(!result.cancelled);
    });
}

#[test]
fn direct_long_osc_payload_is_held_across_a_few_chunks() {
    let payload = "title".repeat(1600);
    let raw = format!("\u{1b}]0;{payload}\u{7}ok");
    let input = Input {
        chunks_hex: [&raw[..3000], &raw[3000..6000], &raw[6000..]]
            .into_iter()
            .map(|part| {
                part.bytes().fold(String::new(), |mut text, byte| {
                    use std::fmt::Write;
                    write!(text, "{byte:02x}").unwrap();
                    text
                })
            })
            .collect(),
        exit_code: Some(0.0),
        signal: "none".into(),
        settle: "ok".into(),
        no_options: false,
        no_callback: false,
        command: None,
        cwd: None,
    };
    let expected = vec![String::new(), String::new(), "ok".into()];
    let chunks = Arc::new(Mutex::new(Vec::new()));
    let mut sink = |text: &str| {
        chunks.lock().unwrap().push(text.to_owned());
    };
    let operations = Supplied {
        input: &input,
        chunks: chunks.clone(),
        expected_chunks: &expected,
        signal: None,
        marker: Arc::new(std::io::Error::other("operation-marker")),
    };
    let result = run(execute_bash_with_operations(
        "command",
        "cwd",
        &operations,
        BashExecutorOptions {
            signal: None,
            on_chunk: Some(&mut sink),
        },
    ))
    .unwrap();
    assert_eq!(*chunks.lock().unwrap(), expected);
    assert_eq!(result.output, "ok");
    assert!(!result.truncated);
    assert!(result.full_output_path.is_none());
}

fn cancellation_from_final_chunk(hex: &str, final_text: &str) {
    let signal = Cancellation::new();
    let input = Input {
        chunks_hex: vec![hex.into()],
        exit_code: Some(7.0),
        signal: "live".into(),
        settle: "ok".into(),
        no_options: false,
        no_callback: false,
        command: None,
        cwd: None,
    };
    let chunks = Arc::new(Mutex::new(Vec::new()));
    let mut sink = |text: &str| {
        chunks.lock().unwrap().push(text.to_owned());
        if text == final_text {
            signal.abort();
        }
    };
    let operations = Supplied {
        input: &input,
        chunks: chunks.clone(),
        expected_chunks: &[String::new()],
        signal: Some(&signal),
        marker: Arc::new(std::io::Error::other("operation-marker")),
    };
    let result = run(execute_bash_with_operations(
        "command",
        "cwd",
        &operations,
        BashExecutorOptions {
            signal: Some(&signal),
            on_chunk: Some(&mut sink),
        },
    ))
    .unwrap();
    assert_eq!(*chunks.lock().unwrap(), ["", final_text]);
    assert_eq!(result.output, final_text);
    assert!(signal.is_aborted());
    assert!(result.cancelled);
    assert_eq!(result.exit_code, None);
}

#[test]
fn direct_final_utf8_callback_can_cancel_success() {
    cancellation_from_final_chunk("f0908d", "\u{fffd}");
}

#[test]
fn direct_final_escape_callback_can_cancel_success() {
    cancellation_from_final_chunk("1b5d303b7469746c65", "itle");
}

#[test]
fn direct_invalid_prefix_tail_persists_complete_output() {
    corpus("direct_invalid_prefix_tail_persists_complete_output");
}

#[test]
fn direct_invalid_prefix_empty_persists_complete_output() {
    corpus("direct_invalid_prefix_empty_persists_complete_output");
}

#[test]
fn direct_invalid_prefix_cancel_persists_complete_output() {
    corpus("direct_invalid_prefix_cancel_persists_complete_output");
}
