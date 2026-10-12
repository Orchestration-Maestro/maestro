//! Controlled read operations, temporary files and result projections for the read tests.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#![allow(dead_code)] // each test binary uses a different part of these helpers

use maestro_agent::AgentToolResult;
use maestro_models::{
    BoxFuture, Cancellation, DiagnosticErrorInfo, Model, ModelCost, ModelInput, UserBlock,
};
use maestro_tools::{
    ReadContext, ReadOperations, ReadToolDetails, ReadToolInput, ReadToolOptions, ToolDefinition,
    create_read_tool_definition,
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Complete the future on a test-owned runtime.
pub fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

/// A directory removed when dropped.
pub struct TempDir(pub PathBuf);

impl TempDir {
    /// Create an empty directory unique to this process and call.
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-read-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    /// The directory as text.
    pub fn text(&self) -> String {
        self.0.to_str().unwrap().to_owned()
    }

    /// Write `bytes` at `name` below the directory and return its text path.
    pub fn file(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().to_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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
    /// Abort the read's signal, then never complete.
    AbortThenPend,
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
    fn step<T: Send + 'static>(
        &self,
        stage: Stage,
        path: &str,
        value: T,
    ) -> BoxFuture<Result<T, DiagnosticErrorInfo>> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{stage:?}:{path}").to_lowercase());
        let behavior = if stage == self.at {
            self.behavior
        } else {
            Behavior::Succeed
        };
        if matches!(
            behavior,
            Behavior::AbortThenSucceed | Behavior::AbortThenPend
        ) {
            self.signal.as_ref().unwrap().abort();
        }
        match behavior {
            Behavior::Fail => Box::pin(std::future::ready(Err(Self::failure(stage)))),
            Behavior::AbortThenPend => Box::pin(std::future::pending()),
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
pub fn content_json(result: &AgentToolResult<ReadToolDetails>) -> Value {
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
    if let Ok(current) = std::env::var("MAESTRO_READ_CHILD") {
        return current == mode;
    }
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", test, "--nocapture"]);
    command.env("MAESTRO_READ_CHILD", mode);
    for (key, value) in envs {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "child {mode} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}
