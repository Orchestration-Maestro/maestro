//! Recorded ls cases, controlled directory operations and the execution and rendering drivers.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::display_support::{Env, NoLanguages, Setup, bind, context, padded_to, palette_of};
use crate::runtime::run;
use futures_util::FutureExt;
use maestro_agent::AgentToolResult;
use maestro_models::{BoxFuture, Cancellation, DiagnosticCode, DiagnosticErrorInfo, UserBlock};
use maestro_tools::{
    LsOperations, LsToolDetails, LsToolInput, LsToolOptions, ToolRenderPhase,
    ToolRenderResultOptions, create_ls_tool, create_ls_tool_definition,
    output::{
        render_utils::strip_ansi,
        truncate::{TruncationBy, TruncationResult},
    },
};
use maestro_tui::{ImageProtocol, Text, tui::ComponentHandle};
use serde::Deserialize;
use serde_json::{Map, Number, Value, json};
use std::{
    collections::HashSet,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Viewport width of recorded call rows.
pub const CALL_WIDTH: usize = 80;

pub use crate::display_support::Case;

/// The committed corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Locale of cases that name none.
    default_locale: String,
    /// Theme keys, whose index plus one is each key's palette color.
    theme_keys: Vec<String>,
    /// Recorded invocations.
    cases: Vec<Case>,
}

/// Parse the committed corpus.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/ls_cases.json")).unwrap()
}

/// The theme keys of the corpus.
pub fn theme_keys() -> Vec<String> {
    corpus().theme_keys
}

/// The locale of cases that name none, as a process locale such as `en_US.UTF-8`.
pub fn default_locale() -> String {
    process_locale(&corpus().default_locale)
}

/// A recorded locale such as `sv-SE` as a process locale.
pub fn process_locale(recorded: &str) -> String {
    format!("{}.UTF-8", recorded.replace('-', "_"))
}

/// The nonempty group of one test, after rejecting repeated queries.
pub fn cases(test: &str) -> Vec<Case> {
    let corpus = corpus();
    let mut queries = HashSet::new();
    for case in &corpus.cases {
        assert!(
            queries.insert((&case.test, &case.op, case.input.to_string())),
            "duplicate query {}",
            case.id
        );
    }
    let selected: Vec<_> = corpus
        .cases
        .into_iter()
        .filter(|case| case.test == test)
        .map(|mut case| {
            if cfg!(target_os = "macos") {
                case.expected = case.expected_macos.take().unwrap_or(case.expected);
            }
            case
        })
        .collect();
    assert!(!selected.is_empty(), "empty test group {test}");
    selected
}

/// The home directory both the tool and the expectations are written against.
fn home() -> String {
    maestro_tools::expand_path("~").unwrap()
}

