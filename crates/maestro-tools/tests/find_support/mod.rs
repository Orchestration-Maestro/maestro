//! Strict recorded file-search inputs and public execution/render drivers.
#![cfg(test)]
use crate::{
    display_support::{Case, search_cases, search_keys},
    ls_support::{
        RecordedTruncation, assert_rows, canon, check_reuse_as, check_theme_error_as, posix_value,
        render_case_as, result_json, with_home,
    },
    read_support::{Fixture, Scratch},
    runtime::run,
};
use futures_util::FutureExt;
use maestro_agent::AgentToolResult;
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo, Fetch};
use maestro_tools::{
    FindGlobOptions, FindOperations, FindToolDetails, FindToolInput, FindToolOptions, ToolsManager,
    create_find_tool, create_find_tool_definition, output::truncate::TruncationBy,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
/// Parse strict cases and reject repeated queries.
pub fn cases(test: &str) -> Vec<Case> {
    search_cases(include_str!("../fixtures/find_cases.json"), test)
}
/// Use the recorded palette rather than a hard-coded color assignment.
fn keys() -> Vec<String> {
    search_keys(include_str!("../fixtures/find_cases.json"))
}
/// Manager whose transport refuses and counts release fetches.
pub fn manager(bin: String, counter: &Arc<AtomicUsize>) -> Arc<ToolsManager> {
    let count = Arc::clone(counter);
    let fetch: Fetch = Arc::new(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(maestro_models::FetchError::Aborted) })
    });
    Arc::new(ToolsManager::new(bin, Some(fetch)))
}
/// Either replaceable operation stage.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Stage {
    Exists,
    Glob,
    Before,
}
/// Strict input for supplied operations.
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub wrapped: bool,
    pub params: Option<Value>,
    pub results: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub exists: Option<bool>,
    fail: Option<Stage>,
    abort: Option<Stage>,
    pending: Option<Stage>,
    pub ambient_cwd: Option<String>,
}
/// Supplied operations with an ordered call log and controlled cancellation.
pub struct Scripted {
    input: Input,
    calls: Mutex<Vec<Value>>,
    signal: Cancellation,
    started: Cancellation,
}
impl Scripted {
    /// Supplied operations over a strict input, sharing the caller's cancellation.
    pub fn new(input: Input, signal: &Cancellation) -> Self {
        Self {
            input,
            calls: Mutex::new(Vec::new()),
            signal: signal.clone(),
            started: Cancellation::new(),
        }
    }

    /// Calls made so far, in order.
    pub fn log(&self) -> Vec<Value> {
        self.calls.lock().unwrap().clone()
    }

