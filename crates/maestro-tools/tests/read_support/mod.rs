//! Controlled read operations, temporary files and result projections for the read tests.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::runtime::run;
use maestro_agent::AgentToolResult;
use maestro_models::{
    BoxFuture, Cancellation, DiagnosticErrorInfo, Model, ModelCost, ModelInput, UserBlock,
};
use maestro_tools::{
    ReadContext, ReadOperations, ReadToolDetails, ReadToolInput, ReadToolOptions, ToolDefinition,
    create_read_tool_definition,
};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicUsize, Ordering},
};

pub use maestro_test_directory::Scratch;

/// Transport counting acquisition attempts, which never succeeds.
///
/// Only the unix search-program tests consume it, so other hosts would see it unused.
#[cfg(unix)]
pub fn counting_fetch(calls: &Arc<AtomicUsize>) -> maestro_models::Fetch {
    let calls = Arc::clone(calls);
    Arc::new(move |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(maestro_models::FetchError::Aborted) })
    })
}

/// Native fixture spellings callers need on top of the shared directory owner.
pub trait Fixture {
    /// The directory as text.
    fn text(&self) -> String;

    /// Write `bytes` at `name` below the directory and return its text path.
    fn file(&self, name: &str, bytes: &[u8]) -> String;
}

impl Fixture for Scratch {
    fn text(&self) -> String {
        self.path().to_str().unwrap().to_owned()
    }

    fn file(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().to_owned()
    }
}

/// The three operations a read performs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Readability probe.
    Access,
    /// Image detection.
    Mime,
    /// Byte read.
    Read,
}

/// What a controlled operation does when called.
#[derive(Clone, Copy)]
pub enum Behavior {
    /// Return its configured value.
    Succeed,
    /// Fail with the stage's configured diagnostic.
    Fail,
    /// Abort the read's signal, then complete normally.
    AbortThenSucceed,
    /// Never complete, counting polls and drops of the returned future.
    Pend,
    /// Complete normally, aborting the read's signal when the future is dropped.
    AbortWhenDropped,
}

/// A future that never completes and records being polled and dropped.
struct PendProbe {
    /// Polls so far.
    polled: Arc<AtomicUsize>,
    /// Drops so far.
    dropped: Arc<AtomicUsize>,
}

impl std::future::Future for PendProbe {
    type Output = ();
    fn poll(self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        self.polled.fetch_add(1, Ordering::SeqCst);
        std::task::Poll::Pending
    }
}

impl Drop for PendProbe {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/// A ready value whose future aborts a signal when dropped.
struct AbortOnDrop<T> {
    /// The value yielded once.
    value: Option<T>,
    /// Signal aborted on drop.
    signal: Cancellation,
}

impl<T: Unpin> std::future::Future for AbortOnDrop<T> {
    type Output = Result<T, DiagnosticErrorInfo>;
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::task::Poll::Ready(Ok(self.value.take().unwrap()))
    }
}

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.signal.abort();
    }
}

/// Controlled operations that log every call.
pub struct ScriptedOps {
    /// Bytes returned by the read.
    pub bytes: Vec<u8>,
    /// Detection result.
    pub mime: Option<String>,
    /// Stage whose behavior is overridden.
    pub at: Stage,
    /// Behavior at `at`.
    pub behavior: Behavior,
    /// Signal aborted by the abort behaviors.
    pub signal: Option<Cancellation>,
    /// Calls made so far, as `stage:path`.
    pub calls: Mutex<Vec<String>>,
    /// Polls of the pending future.
    pub polled: Arc<AtomicUsize>,
    /// Drops of the pending future.
    pub dropped: Arc<AtomicUsize>,
    /// Runs when detection is called, before it answers.
    pub on_detect: Option<Box<dyn Fn() + Send + Sync>>,
}

