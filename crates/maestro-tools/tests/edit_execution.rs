//! Native edit composition and its supplied-operation lifecycle.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::edit_support;
use crate::ls_support::posix_rows;
use crate::read_support;
use crate::runtime::run;
use crate::write_support;
use edit_support::*;
use maestro_agent::AgentToolUpdateCallback;
use maestro_tools::{EditOperations, EditToolOptions, create_edit_tool_definition};
use read_support::{Fixture, Scratch};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// Replay one recorded file group through native execution with a closed progress producer.
fn native_cases(test: &str) {
    for row in rows(test) {
        let args: FileInput = serde_json::from_value(row.input).unwrap();
        assert!(args.mode.is_none());
        let expected: FileExpected = serde_json::from_value(row.expected).unwrap();
        let dir = Scratch::new().unwrap();
        let absolute = maestro_tools::resolve_to_cwd(&args.path, &dir.text()).unwrap();
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&absolute)
            .unwrap();
        std::fs::write(&absolute, &args.bytes).unwrap();
        let tool: Tool = create_edit_tool_definition(dir.text(), EditToolOptions::default());
        let updates = Arc::new(AtomicUsize::new(0));
        let counter = updates.clone();
        let update: AgentToolUpdateCallback<maestro_tools::EditToolDetails> = Arc::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let result = run((tool.execute)(
            "id".into(),
            args.arguments(),
            None,
            Some(update),
            None,
        ));
        assert_eq!(result.is_ok(), expected.ok);
        match result {
            Ok(result) => assert_eq!(Some(result_json(&result)), expected.result),
            Err(error) => {
                assert!(!error.message.is_empty());
                assert!(expected.result.is_none());
            }
        }
        assert_eq!(std::fs::read(absolute).unwrap(), expected.bytes);
        assert_eq!(updates.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn edit_execution_returns_written_bytes_and_numbered_details() {
    native_cases("edit_execution_returns_written_bytes_and_numbered_details");
}

#[test]
fn edit_execution_matches_one_original_snapshot() {
    native_cases("edit_execution_matches_one_original_snapshot");
}

#[test]
fn edit_execution_keeps_compact_distant_hunks() {
    native_cases("edit_execution_keeps_compact_distant_hunks");
}

#[test]
fn edit_execution_uses_shared_fuzzy_comparison_base() {
    native_cases("edit_execution_uses_shared_fuzzy_comparison_base");
}

#[test]
fn edit_execution_preserves_unrelated_exact_text() {
    native_cases("edit_execution_preserves_unrelated_exact_text");
}

#[test]
fn edit_execution_restores_bom_and_first_newline_style() {
    native_cases("edit_execution_restores_bom_and_first_newline_style");
}

#[test]
fn edit_execution_decodes_bytes_lossily() {
    native_cases("edit_execution_decodes_bytes_lossily");
}

#[test]
fn edit_execution_resolves_authored_paths() {
    native_cases("edit_execution_resolves_authored_paths");
}

#[test]
fn edit_execution_inherits_matching_corrections() {
    native_cases("edit_execution_inherits_matching_corrections");
}

use maestro_models::{Cancellation, DiagnosticErrorInfo};
use serde::Deserialize;
use serde_json::{Value, json};
use write_support::{Mode, failure, poll_once, ready};

#[test]
fn edit_matching_failures_leave_original_bytes() {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Expected {
        ok: bool,
        bytes: Vec<u8>,
        error_contains: Vec<String>,
    }
    for row in rows("edit_matching_failures_leave_original_bytes") {
        let args: FileInput = serde_json::from_value(row.input).unwrap();
        assert!(args.mode.is_none());
        let expected: Expected = serde_json::from_value(row.expected).unwrap();
        let empty = args.edits.is_empty();
        let ops = Arc::new(Operations::new(&args.bytes));
        let error = run(start(&tool(&ops, "/work"), args.arguments(), None))
            .err()
            .unwrap();
        assert!(!expected.ok);
        for fragment in &expected.error_contains {
            assert!(
                error.message.contains(fragment),
                "{} lacks {fragment}",
                error.message
            );
        }
        assert_eq!(
            ops.script.names(),
            if empty {
                vec![]
            } else {
                vec!["access", "read"]
            }
        );
        assert_eq!(*ops.bytes.lock().unwrap(), expected.bytes);
        assert_eq!(error.name.as_deref(), Some("Error"));
    }
}

#[cfg(unix)]
#[test]
fn edit_execution_requires_read_and_write_access() {
    use maestro_tools::edit_diff::compute_edit_diff;
    for row in rows("edit_execution_requires_read_and_write_access") {
        let args: FileInput = serde_json::from_value(row.input).unwrap();
        let expected: FileExpected = serde_json::from_value(row.expected).unwrap();
        let dir = Scratch::new().unwrap();
        let path = dir.file(&args.path, &args.bytes);
        let mode = args.mode.unwrap();
        // A privileged run reads and writes anyway, so there is nothing to check.
        let Some(restore) = maestro_test_directory::deny(&path, mode).unwrap() else {
            continue;
        };
        let tool: Tool = create_edit_tool_definition(dir.text(), EditToolOptions::default());
        let error = run(start(&tool, args.arguments(), None)).err().unwrap();
        assert!(error.message.starts_with("Could not edit file:"));
        assert!(!expected.ok && expected.result.is_none());
        if mode == 0o444 {
            assert!(run(compute_edit_diff(&path, "hello", "world", &dir.text()).unwrap()).is_ok());
        }
        drop(restore);
        assert_eq!(std::fs::read(path).unwrap(), expected.bytes);
    }
}

/// Recorded operation stage and optional coded/failed policy.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StageInput {
    stage: String,
    coded: Option<bool>,
    fails: Option<bool>,
}
/// Recorded failure with a closed sequence of calls.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failed {
    calls: Option<Vec<String>>,
    events: Option<Vec<String>>,
    ok: bool,
}

