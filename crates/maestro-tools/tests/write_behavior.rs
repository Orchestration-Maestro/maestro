//! What the write tool does: definition, validation, effects, ordering and cancellation.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

mod loop_support;
mod read_support;
mod write_support;

use loop_support::run_turn;
use maestro_agent::{AgentToolResult, AgentToolUpdateCallback};
use maestro_models::{
    BoxFuture, Cancellation, DiagnosticCode, DiagnosticErrorInfo, JsonObject, ToolCall,
    validate_tool_arguments,
};
use maestro_tools::{
    ToolDefinition, WriteOperations, WriteToolInput, WriteToolOptions, create_write_tool,
    create_write_tool_definition, with_file_mutation_queue,
};
use read_support::{TempDir, isolated};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
};
use write_support::{
    Gate, Journal, Mode, ScriptedWrite, failure, hex, input, poll_once, ready, rows,
};

/// The write definition under test.
type Tool = ToolDefinition<(), WriteToolInput, ()>;

/// A write definition over controlled operations.
fn tool_over(ops: &Arc<ScriptedWrite>, cwd: &str) -> Tool {
    create_write_tool_definition(
        cwd.into(),
        WriteToolOptions {
            operations: Some(Arc::clone(ops) as Arc<dyn WriteOperations>),
        },
    )
}

/// Start one write; nothing runs until the future is polled.
fn start(
    tool: &Tool,
    args: WriteToolInput,
    signal: Option<Cancellation>,
) -> BoxFuture<Result<AgentToolResult<()>, DiagnosticErrorInfo>> {
    (tool.execute)("call".into(), args, signal, None, None)
}

/// The content blocks of a result as the model receives them.
fn content_json(result: &AgentToolResult<()>) -> Value {
    serde_json::to_value(&result.content).unwrap()
}

#[test]
fn write_reports_written_utf8_bytes() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        content: String,
        path: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        result: Value,
        bytes: String,
    }
    for row in rows("write_reports_written_utf8_bytes") {
        let args: Input = serde_json::from_value(row.input).unwrap();
        let expected: Expected = serde_json::from_value(row.expected).unwrap();
        let dir = TempDir::new();
        let tool: Tool = create_write_tool_definition(dir.text(), WriteToolOptions::default());
        let result =
            read_support::run(start(&tool, input(&args.path, &args.content), None)).unwrap();
        assert_eq!(json!({"content": content_json(&result)}), expected.result);
        assert!(result.details.is_none() && result.terminate.is_none());
        let written = std::fs::read(dir.0.join(&args.path)).unwrap();
        assert_eq!(hex(&written), expected.bytes);
    }
}

#[test]
fn write_definition_keeps_schema_and_prompt_metadata() {
    let tool: Tool = create_write_tool_definition("/work".into(), WriteToolOptions::default());
    let observed = json!({
        "name": tool.definition.name,
        "label": tool.label,
        "description": tool.definition.description,
        "promptSnippet": tool.prompt_snippet,
        "promptGuidelines": tool.prompt_guidelines,
        "parameters": tool.definition.parameters,
        "hasPrepare": tool.prepare_arguments.is_some(),
        "hasMode": tool.execution_mode.is_some(),
        "hasShell": tool.render_shell.is_some(),
        "hasCall": tool.render_call.is_some(),
        "hasResult": tool.render_result.is_some(),
    });
    let row = rows("write_definition_keeps_schema_and_prompt_metadata").remove(0);
    assert_eq!(row.input, json!({}));
    assert_eq!(observed, row.expected);
}

#[test]
fn write_input_fields_follow_shared_preflight_validation() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        args: JsonObject,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        ok: bool,
        args: Option<Value>,
        error: Option<String>,
    }
    let tool: Tool = create_write_tool_definition("/work".into(), WriteToolOptions::default());
    for row in rows("write_input_fields_follow_shared_preflight_validation") {
        let arguments = serde_json::from_value::<Input>(row.input).unwrap().args;
        let expected: Expected = serde_json::from_value(row.expected).unwrap();
        let call = ToolCall {
            id: "call".into(),
            name: "write".into(),
            arguments,
            thought_signature: None,
        };
        match validate_tool_arguments(&tool.definition, &call) {
            Ok(arguments) => {
                assert!(expected.ok && expected.error.is_none());
                assert_eq!(Some(Value::Object(arguments)), expected.args);
            }
            Err(error) => {
                assert!(!expected.ok && expected.args.is_none());
                assert_eq!(Some(error.message), expected.error);
            }
        }
    }
}