/// Replace each placeholder of `vars` in every string of `value`.
pub fn substitute(value: &Value, vars: &[(&str, &str)]) -> Value {
    match value {
        Value::String(text) => {
            Value::String(vars.iter().fold(text.clone(), |text, (name, replacement)| {
                text.replace(name, replacement)
            }))
        }
        Value::Array(items) => Value::Array(items.iter().map(|i| substitute(i, vars)).collect()),
        Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(key, member)| (key.clone(), substitute(member, vars)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// `text` with what this host resolved from a recorded POSIX path spelled as the recording
/// does. Windows resolves `/work` on the drive of the working directory, as `D:\\work`.
pub fn posix_spelling(text: &str) -> String {
    let Some(root) = std::env::current_dir()
        .ok()
        .filter(|_| cfg!(windows))
        .and_then(|cwd| {
            cwd.ancestors()
                .last()
                .map(|root| root.display().to_string())
        })
    else {
        return text.to_owned();
    };
    text.replace(&root, "/").replace('\\', "/")
}

/// [`posix_spelling`] applied to every string of `value`.
pub fn posix_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(posix_spelling(text)),
        Value::Array(items) => Value::Array(items.iter().map(posix_value).collect()),
        Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(key, member)| (key.clone(), posix_value(member)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// [`posix_spelling`] applied to every field of recorded call rows.
pub fn posix_rows(rows: &[Vec<String>]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| row.iter().map(|field| posix_spelling(field)).collect())
        .collect()
}

/// Replace the `$HOME` placeholder in every string of `value`.
pub fn with_home(value: &Value) -> Value {
    substitute(value, &[("$HOME", &home())])
}

/// Convert every number to a double so integral doubles equal integers.
pub fn canon(value: &Value) -> Value {
    match value {
        Value::Number(number) => Value::Number(Number::from_f64(number.as_f64().unwrap()).unwrap()),
        Value::Array(items) => Value::Array(items.iter().map(canon).collect()),
        Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(key, member)| (key.clone(), canon(member)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Entry names: strings, and `{"rows": {"count", "suffix"}}` for the numbered names
/// `00000{suffix}` upwards.
pub fn names(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|item| match item.get("rows") {
            Some(rows) => {
                let suffix = rows["suffix"].as_str().unwrap();
                (0..rows["count"].as_u64().unwrap())
                    .map(|index| format!("{index:05}{suffix}"))
                    .collect()
            }
            None => vec![item.as_str().unwrap().to_owned()],
        })
        .collect()
}

/// Recorded text: a string, or `{"lines": names, "suffix"?: text}` for the lines joined by LF.
pub fn recorded_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        lines => {
            let suffix = lines.get("suffix").and_then(Value::as_str).unwrap_or("");
            names(&lines["lines"]).join("\n") + suffix
        }
    }
}

/// Recorded operation calls: `[operation, path]`, and `{"statEach": {"directory", "names"}}`
/// for one stat of each joined name.
fn recorded_calls(items: &Value) -> Vec<(String, String)> {
    let call = |operation: &str, path: &str| (operation.to_owned(), path.to_owned());
    items
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|item| match item.get("statEach") {
            Some(each) => {
                let directory = each["directory"].as_str().unwrap();
                names(&each["names"])
                    .iter()
                    .map(|name| call("stat", &format!("{directory}/{name}")))
                    .collect()
            }
            None => vec![call(item[0].as_str().unwrap(), item[1].as_str().unwrap())],
        })
        .collect()
}

/// Where a controlled operation misbehaves.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The existence probe.
    Exists,
    /// The directory check of the listed path.
    RootStat,
    /// The directory read.
    Readdir,
    /// The directory check of an entry.
    EntryStat,
}

/// When the call's signal is aborted.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum AbortAt {
    /// Before the call starts.
    Before,
    /// While the exists operation runs.
    Exists,
    /// While the root directory check runs.
    RootStat,
    /// While the directory read runs.
    Readdir,
    /// While the first entry check runs.
    EntryStat,
}

impl AbortAt {
    /// The operation stage, unless the abort precedes the call.
    fn stage(self) -> Option<Stage> {
        match self {
            Self::Before => None,
            Self::Exists => Some(Stage::Exists),
            Self::RootStat => Some(Stage::RootStat),
            Self::Readdir => Some(Stage::Readdir),
            Self::EntryStat => Some(Stage::EntryStat),
        }
    }
}

/// A recorded execution.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct ExecuteInput {
    /// Run through the wrapped factory instead of the definition.
    wrapped: bool,
    /// Names the directory reads back, as recorded.
    entries: Option<Value>,
    /// Entries whose check says directory.
    directories: Vec<String>,
    /// Arguments of the call.
    params: Option<Value>,
    /// Entries whose check fails.
    skip: Vec<String>,
    /// Stage that fails.
    fail: Option<Stage>,
    /// Whether the path exists.
    exists: Option<bool>,
    /// Whether the path is a directory.
    directory: Option<bool>,
    /// When the signal is aborted.
    abort: Option<AbortAt>,
    /// Stage that never completes.
    pending: Option<Stage>,
    /// Locale the process sorts with.
    locale: Option<String>,
    /// Working directory the tool is built over.
    cwd: Option<String>,
    /// State of the process working directory.
    #[cfg(unix)]
    ambient_cwd: Option<String>,
}

impl ExecuteInput {
    /// Whether one operation never completes on its own.
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// The process locale this case sorts under.
    pub fn locale(&self) -> String {
        self.locale
            .as_deref()
            .map_or_else(default_locale, process_locale)
    }

    /// Whether the process working directory is removed first.
    #[cfg(unix)]
    pub fn removes_cwd(&self) -> bool {
        self.ambient_cwd.as_deref() == Some("removed")
    }
}

/// What a recorded execution should produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecuteExpected {
    /// Result content and details.
    result: Option<Value>,
    /// Operation calls in order.
    calls: Option<Value>,
    /// Error message.
    error: Option<String>,
    /// Progress updates.
    updates: usize,
}