#[test]
fn edit_operation_failures_stop_later_stages() {
    for row in rows("edit_operation_failures_stop_later_stages") {
        let args: StageInput = serde_json::from_value(row.input).unwrap();
        assert!(args.fails.is_none());
        let expected: Failed = serde_json::from_value(row.expected).unwrap();
        let mut original = failure("controlled stage failure");
        if !args.coded.unwrap() {
            original.code = None;
        }
        let ops =
            Arc::new(Operations::new(b"hello").at(&args.stage, Mode::Fail(original.clone()), None));
        let error = run(start(
            &tool(&ops, "/work"),
            input("failure-file", "hello", "world"),
            None,
        ))
        .err()
        .unwrap();
        assert!(!expected.ok && expected.events.is_none());
        assert_eq!(Some(ops.script.names()), expected.calls);
        if args.stage == "access" {
            assert!(error.message.contains("Could not edit file: failure-file."));
            assert!(error.message.contains(if args.coded.unwrap() {
                "Error code: E_CONTROL"
            } else {
                "controlled stage failure"
            }));
        } else {
            assert_eq!(error, original);
        }
        assert_eq!(*ops.bytes.lock().unwrap(), b"hello");
    }
}

#[test]
fn edit_native_failures_do_not_create_targets() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Args {
        path: String,
    }
    for row in rows("edit_native_failures_do_not_create_targets") {
        let args: Args = serde_json::from_value(row.input).unwrap();
        let expected: Failed = serde_json::from_value(row.expected).unwrap();
        assert!(!expected.ok && expected.calls.is_none() && expected.events.is_none());
        let dir = Scratch::new().unwrap();
        if args.path == "directory" {
            std::fs::create_dir(dir.path().join(&args.path)).unwrap();
        }
        let tool: Tool = create_edit_tool_definition(dir.text(), EditToolOptions::default());
        let error = run(start(&tool, input(&args.path, "hello", "world"), None))
            .err()
            .unwrap();
        if args.path == "directory" {
            assert!(!error.message.starts_with("Could not edit file:"));
        } else {
            assert!(!dir.path().join(args.path).exists());
            assert!(error.message.starts_with("Could not edit file:"));
        }
    }
    let dir = Scratch::new().unwrap();
    let tool: Tool = create_edit_tool_definition(dir.text(), EditToolOptions::default());
    assert!(run(start(&tool, input("missing/file", "hello", "world"), None)).is_err());
    assert!(!dir.path().join("missing").exists());
}