#[test]
fn write_resolves_paths_before_filesystem_work() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        path: String,
        cwd: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        calls: Vec<Vec<String>>,
        result: Value,
    }
    for row in rows("write_resolves_paths_before_filesystem_work") {
        let args: Input = serde_json::from_value(row.input).unwrap();
        let expected: Expected = serde_json::from_value(row.expected).unwrap();
        let ops = Arc::new(ScriptedWrite::new(&Journal::default()));
        let result = read_support::run(start(
            &tool_over(&ops, &args.cwd),
            input(&args.path, "x"),
            None,
        ))
        .unwrap();
        assert_eq!(ops.calls(), expected.calls, "path {:?}", args.path);
        assert_eq!(json!({"content": content_json(&result)}), expected.result);
    }
    let dir = TempDir::new();
    dir.file("shot\u{202F}PM.png", b"existing");
    let ops = Arc::new(ScriptedWrite::new(&Journal::default()));
    read_support::run(start(
        &tool_over(&ops, &dir.text()),
        input("shot PM.png", "x"),
        None,
    ))
    .unwrap();
    assert_eq!(ops.calls()[1][1], format!("{}/shot PM.png", dir.text()));
}

#[test]
fn write_propagates_adapter_failures_without_progress() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        stage: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        calls: Vec<String>,
        error: String,
    }
    for row in rows("write_propagates_adapter_failures_without_progress") {
        let stage = serde_json::from_value::<Input>(row.input).unwrap().stage;
        let expected: Expected = serde_json::from_value(row.expected).unwrap();
        let broken = Mode::Fail(failure(&expected.error));
        let ops = Arc::new(match stage.as_str() {
            "mkdir" => ScriptedWrite::new(&Journal::default()).mkdir_mode(broken),
            "write" => ScriptedWrite::new(&Journal::default()).write_mode(broken),
            other => panic!("unknown stage {other}"),
        });
        let updates = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&updates);
        let progress: AgentToolUpdateCallback<()> = Arc::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let error = read_support::run((tool_over(&ops, "/work").execute)(
            "call".into(),
            input("f.txt", "x"),
            None,
            Some(progress),
            None,
        ))
        .err()
        .unwrap();
        assert_eq!(error, failure(&expected.error));
        assert_eq!(ops.names(), expected.calls);
        assert_eq!(updates.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn write_preaborted_call_starts_no_operations() {
    let row = rows("write_preaborted_call_starts_no_operations").remove(0);
    let expected: Value = row.expected;
    let signal = Cancellation::new();
    signal.abort();
    let ops = Arc::new(ScriptedWrite::new(&Journal::default()));
    let error = read_support::run(start(
        &tool_over(&ops, "/work"),
        input("f.txt", "x"),
        Some(signal),
    ))
    .err()
    .unwrap();
    assert_eq!(
        json!({"calls": ops.names(), "error": error.message}),
        expected
    );
}

/// Queue a journalled `next` operation behind the write of `path`.
fn next_in_line(path: &str, journal: &Journal) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
    let journal = journal.clone();
    with_file_mutation_queue(path, move || async move {
        journal.push("next");
        Ok(())
    })
}

#[test]
fn write_abort_during_mkdir_skips_write_after_queue_release() {
    let expected: Vec<String> = serde_json::from_value(
        rows("write_abort_during_mkdir_skips_write_after_queue_release")
            .remove(0)
            .expected,
    )
    .unwrap();
    let dir = TempDir::new();
    let path = format!("{}/f.txt", dir.text());
    let journal = Journal::default();
    let signal = Cancellation::new();
    let ops = Arc::new(ScriptedWrite::new(&journal).mkdir_mode(Mode::Pend));
    let mut call = start(
        &tool_over(&ops, "/work"),
        input(&path, "x"),
        Some(signal.clone()),
    );
    assert!(poll_once(&mut call).is_pending());
    let mut next = next_in_line(&path, &journal);
    assert!(poll_once(&mut next).is_pending());
    signal.abort();
    journal.push(ready(&mut call).err().unwrap().message);
    assert_eq!(ops.dropped.load(Ordering::SeqCst), 1);
    ready(&mut next).unwrap();
    assert_eq!(journal.entries(), expected);
    assert_eq!(ops.names(), ["mkdir"]);

    let synchronous = Cancellation::new();
    let ops = Arc::new(
        ScriptedWrite::new(&Journal::default())
            .mkdir_mode(Mode::AbortThenSucceed)
            .signal(&synchronous),
    );
    let error = read_support::run(start(
        &tool_over(&ops, "/work"),
        input(&path, "x"),
        Some(synchronous),
    ))
    .err()
    .unwrap();
    assert_eq!(error.message, "Operation aborted");
    assert_eq!(ops.names(), ["mkdir"]);
}