/// Controlled operations that log every call.
pub struct Scripted {
    /// Names the directory reads back.
    entries: Vec<String>,
    /// Entries whose check says directory.
    directories: HashSet<String>,
    /// Entries whose check fails.
    skip: HashSet<String>,
    /// Stage that fails.
    fail: Option<Stage>,
    /// Whether the path exists.
    exists: bool,
    /// Whether the path is a directory.
    directory: bool,
    /// Stage that aborts the signal.
    abort: Option<Stage>,
    /// Stage that never completes.
    pending: Option<Stage>,
    /// The call's signal.
    signal: Cancellation,
    /// Raised when a pending operation has started.
    started: Cancellation,
    /// Calls so far, as operation and path.
    calls: Mutex<Vec<(String, String)>>,
}

impl Scripted {
    /// The recorded input as operations over `signal`.
    pub fn new(input: &ExecuteInput, signal: &Cancellation) -> Self {
        let entries = input
            .entries
            .as_ref()
            .map_or_else(|| vec!["a".to_owned()], names);
        Self {
            entries,
            directories: input.directories.iter().cloned().collect(),
            skip: input.skip.iter().cloned().collect(),
            fail: input.fail,
            exists: input.exists.unwrap_or(true),
            directory: input.directory.unwrap_or(true),
            abort: input.abort.and_then(AbortAt::stage),
            pending: input.pending,
            signal: signal.clone(),
            started: Cancellation::new(),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// The calls made, in order.
    pub fn log(&self) -> Vec<(String, String)> {
        self.calls.lock().unwrap().clone()
    }

    /// Record the call, apply the stage's misbehavior and answer `value`.
    fn step<T: Send + 'static>(
        &self,
        stage: Stage,
        operation: &str,
        path: String,
        value: T,
    ) -> BoxFuture<Result<T, DiagnosticErrorInfo>> {
        self.calls
            .lock()
            .unwrap()
            .push((operation.to_owned(), path));
        if self.abort == Some(stage) {
            self.signal.abort();
        }
        if self.pending == Some(stage) {
            let started = self.started.clone();
            return Box::pin(async move {
                started.abort();
                std::future::pending().await
            });
        }
        let outcome = if self.fail == Some(stage) {
            let label = match stage {
                Stage::Readdir => "read",
                Stage::Exists => "exists",
                Stage::RootStat => "root_stat",
                Stage::EntryStat => "entry_stat",
            };
            Err(DiagnosticErrorInfo {
                name: Some("ControlledFailure".into()),
                message: format!("{label} denied"),
                stack: Some("controlled stack".into()),
                code: Some(DiagnosticCode::Text("E_CONTROL".into())),
            })
        } else {
            Ok(value)
        };
        Box::pin(std::future::ready(outcome))
    }

    /// The stage of a directory check, from whether it examines the listed path.
    fn stat_stage(&self, path: &str) -> Stage {
        if self.log().first().is_some_and(|(_, root)| root == path) {
            Stage::RootStat
        } else {
            Stage::EntryStat
        }
    }
}

impl LsOperations for Scripted {
    fn exists(&self, path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>> {
        self.step(Stage::Exists, "exists", path, self.exists)
    }

    fn stat(&self, path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>> {
        let stage = self.stat_stage(&path);
        // The entry name after the last separator; Windows joins with `\\`.
        let name = path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or_default()
            .to_owned();
        let answer = match stage {
            Stage::RootStat => self.directory,
            _ => self.directories.contains(&name),
        };
        if stage == Stage::EntryStat && self.skip.contains(&name) {
            self.calls.lock().unwrap().push(("stat".into(), path));
            let failure = DiagnosticErrorInfo {
                name: Some("ControlledFailure".into()),
                message: "entry denied".into(),
                stack: None,
                code: None,
            };
            return Box::pin(std::future::ready(Err(failure)));
        }
        self.step(stage, "stat", path, answer)
    }

    fn readdir(&self, path: String) -> BoxFuture<Result<Vec<String>, DiagnosticErrorInfo>> {
        self.step(Stage::Readdir, "readdir", path, self.entries.clone())
    }
}

/// A result as the JSON a caller observes, with `details` only when present.
pub fn result_json(content: &[UserBlock], details: Option<Value>) -> Value {
    let mut result = Map::new();
    result.insert("content".into(), serde_json::to_value(content).unwrap());
    result.extend(details.map(|details| ("details".to_owned(), details)));
    Value::Object(result)
}

/// The observed outcome of one call.
pub struct Outcome {
    /// The result as JSON, or the error message.
    pub result: Result<Value, String>,
    /// Operation calls in order.
    pub calls: Vec<(String, String)>,
    /// Progress updates received.
    pub updates: usize,
}

/// Poll `execution` to its pending operation, abort `signal` there, and require that the same
/// future then settles on its next poll.
fn observe_pending<T>(
    execution: impl std::future::Future<Output = T>,
    operations: &Scripted,
    signal: &Cancellation,
) -> T {
    let mut execution = std::pin::pin!(execution);
    assert!(execution.as_mut().now_or_never().is_none());
    assert!(operations.started.is_aborted());
    assert!(!signal.is_aborted());
    signal.abort();
    execution
        .as_mut()
        .now_or_never()
        .expect("aborting the signal settles a pending operation")
}

/// Call the tool built over `cwd` and `operations` with `params`, observing progress updates.
pub fn call_tool(
    input: &ExecuteInput,
    operations: &Arc<Scripted>,
    signal: &Cancellation,
) -> (Result<Value, String>, usize) {
    let cwd = input.cwd.clone().unwrap_or_else(|| "/work".into());
    let options = LsToolOptions {
        operations: Some(Arc::clone(operations) as Arc<dyn LsOperations>),
    };
    let params = input.params.clone().unwrap_or_else(|| json!({}));
    let updates = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&updates);
    let execution = async {
        if input.wrapped {
            let tool = create_ls_tool(cwd, options);
            let update: maestro_agent::AgentToolUpdateCallback = Arc::new(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            });
            let result = (tool.execute)("call".into(), params, Some(signal.clone()), Some(update))
                .await
                .map_err(|error| error.message);
            result.map(|result| result_json(&result.content, result.details))
        } else {
            let tool = create_ls_tool_definition::<()>(cwd, options);
            let update: maestro_agent::AgentToolUpdateCallback<LsToolDetails> =
                Arc::new(move |_| {
                    counter.fetch_add(1, Ordering::SeqCst);
                });
            let arguments: LsToolInput = serde_json::from_value(params).unwrap();
            let result = (tool.execute)(
                "call".into(),
                arguments,
                Some(signal.clone()),
                Some(update),
                None,
            )
            .await
            .map_err(|error| error.message);
            result.map(|result| {
                let details = result
                    .details
                    .map(|details| serde_json::to_value(details).unwrap());
                result_json(&result.content, details)
            })
        }
    };
    let result = if input.is_pending() {
        run(async { observe_pending(execution, operations, signal) })
    } else {
        run(execution)
    };
    (result, updates.load(Ordering::SeqCst))
}

/// Run one recorded execution over controlled operations.
pub fn execute(case: &Case) -> Outcome {
    let input: ExecuteInput = serde_json::from_value(case.input.clone()).unwrap();
    let signal = Cancellation::new();
    if input.abort == Some(AbortAt::Before) {
        signal.abort();
    }
    let operations = Arc::new(Scripted::new(&input, &signal));
    let (result, updates) = call_tool(&input, &operations, &signal);
    Outcome {
        result,
        calls: operations.log(),
        updates,
    }
}

/// The recorded execution input of `case`.
pub fn execute_input(case: &Case) -> ExecuteInput {
    serde_json::from_value(case.input.clone()).unwrap()
}

/// Assert the recorded progress updates and operation calls of `case`.
pub fn assert_effects(case: &Case, outcome: &Outcome) {
    let expected: ExecuteExpected = serde_json::from_value(case.expected.clone()).unwrap();
    let id = &case.id;
    assert_eq!(outcome.updates, expected.updates, "updates of {id}");
    if let Some(calls) = &expected.calls {
        let calls = recorded_calls(&with_home(calls));
        let observed: Vec<_> = outcome
            .calls
            .iter()
            .map(|(operation, path)| (operation.clone(), posix_spelling(path)))
            .collect();
        assert_eq!(observed, calls, "calls of {id}");
    }
}

/// Assert that the observed outcome of `case` is the recorded one.
pub fn assert_outcome(case: &Case, outcome: &Outcome) {
    let expected: ExecuteExpected = serde_json::from_value(case.expected.clone()).unwrap();
    let id = &case.id;
    assert_effects(case, outcome);
    match (&outcome.result, &expected.result, &expected.error) {
        (Ok(actual), Some(result), None) => {
            let mut result = with_home(result);
            for block in result["content"].as_array_mut().unwrap() {
                block["text"] = Value::String(recorded_text(&block["text"]));
            }
            if let Some(cut) = result.pointer_mut("/details/truncation/content") {
                *cut = Value::String(recorded_text(cut));
            }
            assert_eq!(
                canon(&posix_value(actual)),
                canon(&result),
                "result of {id}"
            );
        }
        (Err(actual), None, Some(error)) => {
            assert_eq!(
                &posix_spelling(actual),
                &with_home(&json!(error)),
                "error of {id}"
            );
        }
        (observed, recorded, _) => panic!(
            "case {id} observed {} but records {}",
            observed.as_ref().map_or("an error", |_| "a result"),
            recorded.as_ref().map_or("an error", |_| "a result"),
        ),
    }
}

/// Run `case` and assert the recorded outcome.
pub fn check_execute(case: &Case) {
    assert_outcome(case, &execute(case));
}

/// A recorded render context.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct ContextInput {
    /// Keys bound to the expand action; absent means the default `ctrl+o`.
    binding: Option<Vec<String>>,
    /// Whether the row is expanded.
    expanded: bool,
    /// Image protocol of the terminal.
    images: Option<String>,
    /// Whether the result is an error.
    is_error: bool,
    /// Whether images are shown; absent means they are.
    show_images: Option<bool>,
    /// Whether execution has started.
    execution_started: Option<bool>,
    /// Whether the arguments are complete.
    args_complete: Option<bool>,
    /// Palette shift of the published theme.
    theme_shift: u8,
}