#[test]
fn edit_synchronous_abort_wins_over_stage_result() {
    for row in rows("edit_synchronous_abort_wins_over_stage_result") {
        let args: StageInput = serde_json::from_value(row.input).unwrap();
        assert!(args.coded.is_none());
        let expected: Failed = serde_json::from_value(row.expected).unwrap();
        let signal = Cancellation::new();
        let mode = if args.fails.unwrap() {
            Mode::AbortThenFail(failure("stage failed"))
        } else {
            Mode::AbortThenSucceed
        };
        let ops = Arc::new(Operations::new(b"hello").at(&args.stage, mode, Some(&signal)));
        let error = run(start(
            &tool(&ops, "/work"),
            input("sync-file", "hello", "world"),
            Some(signal),
        ))
        .err()
        .unwrap();
        assert!(!expected.ok && expected.calls.is_none());
        assert_eq!(Some(ops.script.names()), expected.events);
        assert_eq!(error.message, "Operation aborted");
    }
    let signal = Cancellation::new();
    let ops = Arc::new(Operations::new(b"hello"));
    let result = run(start(
        &tool(&ops, "/work"),
        input("late-file", "hello", "world"),
        Some(signal.clone()),
    ))
    .unwrap();
    signal.abort();
    assert_eq!(result.details.unwrap().diff, "-1 hello\n+1 world");
}

#[test]
fn edit_preaborted_call_starts_no_operations() {
    let row = rows("edit_preaborted_call_starts_no_operations").remove(0);
    assert_eq!(row.input, json!({}));
    let expected: Failed = serde_json::from_value(row.expected).unwrap();
    let signal = Cancellation::new();
    signal.abort();
    let ops = Arc::new(Operations::new(b"hello"));
    let error = run(start(
        &tool(&ops, "/work"),
        input("preabort-file", "hello", "world"),
        Some(signal),
    ))
    .err()
    .unwrap();
    assert_eq!(error.message, "Operation aborted");
    assert!(!expected.ok && expected.events.is_none());
    assert_eq!(Some(ops.script.names()), expected.calls);
    assert!(
        run(start(
            &tool(&ops, "/work"),
            input("preabort-file", "hello", "world"),
            None
        ))
        .is_ok()
    );
}

#[test]
fn edit_abort_drops_each_pending_stage_and_releases_queue() {
    for row in rows("edit_abort_drops_each_pending_stage_and_releases_queue") {
        let args: StageInput = serde_json::from_value(row.input).unwrap();
        assert!(args.coded.is_none() && args.fails.is_none());
        let expected: Vec<String> = serde_json::from_value(row.expected).unwrap();
        let signal = Cancellation::new();
        let ops = Arc::new(Operations::new(b"hello").at(&args.stage, Mode::Pend, None));
        let mut pending = start(
            &tool(&ops, "/work"),
            input("pending-file", "hello", "world"),
            Some(signal.clone()),
        );
        assert!(poll_once(&mut pending).is_pending());
        let journal = write_support::Journal::default();
        let after = journal.clone();
        let mut successor =
            maestro_tools::with_file_mutation_queue("/work/pending-file", move || async move {
                after.push("successor");
                Ok(())
            });
        assert!(poll_once(&mut successor).is_pending());
        signal.abort();
        let error = ready(&mut pending).err().unwrap();
        assert_eq!(ops.script.dropped.load(Ordering::SeqCst), 1);
        let mut events = ops.events.entries();
        events.push(error.message);
        ready(&mut successor).unwrap();
        events.extend(journal.entries());
        assert_eq!(events, expected);
    }
}