#[test]
fn write_abort_releases_queue_and_drops_pending_operation() {
    let expected: Vec<String> = serde_json::from_value(
        rows("write_abort_releases_queue_and_drops_pending_operation")
            .remove(0)
            .expected,
    )
    .unwrap();
    let dir = TempDir::new();
    let path = format!("{}/f.txt", dir.text());
    let journal = Journal::default();
    let signal = Cancellation::new();
    let ops = Arc::new(ScriptedWrite::new(&journal).write_mode(Mode::Pend));
    let mut call = start(
        &tool_over(&ops, "/work"),
        input(&path, "x"),
        Some(signal.clone()),
    );
    assert!(poll_once(&mut call).is_pending());
    let mut next = next_in_line(&path, &journal);
    assert!(poll_once(&mut next).is_pending());
    assert_eq!(ops.dropped.load(Ordering::SeqCst), 0);
    signal.abort();
    journal.push(ready(&mut call).err().unwrap().message);
    assert_eq!(ops.dropped.load(Ordering::SeqCst), 1);
    ready(&mut next).unwrap();
    assert_eq!(journal.entries(), expected);
}

#[test]
fn write_late_abort_cannot_replace_success() {
    for write in [
        Mode::AbortThenSucceed,
        Mode::AbortThenFail(failure("late failure")),
    ] {
        let signal = Cancellation::new();
        let ops = Arc::new(
            ScriptedWrite::new(&Journal::default())
                .write_mode(write)
                .signal(&signal),
        );
        let error = read_support::run(start(
            &tool_over(&ops, "/work"),
            input("f.txt", "x"),
            Some(signal),
        ))
        .err()
        .unwrap();
        assert_eq!(error.message, "Operation aborted");
        assert_eq!(ops.names(), ["mkdir", "write"]);
    }
}

#[test]
fn write_waiting_abort_preserves_predecessor_order() {
    let dir = TempDir::new();
    let path = format!("{}/f.txt", dir.text());
    let journal = Journal::default();
    let (open_a, gate_a) = Gate::new();
    let held = journal.clone();
    let mut a = with_file_mutation_queue(&path, move || async move {
        held.push("a:start");
        gate_a.await.unwrap();
        held.push("a:end");
        Ok(())
    });
    assert!(poll_once(&mut a).is_pending());
    let signal = Cancellation::new();
    let ops = Arc::new(ScriptedWrite::new(&journal));
    let mut b = start(
        &tool_over(&ops, "/work"),
        input(&path, "x"),
        Some(signal.clone()),
    );
    assert!(poll_once(&mut b).is_pending());
    signal.abort();
    assert!(poll_once(&mut b).is_pending());
    let mut c = next_in_line(&path, &journal);
    assert!(poll_once(&mut c).is_pending());
    open_a.open();
    ready(&mut a).unwrap();
    assert_eq!(ready(&mut b).err().unwrap().message, "Operation aborted");
    assert!(ops.calls().is_empty());
    ready(&mut c).unwrap();
    assert_eq!(journal.entries(), ["a:start", "a:end", "next"]);
}

/// Write through the erased tool, as an agent loop does.
fn through(
    options: WriteToolOptions,
    cwd: &str,
    path: &str,
    content: &str,
) -> Result<AgentToolResult, DiagnosticErrorInfo> {
    let tool = create_write_tool(cwd.into(), options);
    read_support::run((tool.execute)(
        "id".into(),
        json!({"path": path, "content": content}),
        None,
        None,
    ))
}

