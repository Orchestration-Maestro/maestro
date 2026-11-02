//! A child test process that serves one fresh session runtime over its standard streams,
//! and the test's end of it.
#![cfg(unix)]
use maestro_app::configuration::config::get_theme_directories;
use maestro_app::rpc_mode::{RpcFuture, WorkspaceEventSink, WorkspaceOperations};
use maestro_app::rpc_types::{workspace, workspace_schema};
use maestro_app::{
    AgentSessionRuntime, CreateAgentSessionFromServicesOptions, CreateAgentSessionRuntimeFactory,
    CreateAgentSessionRuntimeOptions, CreateAgentSessionRuntimeResult,
    CreateAgentSessionServicesOptions, ExtensionFactory, Local, ModuleLoader,
    NativeExportOperations, ResourceLoaderOptions, RpcModeOptions, SessionManager,
    ToolRenderEnvironment, create_agent_session_from_services, create_agent_session_runtime,
    create_agent_session_services,
};
use maestro_rpc::{NativeRpcProcess, run_rpc_mode};
use maestro_test_process::{CheckedChild, CheckedStdout, TestProcess};
use maestro_theme::{SyntectHighlighter, ThemeState};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{ExitStatus, Stdio};
use std::{cell::RefCell, path::Path, rc::Rc};
use tokio::task::LocalSet;

/// A workspace command's result.
pub type WorkspaceResult = Result<workspace_schema::ResponseResult, workspace_schema::ErrorBody>;

/// Answers each workspace command, writing its events through the sink.
pub type WorkspaceAnswer = Rc<dyn Fn(workspace::RpcCommand, WorkspaceEventSink) -> WorkspaceResult>;

/// Workspace operations answered by a test closure.
struct ClosureWorkspace(WorkspaceAnswer);

impl WorkspaceOperations for ClosureWorkspace {
    fn execute(
        &self,
        command: workspace::RpcCommand,
        events: WorkspaceEventSink,
        _write_complete: tokio::sync::oneshot::Receiver<()>,
    ) -> RpcFuture<WorkspaceResult> {
        Box::pin(std::future::ready((self.0)(command, events)))
    }
}

/// What a served runtime loads and how it answers workspace commands.
pub struct Served {
    /// Extensions every session loads.
    pub factories: Vec<ExtensionFactory<Local>>,
    /// The workspace command operations.
    pub workspace: WorkspaceAnswer,
}

impl Default for Served {
    fn default() -> Self {
        Self {
            factories: Vec::new(),
            workspace: Rc::new(|_, _| {
                Err(workspace_schema::ErrorBody {
                    code: "unsupported".into(),
                    message: "no workspace".into(),
                })
            }),
        }
    }
}

/// The factory creating each session over the default services of its directory, with
/// `factories` loaded afresh.
fn runtime_factory(
    local: Rc<LocalSet>,
    factories: Vec<ExtensionFactory<Local>>,
) -> CreateAgentSessionRuntimeFactory {
    Rc::new(move |options: CreateAgentSessionRuntimeOptions| {
        let no_modules: ModuleLoader<Local> = Rc::new(|_| Box::pin(std::future::ready(Ok(None))));
        let mut resources = ResourceLoaderOptions::new(Rc::clone(&local), no_modules);
        resources.extension_factories.clone_from(&factories);
        Box::pin(async move {
            let services = create_agent_session_services(CreateAgentSessionServicesOptions {
                cwd: options.cwd,
                agent_dir: Some(options.agent_dir),
                auth_storage: None,
                settings_manager: None,
                model_registry: None,
                extension_flag_values: None,
                resource_loader_options: resources,
                offline: None,
            })
            .await
            .unwrap();
            let services = Rc::new(services);
            let created =
                create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
                    services: Rc::clone(&services),
                    session_manager: options.session_manager,
                    session_start_event: options.session_start_event,
                    model: None,
                    thinking_level: None,
                    scoped_models: Vec::new(),
                    tools: None,
                    no_tools: None,
                    custom_tools: Vec::new(),
                    theme: maestro_theme::LiveTheme::default(),
                })
                .await?;
            Ok(CreateAgentSessionRuntimeResult {
                session: created.session,
                services,
                diagnostics: Vec::new(),
                extensions_result: created.extensions_result,
                model_fallback_message: created.model_fallback_message,
            })
        })
    })
}

/// A runtime over an in-memory session in the process's working directory.
pub async fn runtime(
    local: &Rc<LocalSet>,
    factories: Vec<ExtensionFactory<Local>>,
) -> AgentSessionRuntime {
    let cwd = std::env::current_dir().unwrap().display().to_string();
    let manager = SessionManager::in_memory(Some(&cwd)).unwrap();
    let options = CreateAgentSessionRuntimeOptions {
        agent_dir: format!("{cwd}/agent"),
        cwd,
        session_manager: Rc::new(RefCell::new(manager)),
        session_start_event: None,
    };
    create_agent_session_runtime(runtime_factory(Rc::clone(local), factories), options)
        .await
        .unwrap()
}

