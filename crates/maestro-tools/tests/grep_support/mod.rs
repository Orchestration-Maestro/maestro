//! Strict content-search corpus and public-boundary drivers.
#![cfg(test)]
use crate::display_support::{Case, search_cases, search_keys};
#[cfg(unix)]
use crate::{
    find_support::{executable, manager},
    ls_support::{canon, result_json, with_home},
    read_support::{Fixture, Scratch},
    runtime::run,
};
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo};
#[cfg(unix)]
use maestro_tools::create_grep_tool;
use maestro_tools::{
    GrepOperations, GrepToolDetails, GrepToolInput, GrepToolOptions, ToolsManager,
    create_grep_tool_definition,
};
use serde::Deserialize;
use serde_json::{Value, json};
#[cfg(unix)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
/// Parse strict cases and reject repeated queries.
pub fn cases(test: &str) -> Vec<Case> {
    search_cases(include_str!("../fixtures/grep_cases.json"), test)
}
/// Use the recorded palette rather than a hard-coded color assignment.
pub fn keys() -> Vec<String> {
    search_keys(include_str!("../fixtures/grep_cases.json"))
}
/// Strict controlled execution input.
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub wrapped: bool,
    pub params: Option<Value>,
    pub cwd: Option<String>,
    pub events: Option<Vec<Value>>,
    pub files: Option<HashMap<String, String>>,
    pub stderr: String,
    pub code: Option<i32>,
    pub signal_exit: Option<String>,
    pub is_directory: Option<bool>,
    pub fail: Option<String>,
    pub abort: Option<String>,
    pub spawn_error: bool,
    pub unterminated: bool,
}
/// Strict observations returned by the oracle.
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    result: Option<Value>,
    error: Option<String>,
    error_prefix: Option<String>,
    updates: usize,
    operations: Vec<Value>,
    argv: Option<Value>,
}
/// Controlled operations with ordered observations.
pub struct Operations {
    pub input: Input,
    pub calls: Mutex<Vec<Value>>,
    pub signal: Cancellation,
}
impl GrepOperations for Operations {
    fn is_directory(&self, path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>> {
        self.calls
            .lock()
            .unwrap()
            .push(json!(["isDirectory", path]));
        if self.input.abort.as_deref() == Some("stat") {
            self.signal.abort();
        }
        let value = if self.input.fail.as_deref() == Some("stat") {
            Err(failure("stat denied"))
        } else {
            Ok(self.input.is_directory.unwrap_or(true))
        };
        Box::pin(async move {
            tokio::task::yield_now().await;
            value
        })
    }
    fn read_file(&self, path: String) -> BoxFuture<Result<String, DiagnosticErrorInfo>> {
        self.calls.lock().unwrap().push(json!(["readFile", path]));
        let value = if self.input.fail.as_deref() == Some("read") {
            Err(failure("read denied"))
        } else {
            Ok(self.input.files.as_ref().map_or_else(
                || "before\nhit\nafter\nlast".into(),
                |files| files.get(&path).cloned().unwrap_or_default(),
            ))
        };
        Box::pin(async move {
            tokio::task::yield_now().await;
            value
        })
    }
}
/// A controlled diagnostic.
pub fn failure(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("Error".into()),
        message: message.into(),
        stack: None,
        code: None,
    }
}
/// Encode controlled events in a file, never a command-line argument.
#[cfg(unix)]
pub fn producer(root: &Scratch, input: &Input) {
    let events = input.events.clone().unwrap_or_else(|| vec![json!({"type":"match","data":{"path":{"text":"/work/z.txt"},"line_number":2,"lines":{"text":"hit\n"}}})]);
    let mut stdout = events
        .iter()
        .map(|e| e.as_str().map_or_else(|| e.to_string(), str::to_owned))
        .collect::<Vec<_>>()
        .join("\n");
    if !input.unterminated {
        stdout.push('\n');
    }
    root.file("stdout", stdout.as_bytes());
    root.file("stderr", input.stderr.as_bytes());
    let termination = input.signal_exit.as_ref().map_or(String::new(), |s| {
        assert_eq!(s, "SIGTERM");
        "os.kill(os.getpid(),signal.SIGTERM)".into()
    });
    let script = format!(
        "#!/usr/bin/python3\nimport os,sys,json,signal\njson.dump(sys.argv[1:],open({},'w'))\nos.write(1,open({},'rb').read())\nos.write(2,open({},'rb').read())\n{termination}\nsys.exit({})\n",
        json!(root.path().join("args.json").to_str().unwrap()),
        json!(root.path().join("stdout").to_str().unwrap()),
        json!(root.path().join("stderr").to_str().unwrap()),
        input.code.unwrap_or(0)
    );
    if input.spawn_error {
        root.file("rg", b"not executable");
    } else {
        executable(root, "rg", &script);
    }
}
/// Invoke one controlled record through a public factory.
#[cfg(unix)]
pub fn controlled(case: &Case) {
    assert_eq!(case.op, "controlled");
    let expected: Expected = serde_json::from_value(with_home(&case.expected)).unwrap();
    let input: Input = serde_json::from_value(with_home(&case.input)).unwrap();
    let root = Scratch::new().unwrap();
    producer(&root, &input);
    let signal = Cancellation::new();
    if matches!(input.abort.as_deref(), Some("before" | "ensure")) {
        signal.abort();
    }
    let operations = Arc::new(Operations {
        input,
        calls: Mutex::new(Vec::new()),
        signal: signal.clone(),
    });
    let fetches = Arc::new(AtomicUsize::new(0));
    let manager = manager(root.text(), &fetches);
    let updates = Arc::new(AtomicUsize::new(0));
    let result = invoke(&operations, manager, signal, &updates);
    assert_eq!(
        updates.load(Ordering::SeqCst),
        expected.updates,
        "updates {}",
        case.id
    );
    assert_eq!(
        *operations.calls.lock().unwrap(),
        expected.operations,
        "operations {}",
        case.id
    );
    if let Some(argv) = expected
        .argv
        .as_ref()
        .filter(|_| !operations.input.spawn_error)
    {
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(root.path().join("args.json")).unwrap())
                .unwrap(),
            *argv,
            "argv {}",
            case.id
        );
    }
    assert_result(case, result, expected);
}
/// Invoke a recorded choice of public factory.
#[cfg(unix)]
fn invoke(
    operations: &Arc<Operations>,
    manager: Arc<ToolsManager>,
    signal: Cancellation,
    updates: &Arc<AtomicUsize>,
) -> Result<Value, DiagnosticErrorInfo> {
    let counter = Arc::clone(updates);
    let cwd = operations
        .input
        .cwd
        .clone()
        .unwrap_or_else(|| "/work".into());
    let params = operations
        .input
        .params
        .clone()
        .unwrap_or_else(|| json!({"pattern":"hit"}));
    let options = GrepToolOptions {
        operations: Some(operations.clone()),
    };
    if operations.input.wrapped {
        let tool = create_grep_tool(cwd, options, manager);
        run((tool.execute)(
            "call".into(),
            params,
            Some(signal),
            Some(Arc::new(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            })),
        ))
        .map(|r| result_json(&r.content, r.details))
    } else {
        let tool = create_grep_tool_definition::<()>(cwd, options, manager);
        run((tool.execute)(
            "call".into(),
            serde_json::from_value(params).unwrap(),
            Some(signal),
            Some(Arc::new(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            })),
            None,
        ))
        .map(|r| {
            result_json(
                &r.content,
                r.details.map(|d| serde_json::to_value(d).unwrap()),
            )
        })
    }
}
/// Compare success, exact failure or native-prefix failure.
#[cfg(unix)]
fn assert_result(case: &Case, result: Result<Value, DiagnosticErrorInfo>, expected: Expected) {
    match result {
        Ok(r) => {
            assert!(expected.error.is_none() && expected.error_prefix.is_none());
            assert_eq!(
                canon(&r),
                canon(&expected.result.unwrap()),
                "result {}",
                case.id
            );
        }
        Err(e) => {
            assert!(
                expected.result.is_none(),
                "unexpected error {}: {}",
                case.id,
                e.message
            );
            if let Some(prefix) = expected.error_prefix {
                assert!(e.message.starts_with(&prefix));
            } else {
                assert_eq!(e.message, expected.error.unwrap(), "error {}", case.id);
            }
        }
    }
}
/// Exercise every controlled record of a test group.
#[cfg(unix)]
pub fn check(test: &str) {
    for case in cases(test) {
        controlled(&case);
    }
}
/// The inert definition used by rendering tests.
pub fn tool() -> maestro_tools::ToolDefinition<(), GrepToolInput, GrepToolDetails> {
    create_grep_tool_definition::<()>(
        "/work".into(),
        GrepToolOptions::default(),
        Arc::new(ToolsManager::new("/unused".into(), None)),
    )
}
/// Renderer-only detail fields; the full output record is not decoded here.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Details {
    match_limit_reached: Option<f64>,
    lines_truncated: Option<bool>,
    truncation: Option<crate::ls_support::RecordedTruncation>,
}
/// Reconstruct the typed renderer input.
pub fn rendered_result(value: &Value) -> maestro_agent::AgentToolResult<GrepToolDetails> {
    let details = value.get("details").map(|d| {
        let d: Details = serde_json::from_value(d.clone()).unwrap();
        GrepToolDetails {
            match_limit_reached: d.match_limit_reached,
            lines_truncated: d.lines_truncated,
            truncation: d.truncation.map(|cut| cut.result(None)),
        }
    });
    maestro_agent::AgentToolResult {
        content: serde_json::from_value(value["content"].clone()).unwrap(),
        details,
        terminate: None,
    }
}
/// Check the shared rendering driver against this tool's recorded rows.
pub fn check_rows(test: &str) {
    for case in cases(test) {
        let (rows, width) =
            crate::ls_support::render_case_as(&case, &keys(), tool(), rendered_result);
        crate::ls_support::assert_rows(&case, &rows, width);
    }
}
/// Construct the ordinary native search tree on every Unix host.
#[cfg(unix)]
pub fn tree() -> Scratch {
    let root = Scratch::new().unwrap();
    for (path, text) in [
        ("example.txt", "first line\nmatch line\nlast line"),
        (
            "context.txt",
            "before\nmatch one\nafter\nmiddle\nmatch two\nafter two",
        ),
        ("ignored.txt", "needle"),
        ("kept.txt", "needle"),
        (".gitignore", "ignored.txt\n"),
        (".secret/hidden.txt", "needle"),
        (".git/meta.txt", "needle"),
        ("node_modules/dependency.txt", "needle"),
        ("a/.gitignore", "local.txt\n"),
        ("a/local.txt", "needle"),
        ("b/local.txt", "needle"),
        ("a/deep/.gitignore", "secret.txt\n"),
        ("a/deep/secret.txt", "needle"),
        ("a/deep/kept.txt", "needle"),
        ("..hidden/a", "needle"),
        ("back\\slash", "needle"),
        ("case.txt", "Alpha\na.b\naXb"),
    ] {
        std::fs::create_dir_all(root.path().join(path).parent().unwrap()).unwrap();
        root.file(path, text.as_bytes());
    }
    root.file("invalid-content", b"hit\xff\n");
    // NUL stays on its own line: searcher output varies by host; controlled tests cover inline NUL.
    root.file("binary", b"needle\n\0tail\n");
    std::os::unix::fs::symlink("kept.txt", root.path().join("linked-file")).unwrap();
    std::os::unix::fs::symlink("a", root.path().join("linked-dir")).unwrap();
    root
}
/// Add the non-UTF-8 filename only when the host can store its bytes.
#[cfg(unix)]
pub fn add_invalid_filename(root: &Scratch) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let invalid = std::ffi::OsStr::from_bytes(b"invalid-\xff");
    if !root.keeps_distinct(&[invalid]).unwrap() {
        eprintln!(
            "only invalid-filename assertions need name bytes {:?}",
            invalid.as_bytes()
        );
        return false;
    }
    std::fs::write(root.path().join(invalid), b"needle\n").unwrap();
    true
}
/// Run real search through the wrapped factory and preserve absent details.
#[cfg(unix)]
pub fn native(root: &Scratch, params: Value) -> Result<Value, String> {
    let tool = create_grep_tool(
        root.text(),
        GrepToolOptions::default(),
        manager(root.text(), &Arc::new(AtomicUsize::new(0))),
    );
    run((tool.execute)("call".into(), params, None, None))
        .map(|r| result_json(&r.content, r.details))
        .map_err(|e| e.message)
}
/// Strictly consume a recorded native invocation and its outcome.
#[cfg(unix)]
pub fn check_native(case: &Case, root: &Scratch) {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Native {
        params: Value,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct NativeExpected {
        result: Option<Value>,
        error: Option<String>,
        marker_exists: Option<bool>,
    }
    assert_eq!(case.op, "native-rg");
    let input: Native = serde_json::from_value(crate::ls_support::substitute(
        &case.input,
        &[("$ROOT", &root.text())],
    ))
    .unwrap();
    let expected: NativeExpected = serde_json::from_value(crate::ls_support::substitute(
        &case.expected,
        &[("$ROOT", &root.text())],
    ))
    .unwrap();
    match native(root, input.params) {
        Ok(r) => assert_eq!(
            canon(&r),
            canon(&expected.result.unwrap()),
            "case {}",
            case.id
        ),
        Err(e) => assert_eq!(e, expected.error.unwrap(), "case {}", case.id),
    }
    if let Some(exists) = expected.marker_exists {
        assert_eq!(root.path().join("injection-marker").exists(), exists);
    }
}
/// A drop witness held for the lifetime of a pending adapter future.
#[cfg(unix)]
pub struct Dropped(pub Cancellation);
#[cfg(unix)]
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.abort();
    }
}
/// Replacement whose selected operation never completes.
#[cfg(unix)]
pub struct PendingOperations {
    pub read: bool,
    pub started: Cancellation,
    pub dropped: Cancellation,
}
#[cfg(unix)]
impl PendingOperations {
    /// Pending work acknowledges entry and owns its drop witness.
    fn pending<T: Send + 'static>(&self) -> BoxFuture<Result<T, DiagnosticErrorInfo>> {
        let started = self.started.clone();
        let guard = Dropped(self.dropped.clone());
        Box::pin(async move {
            let _guard = guard;
            started.abort();
            std::future::pending().await
        })
    }
}
#[cfg(unix)]
impl GrepOperations for PendingOperations {
    fn is_directory(&self, _path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>> {
        if self.read {
            Box::pin(async { Ok(true) })
        } else {
            self.pending()
        }
    }
    fn read_file(&self, _path: String) -> BoxFuture<Result<String, DiagnosticErrorInfo>> {
        self.pending()
    }
}
/// Install a producer connected to a parent-owned readiness channel.
#[cfg(unix)]
pub fn session(root: &Scratch, body: &str) -> tokio::net::UnixListener {
    let path = root.path().join("ready.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    executable(
        root,
        "rg",
        &format!(
            "#!/usr/bin/python3\nimport socket,os,signal,sys,json\ns=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)\ns.connect({})\n{body}\n",
            json!(path.to_str().unwrap())
        ),
    );
    listener
}
/// A wrapped invocation configured for a controlled producer.
#[cfg(unix)]
pub fn invocation(
    root: &Scratch,
    params: Value,
    signal: Option<Cancellation>,
) -> BoxFuture<Result<maestro_agent::AgentToolResult<Value>, DiagnosticErrorInfo>> {
    let tool = create_grep_tool(
        root.text(),
        GrepToolOptions::default(),
        manager(root.text(), &Arc::new(AtomicUsize::new(0))),
    );
    (tool.execute)("call".into(), params, signal, None)
}
