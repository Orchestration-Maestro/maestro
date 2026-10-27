//! Controlled presentations over the existing startup service fixture.
use crate::startup_runtime::Fixture;
use crate::support::{presenters, startup};
use maestro_app::configuration::timings::Timings;
use maestro_app::presentation_data::mode_dispatch::{
    InteractiveModeOptions, InteractiveModeRunner, ModeLaunchers,
};
use maestro_app::{AgentSessionRuntime, ModelResolverOutput, ScopedModel};
use maestro_cli::startup::main::bootstrap::AppMode;
use maestro_cli::startup::main::dispatch::{DispatchOutput, ModeDispatch, dispatch_mode};
use maestro_cli::startup::main::runtime::create_runtime;
use maestro_theme::ThemeState;
use std::{
    cell::{Cell, RefCell},
    error::Error,
    io::{self, Write},
    rc::Rc,
    sync::Arc,
};
use tokio::task::LocalSet;

/// A distinguishable original failure.
#[derive(Debug)]
pub struct Failure {
    /// The phase that produced the failure.
    pub phase: &'static str,
    /// Identity retained through propagation.
    pub identity: Arc<()>,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.phase)
    }
}
impl Error for Failure {}

/// Presentation calls, streams and their controlled failure.
#[derive(Default)]
pub struct Recorder {
    /// Calls and exact bytes, in emission order.
    pub log: Rc<RefCell<Vec<String>>>,
    /// The selected failing phase.
    pub fail: Cell<Option<&'static str>>,
    /// The original error's identity.
    pub identity: Arc<()>,
    /// Inputs moved into the interactive constructor.
    pub options: RefCell<Option<InteractiveModeOptions>>,
    /// Runtime received by the selected presentation.
    pub runtime: RefCell<Option<Rc<AgentSessionRuntime>>>,
}
impl Recorder {
    /// Record a phase before optionally failing.
    pub fn call(&self, phase: &'static str) -> Result<(), Failure> {
        self.log.borrow_mut().push(phase.into());
        if self.fail.get() == Some(phase) {
            Err(Failure {
                phase,
                identity: Arc::clone(&self.identity),
            })
        } else {
            Ok(())
        }
    }
    /// Coalesce writer chunks, without losing bytes, ordering or occurrences.
    pub fn events(&self) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        for entry in self.log.borrow().iter() {
            if let Some((stream, bytes)) = entry.split_once(':')
                && matches!(stream, "stdout" | "stderr")
                && let Some(previous) = result.last_mut()
                && previous.starts_with(&format!("{stream}:"))
            {
                previous.push_str(bytes);
                continue;
            }
            result.push(entry.clone());
        }
        result
    }
    /// Exact bytes written to one selected stream.
    pub fn bytes(&self, stream: &str) -> String {
        self.log
            .borrow()
            .iter()
            .filter_map(|entry| entry.strip_prefix(&format!("{stream}:")))
            .collect()
    }
}
impl InteractiveModeRunner for Recorder {
    fn init(self: Rc<Self>) -> maestro_tui::tui::LocalFuture {
        Box::pin(async move { Ok(self.call("init")?) })
    }
    fn run(self: Rc<Self>) -> maestro_tui::tui::LocalFuture {
        let result = self.call("run");
        Box::pin(async move { Ok(result?) })
    }
    fn stop(&self) -> io::Result<()> {
        self.call("stop").map_err(io::Error::other)
    }
}

/// A writer retaining exact bytes and flush phases.
pub struct Writer {
    /// Shared presentation observations.
    pub recorder: Rc<Recorder>,
    /// The selected stream.
    pub stream: &'static str,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let phase = if self.stream == "stdout" {
            "stdout.write"
        } else {
            "stderr.write"
        };
        if self.recorder.fail.get() == Some(phase) {
            self.recorder.call(phase).map_err(io::Error::other)?;
        }
        self.recorder.log.borrow_mut().push(format!(
            "{}:{}",
            self.stream,
            String::from_utf8_lossy(bytes)
        ));
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.recorder
            .call(if self.stream == "stdout" {
                "stdout.flush"
            } else {
                "stderr.flush"
            })
            .map_err(io::Error::other)
    }
}