impl ScriptedOps {
    /// Operations serving `bytes` with no detected image type.
    pub fn new(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
            mime: None,
            at: Stage::Access,
            behavior: Behavior::Succeed,
            signal: None,
            calls: Mutex::new(Vec::new()),
            polled: Arc::default(),
            dropped: Arc::default(),
            on_detect: None,
        }
    }

    /// Operations that fail or abort at `stage`.
    pub fn at(mut self, stage: Stage, behavior: Behavior, signal: Option<&Cancellation>) -> Self {
        (self.at, self.behavior, self.signal) = (stage, behavior, signal.cloned());
        self
    }

    /// Operations whose detection reports `mime`.
    pub fn with_mime(mut self, mime: &str) -> Self {
        self.mime = Some(mime.to_owned());
        self
    }

    /// The diagnostic a failing stage reports.
    pub fn failure(stage: Stage) -> DiagnosticErrorInfo {
        DiagnosticErrorInfo {
            name: Some("ControlledFailure".into()),
            message: format!("{stage:?} failed"),
            stack: Some("controlled stack".into()),
            code: Some(maestro_models::DiagnosticCode::Text("E_CONTROL".into())),
        }
    }

    /// The calls made, in order.
    pub fn log(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    /// Log the call and apply this stage's behavior.
    fn step<T: Send + Unpin + 'static>(
        &self,
        stage: Stage,
        path: &str,
        value: T,
    ) -> BoxFuture<Result<T, DiagnosticErrorInfo>> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:{path}", format!("{stage:?}").to_lowercase()));
        let behavior = if stage == self.at {
            self.behavior
        } else {
            Behavior::Succeed
        };
        if matches!(behavior, Behavior::AbortThenSucceed) {
            self.signal.as_ref().unwrap().abort();
        }
        match behavior {
            Behavior::Fail => Box::pin(std::future::ready(Err(Self::failure(stage)))),
            Behavior::Pend => {
                let probe = PendProbe {
                    polled: Arc::clone(&self.polled),
                    dropped: Arc::clone(&self.dropped),
                };
                Box::pin(async move {
                    probe.await;
                    Ok(value)
                })
            }
            Behavior::AbortWhenDropped => Box::pin(AbortOnDrop {
                value: Some(value),
                signal: self.signal.clone().unwrap(),
            }),
            Behavior::Succeed | Behavior::AbortThenSucceed => {
                Box::pin(std::future::ready(Ok(value)))
            }
        }
    }
}

impl ReadOperations for ScriptedOps {
    fn read_file(&self, path: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>> {
        self.step(Stage::Read, &path, self.bytes.clone())
    }

    fn access(&self, path: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.step(Stage::Access, &path, ())
    }

    fn detect_image_mime_type(
        &self,
        path: String,
    ) -> BoxFuture<Result<Option<String>, DiagnosticErrorInfo>> {
        if let Some(hook) = &self.on_detect {
            hook();
        }
        self.step(Stage::Mime, &path, self.mime.clone())
    }
}

/// A caller context whose model can be replaced while a read is running.
#[derive(Clone)]
pub struct SharedModel(pub Arc<RwLock<Option<Arc<Model>>>>);

impl ReadContext for SharedModel {
    fn model(&self) -> Option<Arc<Model>> {
        self.0.read().unwrap().clone()
    }
}

/// A catalog-independent model accepting `input`.
pub fn model(input: Vec<ModelInput>) -> Arc<Model> {
    Arc::new(Model {
        id: "fixture".into(),
        name: "Fixture".into(),
        api: "fixture".into(),
        provider: "fixture".into(),
        base_url: "https://example.invalid".into(),
        reasoning: false,
        thinking_level_map: None,
        input,
        cost: ModelCost::default(),
        context_window: 100.0,
        max_tokens: 20.0,
        headers: None,
        compat: None,
    })
}

/// Read arguments with the given operands.
pub fn input(path: &str, offset: Option<f64>, limit: Option<f64>) -> ReadToolInput {
    ReadToolInput {
        path: path.into(),
        offset,
        limit,
    }
}

/// A read tool over `ops` with default options.
pub fn tool_over(ops: &Arc<ScriptedOps>) -> ToolDefinition<(), ReadToolInput, ReadToolDetails> {
    create_read_tool_definition(
        "/work".into(),
        ReadToolOptions {
            auto_resize_images: None,
            operations: Some(Arc::clone(ops) as Arc<dyn ReadOperations>),
        },
    )
}

/// Execute one read to completion.
pub fn call<C: ReadContext + Send + 'static>(
    tool: &ToolDefinition<C, ReadToolInput, ReadToolDetails>,
    args: ReadToolInput,
    signal: Option<Cancellation>,
    context: Option<C>,
) -> Result<AgentToolResult<ReadToolDetails>, DiagnosticErrorInfo> {
    run((tool.execute)("call".into(), args, signal, None, context))
}