/// Write natively below `dir` and check the file each time.
fn native_writes(dir: &TempDir) {
    let write = |path: &str, content: &str| {
        through(WriteToolOptions::default(), &dir.text(), path, content)
    };
    for (content, expected) in [("first", "first"), ("2nd", "2nd"), ("", "")] {
        write("a/b/c.txt", content).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.0.join("a/b/c.txt")).unwrap(),
            expected
        );
    }
    #[cfg(unix)]
    {
        let target = dir.file("target.txt", b"old");
        std::os::unix::fs::symlink(&target, dir.0.join("alias.txt")).unwrap();
        write("alias.txt", "through alias").unwrap();
        let alias = std::fs::symlink_metadata(dir.0.join("alias.txt")).unwrap();
        assert!(alias.file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "through alias");
    }
    std::fs::create_dir(dir.0.join("folder")).unwrap();
    let error = write("folder", "x").err().unwrap();
    assert!(matches!(error.code, Some(DiagnosticCode::Number(_))));
}

#[test]
fn write_native_and_supplied_adapters_share_one_caller() {
    let dir = TempDir::new();
    native_writes(&dir);
    let ops = Arc::new(ScriptedWrite::new(&Journal::default()));
    let options = WriteToolOptions {
        operations: Some(Arc::clone(&ops) as Arc<dyn WriteOperations>),
    };
    let result = through(options, "/work", "x/y.txt", "supplied bytes").unwrap();
    assert_eq!(
        content_json_erased(&result),
        json!([{"type": "text", "text": "Successfully wrote 14 bytes to x/y.txt"}])
    );
    assert_eq!(
        ops.calls(),
        [
            vec!["mkdir", "/work/x"],
            vec!["write", "/work/x/y.txt", "supplied bytes"]
        ]
    );
    assert!(!dir.0.join("x").exists());
}

/// The content blocks of an erased result.
fn content_json_erased(result: &AgentToolResult) -> Value {
    serde_json::to_value(&result.content).unwrap()
}

#[test]
fn wrapped_write_runs_through_agent_loop() {
    let dir = TempDir::new();
    let tool = create_write_tool(dir.text(), WriteToolOptions::default());
    let turn = run_turn(
        vec![tool],
        &[
            (
                "good",
                "write",
                json!({"path": "out/f.txt", "content": "loop"}),
            ),
            ("bad", "write", json!({"path": "never.txt"})),
        ],
    );
    assert_eq!(
        std::fs::read_to_string(dir.0.join("out/f.txt")).unwrap(),
        "loop"
    );
    assert!(!dir.0.join("never.txt").exists());
    let [good, bad] = turn.results.as_slice() else {
        panic!("expected two results");
    };
    assert_eq!(good["toolCallId"], "good");
    assert_eq!(good["isError"], false);
    assert_eq!(
        good["content"],
        json!([{"type": "text", "text": "Successfully wrote 4 bytes to out/f.txt"}])
    );
    assert!(good.get("details").is_none());
    assert_eq!(bad["toolCallId"], "bad");
    assert_eq!(bad["isError"], true);
    assert!(bad["content"][0]["text"].as_str().unwrap().contains("path"));
    assert_eq!(turn.updates, 0);
}

#[test]
fn write_path_capture_avoids_unneeded_ambient_reads() {
    let test = "write_path_capture_avoids_unneeded_ambient_reads";
    if isolated(test, "no-cwd", &[]) {
        let parent = TempDir::new();
        let gone = parent.0.join("gone");
        std::fs::create_dir(&gone).unwrap();
        std::env::set_current_dir(&gone).unwrap();
        std::fs::remove_dir(&gone).unwrap();
        assert!(std::env::current_dir().is_err());
        let ops = Arc::new(ScriptedWrite::new(&Journal::default()));
        let anchored = start(&tool_over(&ops, "relative"), input("/abs/f.txt", "x"), None);
        assert_eq!(ops.calls().len(), 0);
        read_support::run(anchored).unwrap();
        assert_eq!(ops.names(), ["mkdir", "write"]);
        let mut needs = start(&tool_over(&ops, "relative"), input("rel.txt", "x"), None);
        assert!(matches!(poll_once(&mut needs), Poll::Ready(Err(_))));
        assert_eq!(ops.names().len(), 2);
    }
    if isolated(test, "bound", &[]) {
        let (original, decoy) = (TempDir::new(), TempDir::new());
        std::env::set_current_dir(&original.0).unwrap();
        let ops = Arc::new(ScriptedWrite::new(&Journal::default()));
        let pending = start(&tool_over(&ops, "."), input("f.txt", "x"), None);
        std::env::set_current_dir(&decoy.0).unwrap();
        read_support::run(pending).unwrap();
        let written = std::fs::canonicalize(original.0.join("f.txt").parent().unwrap()).unwrap();
        assert_eq!(ops.calls()[1][1], written.join("f.txt").to_str().unwrap());
    }
}