impl ContextInput {
    /// The expand keys this context binds.
    fn keys(&self) -> Vec<String> {
        self.binding
            .clone()
            .unwrap_or_else(|| vec!["ctrl+o".to_owned()])
    }

    /// The renderer setup this context describes.
    fn setup(&self) -> Setup {
        let images = match self.images.as_deref() {
            Some("kitty") => Some(ImageProtocol::Kitty),
            Some(other) => panic!("unknown image protocol {other}"),
            None => None,
        };
        Setup {
            readme: String::new(),
            expanded: self.expanded,
            cwd: "/work".into(),
            images,
            show_images: self.show_images.unwrap_or(true),
            is_error: self.is_error,
            phase: ToolRenderPhase {
                execution_started: self.execution_started.unwrap_or(false),
                args_complete: self.args_complete.unwrap_or(false),
                is_partial: false,
            },
            shift: self.theme_shift,
        }
    }
}

/// Build the recorded setup with the selected renderer's palette keys.
fn environment(input: &ContextInput, keys: &[String]) -> (Setup, Env) {
    (
        input.setup(),
        Env::with(
            Rc::new(palette_of(keys, input.theme_shift)),
            Rc::new(NoLanguages),
        ),
    )
}

/// A recorded call rendering.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallInput {
    /// The call's raw arguments.
    args: Value,
    /// The render context.
    context: ContextInput,
}