#[test]
fn edit_execution_binds_expanded_paths_before_polling() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Args {
        cwd: String,
        path: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        calls: Vec<Vec<String>>,
        result: Value,
    }
    let home = Some("/maestro-test-home");
    if !read_support::isolated(
        "edit_execution_binds_expanded_paths_before_polling",
        "paths",
        &[("HOME", home), ("USERPROFILE", home)],
    ) {
        return;
    }
    for row in rows("edit_execution_binds_expanded_paths_before_polling") {
        let args: Args = serde_json::from_value(row.input).unwrap();
        let expected: Expected = serde_json::from_value(row.expected).unwrap();
        let ops = Arc::new(Operations::new(b"hello"));
        let mut future = start(
            &tool(&ops, &args.cwd),
            input(&args.path, "hello", "world"),
            None,
        );
        assert!(ops.script.calls().is_empty());
        assert_eq!(result_json(&ready(&mut future).unwrap()), expected.result);
        assert_eq!(posix_rows(&ops.script.calls()), expected.calls);
    }
    cwd_binding();
}

/// Compare both constructors against a cwd decoy after invocation.
fn cwd_binding() {
    let original = Scratch::new().unwrap();
    let decoy = Scratch::new().unwrap();
    original.file("file", b"hello");
    decoy.file("file", b"decoy");
    std::env::set_current_dir(original.path()).unwrap();
    for wrapped in [false, true] {
        let ops = Arc::new(Operations::new(b"hello"));
        let absolute = original.path().join("file").to_str().unwrap().to_owned();
        if wrapped {
            let tool = agent_tool(&ops, ".");
            let future = (tool.execute)(
                "id".into(),
                json!({"path":"file","edits":[{"oldText":"hello","newText":"world"}]}),
                None,
                None,
            );
            std::env::set_current_dir(decoy.path()).unwrap();
            run(future).unwrap();
        } else {
            let future = start(&tool(&ops, "."), input("file", "hello", "world"), None);
            std::env::set_current_dir(decoy.path()).unwrap();
            run(future).unwrap();
        }
        assert!(ops.script.calls().iter().all(|call| call[1] == absolute));
        std::env::set_current_dir(original.path()).unwrap();
    }
    std::env::set_current_dir(std::env::temp_dir()).unwrap();
}

/// Native adapter whose first read is captured before a controlled release.
struct GatedNative {
    /// First read's release receiver; subsequent reads are ungated.
    gate: std::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    /// Ordered stages observed on this adapter.
    journal: write_support::Journal,
}
impl GatedNative {
    /// One gated first read.
    fn new() -> (Arc<Self>, write_support::Gate) {
        let (gate, receiver) = write_support::Gate::new();
        (
            Arc::new(Self {
                gate: std::sync::Mutex::new(Some(receiver)),
                journal: write_support::Journal::default(),
            }),
            gate,
        )
    }
}
impl maestro_tools::EditOperations for GatedNative {
    fn access(&self, _: String) -> maestro_models::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.journal.push("access");
        Box::pin(std::future::ready(Ok(())))
    }
    fn read_file(
        &self,
        path: String,
    ) -> maestro_models::BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>> {
        let gate = self.gate.lock().unwrap().take();
        let journal = self.journal.clone();
        Box::pin(async move {
            let bytes = std::fs::read(path).unwrap();
            journal.push("read:captured");
            if let Some(gate) = gate {
                gate.await.unwrap();
            }
            Ok(bytes)
        })
    }
    fn write_file(
        &self,
        path: String,
        content: String,
    ) -> maestro_models::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let journal = self.journal.clone();
        Box::pin(async move {
            std::fs::write(path, content).unwrap();
            journal.push("write:done");
            Ok(())
        })
    }
}
#[test]
fn edit_parallel_calls_preserve_both_changes() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        results: Vec<Value>,
        bytes: Vec<u8>,
    }
    let row = rows("edit_parallel_calls_preserve_both_changes").remove(0);
    assert_eq!(row.input, json!({}));
    let expected: Expected = serde_json::from_value(row.expected).unwrap();
    let dir = Scratch::new().unwrap();
    let path = dir.file("queued.txt", b"alpha\nbeta\n");
    let (ops, gate) = GatedNative::new();
    let tool = tool(&ops, &dir.text());
    let mut a = start(&tool, input("queued.txt", "alpha", "ALPHA"), None);
    assert!(poll_once(&mut a).is_pending());
    assert_eq!(ops.journal.entries(), ["access", "read:captured"]);
    let mut b = start(&tool, input("queued.txt", "beta", "BETA"), None);
    assert!(poll_once(&mut b).is_pending());
    gate.open();
    let a = ready(&mut a).unwrap();
    let b = ready(&mut b).unwrap();
    assert_eq!(vec![result_json(&a), result_json(&b)], expected.results);
    assert_eq!(std::fs::read(path).unwrap(), expected.bytes);
}