/// A retained real application runtime and supplied presentation operations.
pub struct Harness {
    /// Directory-bound services retained until the runtime is finished.
    pub fixture: Fixture,
    /// The actual session runtime.
    pub runtime: Rc<AgentSessionRuntime>,
    /// Constructor and run observations.
    pub recorder: Rc<Recorder>,
    /// Caller-owned frontend operations.
    pub launchers: ModeLaunchers,
    /// The selected theme owner.
    pub themes: Rc<ThemeState>,
    /// Startup samples retained across dispatch.
    pub timings: Timings,
    /// Terminal style selection.
    pub styles: bool,
}
impl Harness {
    /// Build actual services with independently selected quiet-startup settings.
    pub async fn new(local: Rc<LocalSet>, quiet: bool) -> Self {
        let fixture =
            Fixture::new(local, startup::settings(&[("quietStartup", quiet.into())])).await;
        let created = create_runtime(
            &startup::args(&["--model", "startup/first"]),
            fixture.creation(),
            &mut ModelResolverOutput {
                stdout: &mut Vec::new(),
                stderr: &mut Vec::new(),
                color: false,
            },
        )
        .await
        .unwrap();
        let runtime = Rc::new(AgentSessionRuntime::new(
            created,
            Rc::new(|_| panic!("dispatch does not replace a runtime")),
        ));
        let recorder = Rc::new(Recorder::default());
        let launchers = launchers(&recorder);
        let themes = presenters::theme(std::path::Path::new(&runtime.cwd()));
        Self {
            fixture,
            runtime,
            recorder,
            launchers,
            themes,
            timings: Timings::new(None, || 0.0),
            styles: false,
        }
    }
    /// Drive one dispatch through its public caller boundary.
    pub async fn dispatch(&mut self, request: ModeDispatch) -> Result<i32, Box<dyn Error>> {
        let mut stdout = Writer {
            recorder: Rc::clone(&self.recorder),
            stream: "stdout",
        };
        let mut stderr = Writer {
            recorder: Rc::clone(&self.recorder),
            stream: "stderr",
        };
        dispatch_mode(
            Rc::clone(&self.runtime),
            request,
            &self.launchers,
            &self.themes,
            &mut DispatchOutput {
                stdout: &mut stdout,
                stderr: &mut stderr,
                timings: &mut self.timings,
                now: &|| 12.0,
                styles_enabled: self.styles,
            },
        )
        .await
    }
    /// Install a real custom-theme watch over the existing recording effects.
    pub fn watch(&self) {
        let root = self.fixture.creation().services.cwd.clone();
        std::fs::create_dir_all(std::path::Path::new(&root).join("custom")).unwrap();
        std::fs::write(
            std::path::Path::new(&root).join("custom/custom.json"),
            include_str!("../fixtures/custom_theme.json"),
        )
        .unwrap();
        self.themes
            .init_theme(
                Some("custom"),
                Some(Rc::new(presenters::RecordingWatch {
                    log: Rc::clone(&self.recorder.log),
                })),
            )
            .unwrap();
    }
}
/// Presentations that record their construction, runtime and lifecycle into `recorder`.
pub fn launchers(recorder: &Rc<Recorder>) -> ModeLaunchers {
    let (interactive, rpc) = (Rc::clone(recorder), Rc::clone(recorder));
    ModeLaunchers {
        interactive: Rc::new(move |runtime, options| {
            interactive.call("factory")?;
            *interactive.runtime.borrow_mut() = Some(runtime);
            *interactive.options.borrow_mut() = Some(options);
            Ok(interactive.clone())
        }),
        rpc: Rc::new(move |runtime| {
            *rpc.runtime.borrow_mut() = Some(runtime);
            let result = rpc.call("rpc");
            Box::pin(async move { Ok(result?) })
        }),
    }
}

/// A prepared request with no prompt or captured scope.
pub fn request(app_mode: AppMode) -> ModeDispatch {
    ModeDispatch {
        app_mode,
        startup_benchmark: None,
        interactive_options: InteractiveModeOptions::default(),
        scoped_models: Vec::new(),
    }
}
/// One captured descriptor, retaining the catalog's existing typed model owner.
pub fn scoped(h: &Harness, id: &str, level: Option<&str>) -> ScopedModel {
    let model = h.runtime.session().model().read().unwrap().clone();
    let mut model = model;
    model.id = id.into();
    ScopedModel {
        model: Arc::new(std::sync::RwLock::new(model)),
        thinking_level: level.map(|level| level.parse().unwrap()),
    }
}
/// Prior timing samples, followed by the benchmark's existing initialization label.
pub const BENCHMARK_TIMINGS: &str = "\n--- Startup Timings ---\n  prior: 2ms\n  interactiveMode.init: 10ms\n  TOTAL: 12ms\n------------------------\n\n";
/// The same retained prior sample before an ordinary mode starts.
pub const PRIOR_TIMINGS: &str =
    "\n--- Startup Timings ---\n  prior: 2ms\n  TOTAL: 2ms\n------------------------\n\n";
/// Enable timing with a distinguishable existing startup sample.
pub fn timing(h: &mut Harness) {
    h.timings = Timings::new(Some("1"), || 0.0);
    h.timings.time("prior", || 2.0);
}

/// Answer real provider requests, recording the last user record of each request.
pub fn script_print(h: &Harness) -> Rc<RefCell<Vec<serde_json::Value>>> {
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let (received, recorder) = (Rc::clone(&prompts), Rc::clone(&h.recorder));
    h.runtime.session().model().write().unwrap().input = vec![
        maestro_app::ModelInput::Text,
        maestro_app::ModelInput::Image,
    ];
    *h.runtime.session().agent().stream_fn.write().unwrap() =
        Some(Rc::new(move |_, context, _, _| {
            recorder.call("provider").unwrap();
            received
                .borrow_mut()
                .push(serde_json::to_value(context.messages.last().unwrap()).unwrap());
            let message: maestro_app::AssistantMessage =
                serde_json::from_value(serde_json::json!({
                    "role": "assistant", "content": [{"type":"text", "text":"answer"}],
                    "api":"openai-completions", "provider":"startup", "model":"first",
                    "usage": maestro_app::Usage::default(), "stopReason":"stop", "timestamp": 0
                }))
                .unwrap();
            let event: maestro_app::AssistantMessageEvent = serde_json::from_value(
                serde_json::json!({"type":"done", "reason":"stop", "message":message}),
            )
            .unwrap();
            let stream = maestro_app::AssistantMessageEventStream::new();
            stream.push(event);
            Box::pin(std::future::ready(Ok(stream)))
        }));
    prompts
}