/// Read `bytes` as text through scripted operations and return the output text.
pub fn read_text(bytes: &[u8], offset: Option<f64>, limit: Option<f64>) -> Result<String, String> {
    let ops = Arc::new(ScriptedOps::new(bytes));
    call(
        &tool_over(&ops),
        input("/work/f", offset, limit),
        None,
        None,
    )
    .map(|result| only_text(&result))
    .map_err(|error| error.message)
}

/// The single text block of a result.
pub fn only_text(result: &AgentToolResult<ReadToolDetails>) -> String {
    match result.content.as_slice() {
        [UserBlock::Text(text)] => text.text.clone(),
        other => panic!("expected one text block, got {}", other.len()),
    }
}

/// A result's content as the JSON the model receives.
pub fn content_json<D>(result: &AgentToolResult<D>) -> Value {
    serde_json::to_value(&result.content).unwrap()
}

/// A text block's JSON.
pub fn text_json(text: &str) -> Value {
    json!({"type": "text", "text": text})
}

/// Operations without image detection, using the trait's default.
pub struct NoDetector(pub Vec<u8>);

impl ReadOperations for NoDetector {
    fn read_file(&self, _: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>> {
        Box::pin(std::future::ready(Ok(self.0.clone())))
    }

    fn access(&self, _: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(std::future::ready(Ok(())))
    }
}

/// Encode a blank image of the given format.
pub fn blank_image(width: u32, height: u32, format: image::ImageOutputFormat) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::new(width, height))
        .write_to(&mut out, format)
        .unwrap();
    out.into_inner()
}

/// Run the child-process half of a test: spawn it once per mode, or report that this is the child.
///
/// Returns true only inside the child spawned for `mode`; the parent asserts the child passed.
pub fn isolated(test: &str, mode: &str, envs: &[(&str, Option<&str>)]) -> bool {
    let envs: Vec<_> = envs
        .iter()
        .map(|(key, value)| (*key, value.map(std::ffi::OsStr::new)))
        .collect();
    isolated_os(test, mode, &envs)
}

/// [`isolated`] with environment values that need not be text.
///
/// The parent also checks that the selected child test ran and passed.
pub fn isolated_os(test: &str, mode: &str, envs: &[(&str, Option<&std::ffi::OsStr>)]) -> bool {
    if maestro_test_process::selection().is_child() {
        return maestro_test_process::selection().runs_case(mode);
    }
    isolated_os_output(test, mode, envs);
    false
}

/// Run a passing isolated child and retain its stdout and stderr for assertions.
pub fn isolated_os_output(
    test: &str,
    mode: &str,
    envs: &[(&str, Option<&std::ffi::OsStr>)],
) -> std::process::Output {
    isolated_output(test, mode, |mut command| {
        for (key, value) in envs {
            command = match value {
                Some(value) => command.env(key, value),
                None => command.env_remove(key),
            };
        }
        command
    })
}

/// Run a passing isolated child with its temporary directory set to `root`.
pub fn isolated_temp_dir(test: &str, mode: &str, root: &std::path::Path) {
    isolated_output(test, mode, |command| command.temp_dir(root));
}

/// Run a passing isolated child configured by `configure` and retain its output.
fn isolated_output(
    test: &str,
    mode: &str,
    configure: impl FnOnce(maestro_test_process::TestProcess) -> maestro_test_process::TestProcess,
) -> std::process::Output {
    let command = configure(
        maestro_test_process::TestProcess::current(test)
            .unwrap()
            .nocapture()
            .case(mode),
    );
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "child {mode} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// A separately reaped process holds the shell's stdout across root exit.
#[cfg(target_os = "linux")]
pub async fn pipe_holder(pid: u32) -> tokio::process::Child {
    use tokio::io::AsyncReadExt;
    let mut command = tokio::process::Command::new("/bin/bash");
    command.args(["-c", &format!("exec 3>/proc/{pid}/fd/1; printf held; read action; printf grace >&3; read finish; exit 0")]);
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().unwrap();
    let mut ready = [0; 4];
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut ready)
        .await
        .unwrap();
    assert_eq!(&ready, b"held");
    child
}