#[test]
fn edit_and_write_use_one_queue() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Args {
        order: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        bytes: Vec<u8>,
    }
    for row in rows("edit_and_write_use_one_queue") {
        let args: Args = serde_json::from_value(row.input).unwrap();
        let expected: Expected = serde_json::from_value(row.expected).unwrap();
        let dir = Scratch::new().unwrap();
        dir.file("mixed.txt", b"hello\n");
        let (ops, gate) = GatedNative::new();
        let edit = tool(&ops, &dir.text());
        let replacement = write_support::input("mixed.txt", "replacement\n");
        if args.order == "edit-write" {
            let (writer, release) = GatedNative::new();
            let write = maestro_tools::create_write_tool_definition::<()>(
                dir.text(),
                maestro_tools::WriteToolOptions {
                    operations: Some(writer),
                },
            );
            release.open();
            let mut a = start(&edit, input("mixed.txt", "hello", "world"), None);
            assert!(poll_once(&mut a).is_pending());
            let mut b = (write.execute)("id".into(), replacement, None, None, None);
            assert!(poll_once(&mut b).is_pending());
            gate.open();
            ready(&mut a).unwrap();
            run(b).unwrap();
        } else {
            assert_eq!(args.order, "write-edit");
            let (writer, release) = GatedNative::new();
            let write = maestro_tools::create_write_tool_definition::<()>(
                dir.text(),
                maestro_tools::WriteToolOptions {
                    operations: Some(writer.clone()),
                },
            );
            let mut b = (write.execute)("id".into(), replacement, None, None, None);
            assert!(poll_once(&mut b).is_pending());
            let mut c = start(&edit, input("mixed.txt", "replacement", "edited"), None);
            assert!(poll_once(&mut c).is_pending());
            release.open();
            ready(&mut b).unwrap();
            gate.open();
            run(c).unwrap();
        }
        assert_eq!(
            std::fs::read(dir.path().join("mixed.txt")).unwrap(),
            expected.bytes
        );
    }
}

impl maestro_tools::WriteOperations for GatedNative {
    fn mkdir(&self, _: String) -> maestro_models::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(std::future::ready(Ok(())))
    }
    fn write_file(
        &self,
        path: String,
        content: String,
    ) -> maestro_models::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let gate = self.gate.lock().unwrap().take();
        let write = EditOperations::write_file(self, path, content);
        Box::pin(async move {
            if let Some(gate) = gate {
                gate.await.unwrap();
            }
            write.await
        })
    }
}

#[cfg(unix)]
#[test]
fn edit_empty_list_fails_before_path_or_queue() {
    if !read_support::isolated("edit_empty_list_fails_before_path_or_queue", "empty", &[]) {
        return;
    }
    let removed = maestro_test_directory::Scratch::new().unwrap();
    removed.enter_removed("removed").unwrap();
    let signal = Cancellation::new();
    signal.abort();
    let ops = Arc::new(Operations::new(b"hello"));
    let args = maestro_tools::EditToolInput {
        path: "relative".into(),
        edits: Vec::new(),
    };
    let error = run(start(&tool(&ops, "."), args, Some(signal.clone())))
        .err()
        .unwrap();
    assert!(error.message.contains("edits must contain"));
    let wrapped = agent_tool(&ops, ".");
    let error = run((wrapped.execute)(
        "id".into(),
        json!({"path":"relative","edits":[]}),
        Some(signal),
        None,
    ))
    .err()
    .unwrap();
    assert!(error.message.contains("edits must contain"));
    assert!(ops.script.calls().is_empty());
    std::env::set_current_dir(std::env::temp_dir()).unwrap();
}