/// The truncation fields the result renderer reads.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordedTruncation {
    /// Whether a limit required selection.
    pub truncated: bool,
    /// Byte budget.
    pub max_bytes: usize,
}
impl RecordedTruncation {
    /// The typed truncation with `truncated_by` as the tool reports it.
    pub fn result(self, truncated_by: Option<TruncationBy>) -> TruncationResult {
        TruncationResult {
            content: String::new(),
            truncated: self.truncated,
            max_bytes: self.max_bytes,
            truncated_by,
            total_lines: 0,
            total_bytes: 0,
            output_lines: 0,
            output_bytes: 0,
            last_line_partial: false,
            first_line_exceeds_limit: false,
            max_lines: 0,
        }
    }
}

/// Recorded result details.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordedDetails {
    /// The entry limit that stopped the listing.
    entry_limit_reached: Option<f64>,
    /// The byte truncation.
    truncation: Option<RecordedTruncation>,
}

/// A recorded result object as the ls tool's typed result.
fn ls_result(result: &Value) -> AgentToolResult<LsToolDetails> {
    let content: Vec<UserBlock> = serde_json::from_value(result["content"].clone()).unwrap();
    let details = result.get("details").map(|details| {
        let recorded: RecordedDetails = serde_json::from_value(details.clone()).unwrap();
        LsToolDetails {
            entry_limit_reached: recorded.entry_limit_reached,
            truncation: recorded
                .truncation
                .map(|cut| cut.result(Some(TruncationBy::Bytes))),
        }
    });
    AgentToolResult {
        content,
        details,
        terminate: None,
    }
}

