//! Recorded shell inputs driven through the public operation seam.
#![cfg(test)]

use crate::runtime::run_paused;
use maestro_agent::{AgentToolResult, AgentToolUpdateCallback};
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use maestro_tools::{
    BashError, BashExecOptions, BashExecResult, BashOperations, BashSpawnContext, BashToolDetails,
    BashToolInput, BashToolOptions, create_bash_tool, create_bash_tool_definition,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

/// The recorded fixture, including metadata and retained call sequences.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub metadata: Value,
    pub rows: Vec<Row>,
    pub sequences: Vec<Value>,
}
/// One independent invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub input: Input,
    pub expected: Expected,
}
/// Supplied operands and scripted operation events.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub command: Option<String>,
    pub prefix: Option<String>,
    pub shell_path: Option<String>,
    pub hook: Option<Hook>,
    pub hook_error: Option<String>,
    pub signal: Option<bool>,
    pub pre_abort: Option<bool>,
    pub updates: Option<bool>,
    pub timeout: Option<f64>,
    #[serde(default)]
    pub code: Status,
    pub wrapped: Option<bool>,
    pub error: Option<String>,
    #[serde(default)]
    pub events: Vec<Event>,
}
/// Default success and explicit absent status stay distinct fixture inputs.
#[derive(Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Status(pub Option<f64>);
impl Default for Status {
    fn default() -> Self {
        Self(Some(0.0))
    }
}
/// Complete hook replacement.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Hook {
    pub command: String,
    pub cwd: String,
    pub env: std::collections::BTreeMap<String, String>,
}
/// A single producer action.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub text: Option<String>,
    pub hex: Option<String>,
    pub repeat: Option<usize>,
    pub lines: Option<usize>,
    pub chatty: Option<usize>,
    pub advance: Option<u64>,
    pub abort: Option<bool>,
    pub observe: Option<String>,
}
/// Every retained observation is compared, including byte artifacts.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Expected {
    pub value: Option<Value>,
    pub error: Option<String>,
    pub updates: Vec<Value>,
    pub ops_trace: Vec<Value>,
    pub phases: Vec<Value>,
    pub hook_calls: usize,
    pub artifacts: Vec<Artifact>,
}
/// Raw artifact bytes and normalized identity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: String,
    pub hex: String,
}
/// Ordered observations made by the actual hook, operation and observer.
#[derive(Default)]
pub struct Observations {
    pub updates: Vec<Value>,
    pub ops: Vec<Value>,
    pub phases: Vec<Value>,
    pub hooks: usize,
}
/// Shared transcript, never held while calling external code.
pub type Log = Arc<Mutex<Observations>>;
/// An advance request with a producer acknowledgement.
type Advance = (u64, oneshot::Sender<()>);
/// A supplied operation with no tool execution policy.
pub struct Script {
    pub input: Arc<Input>,
    pub log: Log,
    pub advances: mpsc::UnboundedSender<Advance>,
}
impl BashOperations for Script {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        mut options: BashExecOptions<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<BashExecResult, BashError>> + Send + 'a>> {
        Box::pin(async move {
            let index = self.record_start(command, cwd, &options);
            for event in &self.input.events {
                self.event(event, &mut options).await;
            }
            if let Some(signal) = options.signal {
                self.log.lock().unwrap().ops[index]["signalAtEnd"] = json!(signal.is_aborted());
            }
            match self.input.error.as_deref() {
                Some(error) => Err(std::io::Error::other(error).into()),
                None => Ok(BashExecResult {
                    exit_code: self.input.code.0,
                }),
            }
        })
    }
}
impl Script {
    /// Observe forwarding at operation entry, before streaming.
    fn record_start(&self, command: &str, cwd: &str, options: &BashExecOptions<'_>) -> usize {
        let mut trace =
            json!({"command": command, "cwd": cwd, "signalPresent": options.signal.is_some()});
        if let Some(seconds) = options.timeout {
            trace["timeout"] = serde_json::from_str(ryu_js::Buffer::new().format(seconds)).unwrap();
        }
        if let Some(signal) = options.signal {
            trace["aborted"] = json!(signal.is_aborted());
        }
        if self.input.hook.is_some() {
            trace["env"] = json!(
                options
                    .env
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.to_str().unwrap(), v.as_ref().unwrap().to_str().unwrap()))
                    .collect::<std::collections::BTreeMap<_, _>>()
            );
        }
        let mut log = self.log.lock().unwrap();
        let count = log.updates.len();
        log.phases.push(json!(["exec", count]));
        let index = log.ops.len();
        log.ops.push(trace);
        index
    }
    /// Emit raw data, request controlled time, or observe the synchronous callback boundary.
    async fn event(&self, event: &Event, options: &mut BashExecOptions<'_>) {
        if let Some(text) = &event.text {
            (options.on_data)(text.repeat(event.repeat.unwrap_or(1)).as_bytes());
        }
        if let Some(hex) = &event.hex {
            (options.on_data)(&decode_hex(hex));
        }
        if let Some(lines) = event.lines {
            (options.on_data)(&numbered_lines(lines));
        }
        if let Some(chunks) = event.chatty {
            for n in 0..chunks {
                (options.on_data)(format!("line {n}\n").as_bytes());
            }
        }
        if event.abort == Some(true) {
            options.signal.unwrap().abort();
        }
        if let Some(ms) = event.advance {
            let (tx, rx) = oneshot::channel();
            self.advances.send((ms, tx)).unwrap();
            rx.await.unwrap();
        }
        if let Some(label) = &event.observe {
            let mut log = self.log.lock().unwrap();
            let count = log.updates.len();
            log.phases.push(json!([label, count]));
        }
    }
}
/// Decode independent recorded byte strings.
pub fn decode_hex(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
/// Serialize the public result without inventing absent members.
pub fn result_value<D: Serialize>(result: AgentToolResult<D>) -> Value {
    assert!(result.terminate.is_none());
    let mut value = json!({"content": result.content});
    if let Some(details) = result.details {
        value["details"] = serde_json::to_value(details).unwrap();
    }
    value
}
/// Read and validate the fixture once per selected test.
pub fn fixture() -> Fixture {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/agent-shell-cases.json")).unwrap();
    let mut ids = HashSet::new();
    let raw: Value =
        serde_json::from_str(include_str!("../fixtures/agent-shell-cases.json")).unwrap();
    let mut queries = Vec::new();
    for row in raw["rows"].as_array().unwrap() {
        assert!(!queries.contains(&&row["input"]), "duplicate query");
        queries.push(&row["input"]);
    }
    for row in &fixture.rows {
        assert!(ids.insert(&row.id), "duplicate row id");
    }
    fixture
}
/// Replay selected cases, each owning a fresh transcript and completed operation.
pub fn cases(ids: &[&str]) {
    let fixture = fixture();
    for id in ids {
        let row = fixture.rows.iter().find(|row| &row.id == id).unwrap();
        run_paused(check_row(row, None));
    }
}
/// Replay the existing controlled oracle with a caller-supplied public constructor.
pub fn cases_with(ids: &[&str], factory: &dyn Fn(BashToolOptions) -> maestro_agent::AgentTool) {
    let fixture = fixture();
    for id in ids {
        let row = fixture.rows.iter().find(|row| &row.id == id).unwrap();
        run_paused(check_row(row, Some(factory)));
    }
}
/// Invoke the public definition or wrapped tool and inspect every expected field.
async fn check_row(
    row: &Row,
    factory: Option<&dyn Fn(BashToolOptions) -> maestro_agent::AgentTool>,
) {
    let input = Arc::new(row.input.clone());
    let log = Arc::new(Mutex::new(Observations::default()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let ops = Arc::new(Script {
        input: input.clone(),
        log: log.clone(),
        advances: tx,
    });
    let options = BashToolOptions {
        operations: Some(ops),
        command_prefix: input.prefix.clone(),
        shell_path: input.shell_path.clone(),
        spawn_hook: hook(&input, &log),
    };
    let future = start(&input, options, &log, factory);
    let result = drive(future, &mut rx).await;
    check_observations(row, result, &log);
}

/// Build a public invocation; arbitrary context is consumed synchronously.
fn start(
    input: &Arc<Input>,
    options: BashToolOptions,
    log: &Log,
    factory: Option<&dyn Fn(BashToolOptions) -> maestro_agent::AgentTool>,
) -> Pin<Box<dyn Future<Output = Result<Value, DiagnosticErrorInfo>>>> {
    let signal = (input.signal != Some(false)).then(Cancellation::new);
    if input.pre_abort == Some(true) {
        signal.as_ref().unwrap().abort();
    }
    let params = BashToolInput {
        command: input
            .command
            .clone()
            .unwrap_or_else(|| "literal $COMMAND".into()),
        timeout: input.timeout,
    };
    let observer_log = log.clone();
    let update: AgentToolUpdateCallback<BashToolDetails> = Arc::new(move |value| {
        observer_log
            .lock()
            .unwrap()
            .updates
            .push(result_value(value));
    });
    if input.wrapped == Some(true) || factory.is_some() {
        let tool = match factory {
            Some(factory) => factory(options),
            None => create_bash_tool("/work".into(), "/maestro/bin".into(), options),
        };
        let observer_log = log.clone();
        let erased: AgentToolUpdateCallback = Arc::new(move |value| {
            observer_log
                .lock()
                .unwrap()
                .updates
                .push(result_value(value));
        });
        let execution = (tool.execute)(
            "call".into(),
            json!({"command":params.command, "timeout":params.timeout}),
            signal,
            (input.updates != Some(false)).then_some(erased),
        );
        Box::pin(async move { execution.await.map(result_value) })
            as Pin<Box<dyn Future<Output = Result<Value, DiagnosticErrorInfo>>>>
    } else {
        let tool = create_bash_tool_definition::<std::rc::Rc<()>>(
            "/work".into(),
            "/maestro/bin".into(),
            options,
        );
        let execution = (tool.execute)(
            "call".into(),
            params,
            signal,
            (input.updates != Some(false)).then_some(update),
            Some(std::rc::Rc::new(())),
        );
        Box::pin(async move { execution.await.map(result_value) })
    }
}

/// Assert each retained result, progress, forwarding and artifact field.
fn check_observations(row: &Row, result: Result<Value, DiagnosticErrorInfo>, log: &Log) {
    let mut observed = match result {
        Ok(value) => json!({"value":value}),
        Err(error) => json!({"error":error.message}),
    };
    let mut log = log.lock().unwrap();
    let artifacts = normalize(&mut observed, &mut log.updates);
    assert_eq!(
        observed.get("value"),
        row.expected.value.as_ref(),
        "{} value",
        row.id
    );
    assert_eq!(
        observed.get("error").and_then(Value::as_str),
        row.expected.error.as_deref(),
        "{} error",
        row.id
    );
    assert_eq!(log.updates, row.expected.updates, "{} updates", row.id);
    assert_eq!(log.ops, row.expected.ops_trace, "{} forwarding", row.id);
    assert_eq!(log.phases, row.expected.phases, "{} phases", row.id);
    assert_eq!(log.hooks, row.expected.hook_calls, "{} hook calls", row.id);
    assert_eq!(
        artifacts.len(),
        row.expected.artifacts.len(),
        "{} artifact count",
        row.id
    );
    for (bytes, expected) in artifacts.iter().zip(&row.expected.artifacts) {
        assert_eq!(expected.path, "<artifact>");
        assert_eq!(*bytes, decode_hex(&expected.hex), "{} raw artifact", row.id);
    }
}
/// Hook records its phase and receives the prefixed command before replacement.
fn hook(input: &Arc<Input>, log: &Log) -> Option<maestro_tools::BashSpawnHook> {
    if input.hook.is_none() && input.hook_error.is_none() {
        return None;
    }
    let input = input.clone();
    let log = log.clone();
    Some(Arc::new(move |context| {
        {
            let mut log = log.lock().unwrap();
            let count = log.updates.len();
            log.phases.push(json!(["hook", count]));
            log.hooks += 1;
        }
        let command = input.command.as_deref().unwrap_or("literal $COMMAND");
        let expected = input
            .prefix
            .as_ref()
            .filter(|prefix| !prefix.is_empty())
            .map_or_else(
                || command.to_owned(),
                |prefix| format!("{prefix}\n{command}"),
            );
        assert_eq!(context.command, expected);
        assert_eq!(context.cwd, "/work");
        if let Some(error) = &input.hook_error {
            return Err(DiagnosticErrorInfo::error(error));
        }
        let hook = input.hook.as_ref().unwrap();
        Ok(BashSpawnContext {
            command: hook.command.clone(),
            cwd: hook.cwd.clone(),
            env: hook
                .env
                .iter()
                .map(|(k, v)| (k.into(), Some(v.into())))
                .collect(),
        })
    }))
}
/// Complete execution; the producer cannot resume after an advance until the timer is polled.
async fn drive<F: Future>(future: F, rx: &mut mpsc::UnboundedReceiver<Advance>) -> F::Output {
    let mut future = std::pin::pin!(future);
    loop {
        tokio::select! {
            result = &mut future => return result,
            Some((ms, ack)) = rx.recv() => {
                tokio::time::advance(Duration::from_millis(ms)).await;
                assert!(std::future::poll_fn(|cx| std::task::Poll::Ready(future.as_mut().poll(cx))).await.is_pending());
                ack.send(()).unwrap();
            }
        }
    }
}
/// Normalize only real artifact identities, after reading all bytes from closed writers.
pub fn normalize(result: &mut Value, updates: &mut [Value]) -> Vec<Vec<u8>> {
    let mut paths = Vec::new();
    for value in std::iter::once(&*result).chain(updates.iter()) {
        if let Some(path) = value
            .pointer("/details/fullOutputPath")
            .or_else(|| value.pointer("/value/details/fullOutputPath"))
            .and_then(Value::as_str)
            && !paths.iter().any(|known| known == path)
        {
            paths.push(path.to_owned());
        }
    }
    let bytes = paths
        .iter()
        .map(|path| {
            assert_output_file_name(path);
            let bytes = std::fs::read(path).unwrap();
            std::fs::remove_file(path).unwrap();
            bytes
        })
        .collect();
    for value in std::iter::once(result).chain(updates.iter_mut()) {
        replace_paths(value, &paths);
    }
    bytes
}
/// Require the full-output file name: `maestro-bash-`, 16 lowercase hex digits, `.log`.
///
/// # Panics
/// Panics when the name differs.
pub fn assert_output_file_name(path: &str) {
    let filename = std::path::Path::new(path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let id = filename
        .strip_prefix("maestro-bash-")
        .unwrap()
        .strip_suffix(".log")
        .unwrap();
    assert_eq!(id.len(), 16);
    assert!(
        id.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
}
/// Keep the complete JSON tree while replacing only known artifact strings.
fn replace_paths(value: &mut Value, paths: &[String]) {
    match value {
        Value::String(text) => {
            for path in paths {
                *text = text.replace(path, "<artifact>");
            }
        }
        Value::Array(values) => {
            for value in values {
                replace_paths(value, paths);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_paths(value, paths);
            }
        }
        _ => {}
    }
}

mod immediate;
pub use immediate::Immediate;

/// Execute the retained failure-then-success hook sequence on one definition.
pub fn hook_sequence() {
    let fixture = fixture();
    let sequence = fixture
        .sequences
        .iter()
        .find(|value| value["id"] == "hook_failure_does_not_poison_definition")
        .unwrap();
    assert_eq!(sequence.as_object().unwrap().len(), 3);
    assert_eq!(sequence["input"]["hookFailsFirst"], true);
    assert_eq!(sequence["input"].as_object().unwrap().len(), 2);
    let phases = Arc::new(Mutex::new(Vec::new()));
    let hook_phases = phases.clone();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls = calls;
    let hook = Arc::new(move |context| {
        hook_phases.lock().unwrap().push("hook");
        if hook_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            Err(DiagnosticErrorInfo::error("hook rejected"))
        } else {
            Ok(context)
        }
    });
    let operation_phases = phases.clone();
    let ops = Immediate(Arc::new(move |_, _, _| {
        operation_phases.lock().unwrap().push("exec");
        Ok(BashExecResult {
            exit_code: Some(0.0),
        })
    }));
    let definition = create_bash_tool_definition::<()>(
        "/work".into(),
        "/maestro/bin".into(),
        BashToolOptions {
            operations: Some(Arc::new(ops)),
            spawn_hook: Some(hook),
            ..BashToolOptions::default()
        },
    );
    let mut results = Vec::new();
    for _ in 0..2 {
        let update_phases = phases.clone();
        let update = Arc::new(move |_| update_phases.lock().unwrap().push("update"));
        let result = run_paused((definition.execute)(
            "call".into(),
            BashToolInput {
                command: sequence["input"]["command"].as_str().unwrap().into(),
                timeout: None,
            },
            None,
            Some(update),
            None,
        ));
        results.push(match result {
            Ok(value) => json!({"value":result_value(value)}),
            Err(error) => json!({"error":error.message}),
        });
    }
    assert_eq!(
        json!({"results":results, "phases":*phases.lock().unwrap()}),
        sequence["expected"]
    );
}

/// The test that reruns `environment_sequence` in a child process.
#[cfg(unix)]
const SEQUENCE_TEST: &str = "bash_operations_receive_signal_timeout_and_environment";

/// Fresh inherited environment on successive calls, isolated from the test runner.
#[cfg(unix)]
pub fn environment_sequence() {
    let sequence = fixture()
        .sequences
        .into_iter()
        .find(|value| value["id"] == "fresh_spawn_context_per_call")
        .unwrap();
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(SEQUENCE_TEST)
            .unwrap()
            .nocapture()
            .case("*")
            .env_remove("ADDED")
            .envs(
                sequence["input"]["env"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(key, value)| (key, value.as_str().unwrap())),
            )
            .output()
            .unwrap();
        assert!(output.status.success());
        return;
    }
    assert_eq!(sequence.as_object().unwrap().len(), 3);
    assert_eq!(sequence["input"].as_object().unwrap().len(), 3);
    for (key, value) in sequence["input"]["env"].as_object().unwrap() {
        assert_eq!(std::env::var(key).unwrap(), value.as_str().unwrap());
    }
    let bases = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let definition = environment_definition(
        &bases,
        &calls,
        sequence["input"]["prefix"].as_str().unwrap().into(),
    );
    for command in sequence["input"]["commands"].as_array().unwrap() {
        let result = run_paused((definition.execute)(
            "call".into(),
            BashToolInput {
                command: command.as_str().unwrap().into(),
                timeout: None,
            },
            None,
            None,
            None,
        ))
        .unwrap();
        assert_eq!(
            result_value(result),
            json!({"content":[{"type":"text", "text":"(no output)"}]})
        );
    }
    assert_eq!(
        json!({"bases":*bases.lock().unwrap(), "calls":*calls.lock().unwrap()}),
        sequence["expected"]
    );
}
/// Inspect only the fixture's named inheritance entries; native inheritance may add others.
#[cfg(unix)]
fn selected_context(context: &BashSpawnContext) -> Value {
    let env = ["PATH", "KEEP", "ADDED"]
        .into_iter()
        .filter_map(|key| {
            context
                .env
                .get(std::ffi::OsStr::new(key))
                .map(|value| (key, value.as_ref().unwrap().to_str().unwrap()))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    json!({"command":context.command, "cwd":context.cwd, "env":env})
}

/// Independently generated native line-output expectation.
pub fn numbered_lines(lines: usize) -> Vec<u8> {
    use std::fmt::Write as _;
    let mut text = String::new();
    for line in 1..=lines {
        writeln!(&mut text, "{line}").unwrap();
    }
    text.into_bytes()
}

/// Construct the one retained definition for the environment sequence.
#[cfg(unix)]
fn environment_definition(
    bases: &Arc<Mutex<Vec<Value>>>,
    calls: &Arc<Mutex<Vec<Value>>>,
    prefix: String,
) -> maestro_tools::ToolDefinition<(), BashToolInput, BashToolDetails, maestro_tools::BashRenderState>
{
    let hook_bases = bases.clone();
    let hook = Arc::new(move |mut context: BashSpawnContext| {
        hook_bases.lock().unwrap().push(selected_context(&context));
        context.env.insert("ADDED".into(), Some("call-only".into()));
        Ok(context)
    });
    let operation_calls = calls.clone();
    let ops = Immediate(Arc::new(move |command, cwd, options| {
        operation_calls
            .lock()
            .unwrap()
            .push(selected_context(&BashSpawnContext {
                command: command.into(),
                cwd: cwd.into(),
                env: options.env.unwrap().clone(),
            }));
        Ok(BashExecResult {
            exit_code: Some(0.0),
        })
    }));
    create_bash_tool_definition::<()>(
        "/work".into(),
        "/maestro/bin".into(),
        BashToolOptions {
            operations: Some(Arc::new(ops)),
            command_prefix: Some(prefix),
            spawn_hook: Some(hook),
            ..BashToolOptions::default()
        },
    )
}