    /// Record effects with values taken from the strict input.
    fn step<T: Send + 'static>(
        &self,
        stage: Stage,
        call: Value,
        value: T,
    ) -> BoxFuture<Result<T, DiagnosticErrorInfo>> {
        self.calls.lock().unwrap().push(call);
        if self.input.abort == Some(stage) {
            self.signal.abort();
        }
        if self.input.pending == Some(stage) {
            let started = self.started.clone();
            return Box::pin(async move {
                started.abort();
                std::future::pending().await
            });
        }
        let result = if self.input.fail == Some(stage) {
            Err(DiagnosticErrorInfo {
                name: Some("Error".into()),
                message: format!(
                    "Error: {} denied",
                    if stage == Stage::Exists {
                        "exists"
                    } else {
                        "glob"
                    }
                ),
                stack: None,
                code: None,
            })
        } else {
            Ok(value)
        };
        Box::pin(std::future::ready(result))
    }
}
impl FindOperations for Scripted {
    fn exists(&self, path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>> {
        self.step(
            Stage::Exists,
            json!(["exists", path]),
            self.input.exists.unwrap_or(true),
        )
    }
    fn glob(
        &self,
        pattern: String,
        cwd: String,
        options: FindGlobOptions,
    ) -> BoxFuture<Result<Vec<String>, DiagnosticErrorInfo>> {
        self.step(
            Stage::Glob,
            json!(["glob",pattern,cwd,{"ignore":options.ignore,"limit":options.limit}]),
            self.input
                .results
                .clone()
                .unwrap_or_else(|| vec!["/work/z".into(), "/work/a".into(), "/work/z".into()]),
        )
    }
}
/// Invoke the selected typed or wrapped factory with the same controlled effects.
async fn invoke(
    operations: &Arc<Scripted>,
    signal: &Cancellation,
    updates: &Arc<AtomicUsize>,
    fetches: &Arc<AtomicUsize>,
) -> Result<Value, DiagnosticErrorInfo> {
    let input = &operations.input;
    let cwd = input.cwd.clone().unwrap_or_else(|| "/work".into());
    let params = input
        .params
        .clone()
        .unwrap_or_else(|| json!({"pattern":"*.txt"}));
    let options = FindToolOptions {
        operations: Some(Arc::clone(operations) as Arc<dyn FindOperations>),
    };
    let bin = Scratch::new().unwrap();
    let tools_manager = manager(bin.text(), fetches);
    let count = Arc::clone(updates);
    if input.wrapped {
        let tool = create_find_tool(cwd, options, tools_manager);
        let update: maestro_agent::AgentToolUpdateCallback = Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        });
        (tool.execute)("call".into(), params, Some(signal.clone()), Some(update))
            .await
            .map(|r| result_json(&r.content, r.details))
    } else {
        let tool = create_find_tool_definition::<String>(cwd, options, tools_manager);
        let update: maestro_agent::AgentToolUpdateCallback<FindToolDetails> = Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        });
        let r = (tool.execute)(
            "call".into(),
            serde_json::from_value(params).unwrap(),
            Some(signal.clone()),
            Some(update),
            None,
        )
        .await?;
        Ok(result_json(
            &r.content,
            r.details.map(|d| serde_json::to_value(d).unwrap()),
        ))
    }
}
/// Observe a supplied-operation invocation, including its calls and progress.
pub fn custom(case: &Case) -> Value {
    assert_eq!(case.op, "custom");
    let input: Input = serde_json::from_value(with_home(&case.input)).unwrap();
    assert!(input.ambient_cwd.as_deref().is_none_or(|v| v == "removed"));
    let signal = Cancellation::new();
    if input.abort == Some(Stage::Before) {
        signal.abort();
    }
    let operations = Arc::new(Scripted::new(input, &signal));
    let updates = Arc::new(AtomicUsize::new(0));
    let fetches = Arc::new(AtomicUsize::new(0));
    let result = run(async {
        let mut future = std::pin::pin!(invoke(&operations, &signal, &updates, &fetches));
        if operations.input.pending.is_some() {
            assert!(future.as_mut().now_or_never().is_none());
            assert!(operations.started.is_aborted());
            signal.abort();
        }
        future.await
    });
    assert_eq!(fetches.load(Ordering::SeqCst), 0);
    let calls = operations.calls.lock().unwrap().clone();
    let mut observed = json!({"calls":calls,"updates":updates.load(Ordering::SeqCst)});
    match result {
        Ok(r) => observed["result"] = r,
        Err(e) => observed["error"] = json!(e.message),
    }
    observed
}
/// Run every supplied-operation case in a named test.
pub fn check_custom(test: &str) {
    for c in cases(test) {
        let mut actual = custom(&c);
        if c.expected.get("calls").is_none() {
            actual.as_object_mut().unwrap().shift_remove("calls");
        }
        if c.expected.get("error_kind").is_some() {
            assert!(actual.get("error").is_some());
            assert_eq!(actual["calls"], json!([]));
            assert_eq!(actual["updates"], json!(0));
            assert_eq!(c.expected["error_kind"], json!("required-cwd"));
        } else {
            // Windows reports `\` as its separator, so a recorded name holding one is a
            // path there, spelled with `/` like every other.
            assert_eq!(
                canon(&posix_value(&actual)),
                canon(&posix_value(&with_home(&c.expected))),
                "case {}",
                c.id
            );
        }
    }
}
/// Details read by result rendering, not a decoder for product output records.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Details {
    result_limit_reached: Option<f64>,
    truncation: Option<RecordedTruncation>,
}
/// Selected fields of a renderer input, with unknown members rejected.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedResult {
    content: Vec<maestro_models::UserBlock>,
    details: Option<Details>,
}
/// Build a typed result from the rendering fixture's selected fields.
fn result(value: &Value) -> AgentToolResult<FindToolDetails> {
    let recorded: RecordedResult = serde_json::from_value(value.clone()).unwrap();
    let details = recorded.details.map(|d| FindToolDetails {
        result_limit_reached: d.result_limit_reached,
        truncation: d.truncation.map(|c| c.result(Some(TruncationBy::Bytes))),
    });
    AgentToolResult {
        content: recorded.content,
        details,
        terminate: None,
    }
}
/// Public renderer definition with a poison native transport.
fn definition() -> maestro_tools::ToolDefinition<(), FindToolInput, FindToolDetails> {
    create_find_tool_definition(
        "/work".into(),
        FindToolOptions::default(),
        manager("/unused".into(), &Arc::new(AtomicUsize::new(0))),
    )
}
/// Compare each recorded render including component reuse and failure atomicity.
pub fn check_render(test: &str) {
    for c in cases(test) {
        match c.op.as_str() {
            "reuse" => check_reuse_as(
                &c,
                &keys(),
                definition(),
                &result(&json!({"content":[{"type":"text","text":"after"}]})),
                &json!({"pattern":"after"}),
            ),
            "theme-error" => check_theme_error_as(
                &c,
                &keys(),
                definition(),
                &result(&json!({"content":[{"type":"text","text":"after"}]})),
                &json!({"pattern":"after"}),
            ),
            "call" | "render" => {
                let (rows, width) = render_case_as(&c, &keys(), definition(), result);
                assert_rows(&c, &rows, width);
            }
            _ => panic!("unknown render operation"),
        }
    }
}
/// Strict inputs of the controlled native producer.
#[cfg(unix)]
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct Native {
    params: Option<Value>,
    cwd: Option<String>,
    records: Option<Vec<String>>,
    stderr: String,
    code: Option<i32>,
    signal_exit: Option<String>,
    unavailable: bool,
    unterminated: bool,
    spawn_error: bool,
    abort: Option<String>,
}
/// Install an executable via a child so no parent-owned writable descriptor survives.
#[cfg(unix)]
pub fn executable(root: &Scratch, name: &str, body: &str) {
    let source = root.file(&format!("{name}.source"), body.as_bytes());
    assert!(
        std::process::Command::new("install")
            .args(["-m", "0700", &source])
            .arg(root.path().join(name))
            .status()
            .unwrap()
            .success()
    );
}
/// Install an `fd` program that records its operands and return the file it writes.
#[cfg(unix)]
pub fn recorder(root: &Scratch) -> std::path::PathBuf {
    let args = root.path().join("args.json");
    executable(root, "fd", &native_script(&Native::default(), &args));
    args
}
/// Encode a producer's bytes and exit policy independently of the tool's operands.
#[cfg(unix)]
fn native_script(input: &Native, args: &std::path::Path) -> String {
    let records = input.records.as_ref().map_or_else(
        || "/work/z\0/work/a\0/work/z".to_owned(),
        |paths| paths.join("\0"),
    );
    let records = if records.is_empty() || input.unterminated {
        records
    } else {
        records + "\0"
    };
    let termination = input.signal_exit.as_ref().map_or(String::new(), |s| {
        assert_eq!(s, "SIGTERM");
        "os.kill(os.getpid(),signal.SIGTERM)".into()
    });
    format!(
        "#!/usr/bin/python3\nimport sys,json,os,signal\njson.dump(sys.argv[1:],open({},'w'))\nsys.stdout.buffer.write({}.encode())\nsys.stderr.buffer.write({}.encode())\n{termination}\nsys.exit({})\n",
        json!(args.to_str().unwrap()),
        json!(records),
        json!(input.stderr),
        input.code.unwrap_or(0)
    )
}
/// Invoke a controlled native program through the actual discovery and process owners.
#[cfg(unix)]
pub fn check_controlled(test: &str) {
    for c in cases(test).into_iter().filter(|c| c.id != "unavailable") {
        assert_eq!(c.op, "native-controlled");
        let input: Native = serde_json::from_value(c.input.clone()).unwrap();
        let root = Scratch::new().unwrap();
        let args = root.path().join("args.json");
        let body = native_script(&input, &args);
        if input.spawn_error {
            root.file("fd", b"not executable");
        } else {
            executable(&root, "fd", &body);
        }
        let fetches = Arc::new(AtomicUsize::new(0));
        let updates = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&updates);
        let update: maestro_agent::AgentToolUpdateCallback = Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        });
        let signal = Cancellation::new();
        if input.abort.is_some() {
            assert_eq!(input.abort.as_deref(), Some("ensure"));
            signal.abort();
        }
        assert!(!input.unavailable);
        let tool = create_find_tool(
            input.cwd.unwrap_or_else(|| "/work".into()),
            FindToolOptions::default(),
            manager(root.text(), &fetches),
        );
        let output = run((tool.execute)(
            "call".into(),
            input.params.unwrap_or_else(|| json!({"pattern":"*"})),
            Some(signal),
            Some(update),
        ));
        let mut actual = json!({"updates":updates.load(Ordering::SeqCst)});
        assert_eq!(fetches.load(Ordering::SeqCst), 0);
        match output {
            Ok(r) => actual["result"] = result_json(&r.content, r.details),
            Err(e) => {
                if c.expected.get("error_prefix").is_some() {
                    assert!(
                        e.message
                            .starts_with(c.expected["error_prefix"].as_str().unwrap())
                    );
                    actual["error_prefix"] = c.expected["error_prefix"].clone();
                } else {
                    actual["error"] = json!(e.message);
                }
            }
        }
        if c.expected.get("argv").is_some() && !input.spawn_error {
            let operands: Value = serde_json::from_slice(&std::fs::read(&args).unwrap()).unwrap();
            actual["argv"] = operands;
        }
        assert_eq!(canon(&actual), canon(&c.expected), "case {}", c.id);
    }
}
/// Run real fd; missing binaries fail instead of skipping.
pub fn native(root: &Scratch, pattern: &str, limit: Option<f64>) -> Result<Value, String> {
    let tool = create_find_tool(
        root.text(),
        FindToolOptions::default(),
        manager(root.text(), &Arc::new(AtomicUsize::new(0))),
    );
    let params = json!({"pattern":pattern,"limit":limit});
    run((tool.execute)("call".into(), params, None, None))
        .map(|r| result_json(&r.content, r.details))
        .map_err(|e| e.message)
}
/// Body of an untruncated native result.
pub fn text(result: &Value) -> &str {
    result["content"][0]["text"].as_str().unwrap()
}