/// A recorded result rendering.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderInput {
    /// The tool result.
    result: Value,
    /// The result options.
    options: OptionsInput,
    /// The render context.
    context: ContextInput,
    /// Viewport width.
    width: usize,
}

/// Recorded result options.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OptionsInput {
    /// Whether the full result is requested.
    expanded: bool,
    /// Whether the result is still arriving.
    is_partial: bool,
}

/// The default ls tool whose renderers are under test.
fn tool() -> maestro_tools::ToolDefinition<(), LsToolInput, LsToolDetails> {
    create_ls_tool_definition::<()>("/work".into(), LsToolOptions::default())
}

/// Render a recorded call or result and return its rows.
pub fn render_case_as<D: serde::Serialize>(
    case: &Case,
    keys: &[String],
    tool: maestro_tools::ToolDefinition<(), impl Send, D>,
    result: impl Fn(&Value) -> AgentToolResult<D>,
) -> (Vec<String>, usize) {
    if case.op == "call" {
        let input: CallInput = serde_json::from_value(case.input.clone()).unwrap();
        let (setup, env) = environment(&input.context, keys);
        let _bindings = bind(&input.context.keys());
        let args = with_home(&input.args);
        let ctx = context(&args, &setup, &env, None);
        let row = (tool.render_call.unwrap())(Some(&args), &env.live, &ctx).unwrap();
        return (row.render(CALL_WIDTH), CALL_WIDTH);
    }
    let input: RenderInput = serde_json::from_value(case.input.clone()).unwrap();
    let (setup, env) = environment(&input.context, keys);
    let _bindings = bind(&input.context.keys());
    let options = ToolRenderResultOptions {
        expanded: input.options.expanded,
        is_partial: input.options.is_partial,
    };
    let args = Value::Null;
    let ctx = context(&args, &setup, &env, None);
    let row = crate::display_support::native(
        &(tool.render_result.unwrap()),
        &maestro_tools::output::render_utils::ToolRenderResultLike::from(&result(&input.result)),
        options,
        &env.live,
        &ctx,
    )
    .unwrap();
    (row.render(input.width), input.width)
}

/// A recorded row as the toolkit lays it out at `width`: a row with no text is unstyled spaces.
fn laid_out(recorded: &Value, width: usize) -> String {
    let row = recorded.as_str().unwrap();
    if strip_ansi(row).unwrap().chars().all(|scalar| scalar == ' ') {
        " ".repeat(width)
    } else {
        padded_to(row, width)
    }
}

/// Assert that the rows of a recorded call or result are the expected ones, padded to `width`.
pub fn assert_rows(case: &Case, actual: &[String], width: usize) {
    let expected: Vec<String> = case
        .expected
        .as_array()
        .unwrap()
        .iter()
        .map(|row| laid_out(row, width))
        .collect();
    assert_eq!(actual, expected, "case {}", case.id);
}

/// Run every case of `test`, comparing rows.
pub fn check_rows(test: &str) {
    for case in cases(test) {
        let (rows, width) = render_case(&case);
        assert_rows(&case, &rows, width);
    }
}

/// A recorded text-reuse case: the row slot and the arguments or text that replace its content.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotInput {
    /// Which renderer is exercised: `call` or `result`.
    slot: String,
}

/// Render the replacement for a slot over `previous`.
pub fn render_slot_as<D: serde::Serialize>(
    input: (
        maestro_tools::ToolDefinition<(), impl Send, D>,
        &AgentToolResult<D>,
        &Value,
    ),
    slot: &str,
    theme: &maestro_theme::LiveTheme,
    env: &Env,
    previous: &ComponentHandle,
) -> Result<ComponentHandle, DiagnosticErrorInfo> {
    let setup = Setup::from(&json!({"readme": "", "bindings": []}));
    let (tool, result, args) = input;
    if slot == "call" {
        let ctx = context(args, &setup, env, Some(Rc::clone(previous)));
        (tool.render_call.unwrap())(Some(args), theme, &ctx)
    } else {
        let options = ToolRenderResultOptions {
            expanded: false,
            is_partial: false,
        };
        let ctx = context(&Value::Null, &setup, env, Some(Rc::clone(previous)));
        crate::display_support::native(
            &(tool.render_result.unwrap()),
            &maestro_tools::output::render_utils::ToolRenderResultLike::from(result),
            options,
            theme,
            &ctx,
        )
    }
}