#[test]
fn edit_definition_and_wrapper_share_operations() {
    let dir = Scratch::new().unwrap();
    for wrapped in [false, true] {
        let ops = Arc::new(Operations::new(b"supplied"));
        if wrapped {
            let tool = agent_tool(&ops, &dir.text());
            let result = run((tool.execute)(
                "id".into(),
                json!({"path":"target","edits":[{"oldText":"supplied","newText":"changed"}]}),
                None,
                None,
            ))
            .unwrap();
            assert_eq!(
                result.details,
                Some(json!({"diff":"-1 supplied\n+1 changed","firstChangedLine":1}))
            );
        } else {
            let result = run(start(
                &tool(&ops, &dir.text()),
                input("target", "supplied", "changed"),
                None,
            ))
            .unwrap();
            assert_eq!(result.details.unwrap().diff, "-1 supplied\n+1 changed");
        }
        assert_eq!(
            posix_rows(&ops.script.calls()),
            posix_rows(&[
                vec!["access".into(), format!("{}/target", dir.text())],
                vec!["read".into(), format!("{}/target", dir.text())],
                vec![
                    "write".into(),
                    format!("{}/target", dir.text()),
                    "changed".into()
                ]
            ])
        );
        assert_eq!(*ops.bytes.lock().unwrap(), b"changed");
    }
    native_adapter_routes(&dir);
}

/// Check the default adapter's symlink and literal filename routes.
fn native_adapter_routes(dir: &Scratch) {
    let path = dir.file("target", b"hello");
    #[cfg(unix)]
    let selected = {
        std::os::unix::fs::symlink("target", dir.path().join("alias")).unwrap();
        "alias"
    };
    #[cfg(not(unix))]
    let selected = "target";
    let tool = maestro_tools::create_edit_tool(dir.text(), EditToolOptions::default());
    run((tool.execute)(
        "id".into(),
        json!({"path":selected,"edits":[{"oldText":"hello","newText":"world"}]}),
        None,
        None,
    ))
    .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"world");
    #[cfg(unix)]
    assert!(
        std::fs::symlink_metadata(dir.path().join("alias"))
            .unwrap()
            .is_symlink()
    );
    // A host that folds canonically equivalent names finds the decomposed file under the
    // precomposed spelling, so the edit would reach it there.
    let pair = ["caf\u{e9}.txt", "cafe\u{301}.txt"].map(std::ffi::OsStr::new);
    if !dir.keeps_distinct(&pair).unwrap() {
        eprintln!("the fallback check needs a host that keeps {pair:?} apart");
        return;
    }
    dir.file("cafe\u{301}.txt", b"hello");
    assert!(
        run((tool.execute)(
            "id".into(),
            json!({"path":"café.txt","edits":[{"oldText":"hello","newText":"world"}]}),
            None,
            None
        ))
        .is_err()
    );
    assert_eq!(
        std::fs::read(dir.path().join("cafe\u{301}.txt")).unwrap(),
        b"hello"
    );
}