/// The server's facilities: the native signals, the live theme and HTML export over the
/// real rendering environment.
pub fn options(
    local: Rc<LocalSet>,
    runtime: Rc<maestro_app::AgentSessionRuntime>,
    workspace: WorkspaceAnswer,
) -> RpcModeOptions {
    // The export facilities borrow the theme state for the process's lifetime.
    let state: &'static ThemeState =
        Box::leak(Box::new(ThemeState::new(get_theme_directories().unwrap())));
    let theme = state.theme();
    RpcModeOptions {
        workspace: Some(Rc::new(ClosureWorkspace(workspace))),
        session: Some(maestro_app::RpcSessionOptions {
            runtime,
            process: Rc::new(NativeRpcProcess),
            theme: theme.clone(),
            export_operations: Rc::new(NativeExportOperations {
                app_name: "maestro",
                template_dir: "",
                theme: state,
            }),
            render_environment: Rc::new(ToolRenderEnvironment {
                runtime: Rc::new(maestro_tui_crossterm::ProcessTuiRuntime::new(local)),
                readme_path: String::new(),
                live_theme: theme,
                highlighter: Rc::new(SyntectHighlighter::new().unwrap()),
                terminal_image: inferred(),
            }),
        }),
    }
}

/// The default value of a type this crate's edges do not name, such as the terminal image
/// record of the rendering environment.
fn inferred<T: Default>() -> T {
    T::default()
}

/// Serves a fresh runtime over this process's standard streams after writing the process
/// identifier to standard error; the server exits the process.
pub fn serve(served: Served) -> ! {
    eprintln!("{}", std::process::id());
    let local = Rc::new(LocalSet::new());
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let failure = executor.block_on(local.run_until(async {
        let runtime = runtime(&local, served.factories).await;
        let runtime = Rc::new(runtime);
        let running = Rc::clone(&local);
        let runner: maestro_app::presentation_data::mode_dispatch::RpcModeRunner =
            Rc::new(move |runtime| {
                let options = options(Rc::clone(&running), runtime, Rc::clone(&served.workspace));
                Box::pin(async move { match run_rpc_mode(options).await? {} })
            });
        runner(runtime).await.unwrap_err()
    }));
    panic!("the server failed: {failure}");
}

/// The next line `process` writes to standard error, read byte by byte so the rest stays
/// unread.
fn error_line(process: &mut CheckedChild) -> String {
    let errors = process.stderr.as_mut().unwrap();
    let (mut line, mut byte) = (Vec::new(), [0]);
    while line.last() != Some(&b'\n') && errors.read_exact(&mut byte).is_ok() {
        line.push(byte[0]);
    }
    String::from_utf8(line).unwrap()
}

/// The test's end of a serving child.
pub struct Child {
    /// The running child.
    process: CheckedChild,
    /// The child's protocol output, after the harness prologue.
    output: BufReader<CheckedStdout>,
    /// The child's process identifier.
    pub pid: String,
}

impl Child {
    /// Starts the running test `test` as a serving child for `case` in `dir`, with `dir` as
    /// its home.
    pub fn start(test: &str, case: &str, dir: &Path) -> Self {
        let mut process = TestProcess::current(test)
            .unwrap()
            .case(case)
            .nocapture()
            .current_dir(dir)
            .env("HOME", dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(process.stdout.take().unwrap());
        let mut prologue = String::new();
        // The harness prologue is an empty line, then the run's test count.
        output.read_line(&mut prologue).unwrap();
        output.read_line(&mut prologue).unwrap();
        assert_eq!(prologue, "\nrunning 1 test\n");
        let pid = error_line(&mut process).trim().to_owned();
        Self {
            process,
            output,
            pid,
        }
    }

    /// Writes `bytes` to the child's input.
    pub fn send(&mut self, bytes: &[u8]) {
        let input = self.process.stdin.as_mut().unwrap();
        input.write_all(bytes).unwrap();
        input.flush().unwrap();
    }

    /// The next protocol line the child writes, without its LF.
    pub fn next_line(&mut self) -> String {
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        line.strip_suffix('\n').unwrap().to_owned()
    }

    /// The next protocol record the child writes.
    pub fn next(&mut self) -> Value {
        serde_json::from_str(&self.next_line()).unwrap()
    }

    /// Sends `signal` to the child.
    pub fn signal(&self, signal: &str) {
        let sent = std::process::Command::new("kill")
            .args([&format!("-{signal}"), &self.pid])
            .status()
            .unwrap();
        assert!(sent.success());
    }

    /// The next line the child writes to standard error.
    pub fn next_error_line(&mut self) -> String {
        error_line(&mut self.process)
    }

    /// Waits for the child to exit with its input still open, so only a signal ends it, then
    /// returns the exit status and the rest of standard error.
    pub fn wait_open(mut self) -> (ExitStatus, String) {
        let input = self.process.stdin.take();
        let output = self.process.wait_with_output().unwrap();
        drop(input);
        (output.status, String::from_utf8(output.stderr).unwrap())
    }

    /// Closes the input, then returns the exit status, the remaining protocol lines and the
    /// rest of standard error.
    pub fn finish(mut self) -> (ExitStatus, Vec<String>, String) {
        drop(self.process.stdin.take());
        let mut rest = String::new();
        self.output.read_to_string(&mut rest).unwrap();
        let output = self.process.wait_with_output().unwrap();
        let lines = rest.lines().map(str::to_owned).collect();
        (
            output.status,
            lines,
            String::from_utf8(output.stderr).unwrap(),
        )
    }
}