/// Run a recorded reuse case: the replacement is the same component and shows the new text.
pub fn check_reuse_as<D: serde::Serialize>(
    case: &Case,
    keys: &[String],
    tool: maestro_tools::ToolDefinition<(), impl Send, D>,
    result: &AgentToolResult<D>,
    args: &Value,
) {
    let input: SlotInput = serde_json::from_value(case.input.clone()).unwrap();
    let env = Env::with(Rc::new(palette_of(keys, 0)), Rc::new(NoLanguages));
    let previous: ComponentHandle = Rc::new(Text::new("before".into(), 0, 0, None));
    let row = render_slot_as(
        (tool, result, args),
        &input.slot,
        &env.live,
        &env,
        &previous,
    )
    .unwrap();
    assert!(Rc::ptr_eq(&row, &previous), "case {}", case.id);
    let expected = &case.expected;
    assert_eq!(expected["same"], json!(true));
    let rows = previous.render(CALL_WIDTH);
    let lines: Vec<String> = expected["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| laid_out(row, CALL_WIDTH))
        .collect();
    assert_eq!(rows, lines, "case {}", case.id);
}

/// Run a recorded theme-error case: the renderer fails and the previous component keeps its text.
pub fn check_theme_error_as<D: serde::Serialize>(
    case: &Case,
    keys: &[String],
    tool: maestro_tools::ToolDefinition<(), impl Send, D>,
    result: &AgentToolResult<D>,
    args: &Value,
) {
    let input: SlotInput = serde_json::from_value(case.input.clone()).unwrap();
    let missing = if input.slot == "call" {
        "error"
    } else {
        "toolOutput"
    };
    let keys: Vec<String> = keys.iter().filter(|&key| key != missing).cloned().collect();
    let env = Env::with(Rc::new(palette_of(&keys, 0)), Rc::new(NoLanguages));
    let previous: ComponentHandle = Rc::new(Text::new("before".into(), 0, 0, None));
    let Err(error) = render_slot_as(
        (tool, result, args),
        &input.slot,
        &env.live,
        &env,
        &previous,
    ) else {
        panic!("case {} must fail", case.id);
    };
    assert_eq!(error.message, case.expected["error"].as_str().unwrap());
    let lines: Vec<String> = case.expected["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| padded_to(row.as_str().unwrap(), CALL_WIDTH))
        .collect();
    assert_eq!(previous.render(CALL_WIDTH), lines, "case {}", case.id);
}

/// A recorded listing of a real directory.
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeInput {
    /// Path to list, written against `$ROOT`.
    path: String,
}

/// Run a recorded listing of the real directory `root` and assert the recorded outcome.
#[cfg(unix)]
pub fn check_native(case: &Case, root: &str) {
    let input: NativeInput = serde_json::from_value(case.input.clone()).unwrap();
    let tool = create_ls_tool(root.into(), LsToolOptions::default());
    let params = json!({"path": input.path.replace("$ROOT", root)});
    let outcome = run((tool.execute)("call".into(), params, None, None));
    let expected = substitute(&case.expected, &[("$ROOT", root)]);
    let observed = match outcome {
        Ok(result) => result_json(&result.content, result.details),
        Err(error) => json!({"error": error.message}),
    };
    assert_eq!(observed, expected, "case {}", case.id);
}

/// Check listing component reuse with the recorded palette.
pub fn check_reuse(case: &Case) {
    check_reuse_as(
        case,
        &theme_keys(),
        tool(),
        &ls_result(&json!({"content":[{"type":"text","text":"after"}]})),
        &json!({"path":"after"}),
    );
}
/// Check listing theme failure with the recorded palette.
pub fn check_theme_error(case: &Case) {
    check_theme_error_as(
        case,
        &theme_keys(),
        tool(),
        &ls_result(&json!({"content":[{"type":"text","text":"after"}]})),
        &json!({"path":"after"}),
    );
}

/// Render a directory listing through the shared fixture driver.
pub fn render_case(case: &Case) -> (Vec<String>, usize) {
    render_case_as(case, &theme_keys(), tool(), ls_result)
}