#[test]
fn edit_preview_and_execution_share_details() {
    let dir = Scratch::new().unwrap();
    let bytes = "\u{feff}“hello”  \r\nnext\r\n".as_bytes();
    let path = dir.file("file", bytes);
    let args = maestro_tools::EditToolInput {
        path: "file".into(),
        edits: vec![
            maestro_tools::edit_diff::Edit {
                old_text: "\"hello\"".into(),
                new_text: "world".into(),
            },
            maestro_tools::edit_diff::Edit {
                old_text: "next".into(),
                new_text: "NEXT".into(),
            },
        ],
    };
    let preview =
        run(
            maestro_tools::edit_diff::compute_edits_diff("file", args.edits.clone(), &dir.text())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let tool: Tool = create_edit_tool_definition(dir.text(), EditToolOptions::default());
    let result = run(start(&tool, args, None)).unwrap();
    assert_eq!(result.details, Some(preview));
    assert_eq!(
        std::fs::read(path).unwrap(),
        "\u{feff}world\r\nNEXT\r\n".as_bytes()
    );
}

#[test]
fn edit_queued_abort_waits_for_predecessor() {
    let dir = Scratch::new().unwrap();
    let path = dir.file("file", b"hello");
    let (ops, release) = GatedNative::new();
    let first = tool(&ops, &dir.text());
    let mut a = start(&first, input("file", "hello", "world"), None);
    assert!(poll_once(&mut a).is_pending());
    let signal = Cancellation::new();
    let never = Arc::new(Operations::new(b"world"));
    let next = tool(&never, &dir.text());
    let mut b = start(
        &next,
        input("file", "world", "discarded"),
        Some(signal.clone()),
    );
    let dropped = start(&next, input("file", "world", "unpolled"), None);
    let mut failed = start(&next, input("file", "missing", "bad"), None);
    let mut c = start(&next, input("file", "world", "finished"), None);
    assert!(poll_once(&mut b).is_pending());
    signal.abort();
    assert!(poll_once(&mut b).is_pending());
    drop(dropped);
    assert!(poll_once(&mut failed).is_pending() && poll_once(&mut c).is_pending());
    assert!(never.script.calls().is_empty());
    release.open();
    ready(&mut a).unwrap();
    assert_eq!(ready(&mut b).err().unwrap().message, "Operation aborted");
    assert!(never.script.calls().is_empty());
    assert!(ready(&mut failed).is_err());
    ready(&mut c).unwrap();
    assert_eq!(*never.bytes.lock().unwrap(), b"finished");
    assert_eq!(std::fs::read(path).unwrap(), b"world");
}

#[cfg(unix)]
#[test]
fn edit_queue_preserves_aliases_and_independent_files() {
    let dir = Scratch::new().unwrap();
    let path = dir.file("file", b"hello");
    dir.file("other", b"hello");
    std::os::unix::fs::symlink("file", dir.path().join("alias")).unwrap();
    let (ops, release) = GatedNative::new();
    let tool = tool(&ops, &dir.text());
    let mut a = start(&tool, input("file", "hello", "world"), None);
    assert!(poll_once(&mut a).is_pending());
    let mut b = start(&tool, input("alias", "world", "aliased"), None);
    assert!(poll_once(&mut b).is_pending());
    let mut other = start(&tool, input("other", "hello", "independent"), None);
    ready(&mut other).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("other")).unwrap(),
        b"independent"
    );
    release.open();
    ready(&mut a).unwrap();
    ready(&mut b).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"aliased");
}

#[cfg(unix)]
#[test]
fn edit_path_capture_reads_ambient_state_only_when_needed() {
    if !read_support::isolated(
        "edit_path_capture_reads_ambient_state_only_when_needed",
        "ambient",
        &[],
    ) {
        return;
    }
    let dir = Scratch::new().unwrap();
    let gone = maestro_test_directory::Scratch::new().unwrap();
    let absolute = dir.file("file", b"hello");
    gone.enter_removed("removed").unwrap();
    for wrapped in [false, true] {
        for (cwd, path, success) in [
            (".".to_owned(), absolute.clone(), true),
            (dir.text(), "file".into(), true),
            (".".into(), "file".into(), false),
        ] {
            let ops = Arc::new(Operations::new(b"hello"));
            let ok = if wrapped {
                let tool = agent_tool(&ops, &cwd);
                run((tool.execute)(
                    "id".into(),
                    json!({"path":path,"edits":[{"oldText":"hello","newText":"world"}]}),
                    None,
                    None,
                ))
                .is_ok()
            } else {
                run(start(
                    &tool(&ops, &cwd),
                    input(&path, "hello", "world"),
                    None,
                ))
                .is_ok()
            };
            assert_eq!(ok, success);
            assert_eq!(
                ops.script.names(),
                if success {
                    vec!["access", "read", "write"]
                } else {
                    vec![]
                }
            );
            if success {
                assert_eq!(*ops.bytes.lock().unwrap(), b"world");
            }
        }
    }
    std::env::set_current_dir(std::env::temp_dir()).unwrap();
}
