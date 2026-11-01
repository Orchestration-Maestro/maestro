//! Checked Rust children behind the application's native process port.
#![cfg(unix)]

use std::{cell::RefCell, rc::Rc};

use maestro_extensions_wasmtime::native_io::{
    ChildProcess, ExitStatus, FsWatcher, IoFuture, NativeIo, Readable, Request, Response,
    SpawnOptions, Writable,
};
use maestro_test_process::{HARNESS_START, TestProcess, child};
use serde_json::{Value, json};

/// Data identifying the disposable launch operand that selects the checked child.
pub(super) const MARKER: &[u8] = b"maestro-controlled-subagent";
/// The untouched product arguments passed to the checked child.
const ARGUMENTS: &str = "MAESTRO_SUBAGENT_ARGUMENTS";

/// Loads the real component with checked-child selection at its native process boundary.
pub(super) async fn session(
    root: &crate::support::Root,
    local: Rc<tokio::task::LocalSet>,
    component: String,
) -> Rc<maestro_app::AgentSession> {
    let io = Rc::new(Io(maestro_app::extension_native_io::extension_native_io(
        &local,
    )));
    let mut resources = maestro_app::ResourceLoaderOptions::<maestro_app::Local>::new(
        local,
        Rc::new(|_| Box::pin(async { Ok(None) })),
    );
    resources
        .extension_factories
        .push(maestro_extensions_wasmtime::artifact_factory_with_io(
            component,
            crate::support::widgets(),
            maestro_extensions_wasmtime::Installation {
                version: env!("CARGO_PKG_VERSION").into(),
                agent_dir: root.path("agent").display().to_string(),
            },
            io,
        ));
    crate::support::session::resource_session(root, resources, Some(maestro_app::NoTools::Builtin))
        .await
}

/// Keeps native file, network and process effects while selecting the controlled child.
pub(super) struct Io(pub(super) Rc<dyn NativeIo>);

impl NativeIo for Io {
    fn watch(&self, path: String, listener: Rc<dyn Fn()>) -> Result<Rc<dyn FsWatcher>, String> {
        self.0.watch(path, listener)
    }

    fn fetch(&self, request: Request) -> IoFuture<'_, Rc<dyn Response>> {
        self.0.fetch(request)
    }

    fn kill(&self, pid: i32, signal: Option<String>) -> Result<(), String> {
        self.0.kill(pid, signal)
    }

    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        mut options: SpawnOptions,
    ) -> IoFuture<'_, Rc<dyn ChildProcess>> {
        Box::pin(async move {
            // An unreadable marker falls through so native spawning reports its own error.
            if std::fs::read(&command).ok().as_deref() != Some(MARKER) {
                return self.0.spawn(command, args, options).await;
            }
            options
                .env
                .get_or_insert_with(|| std::env::vars().collect())
                .push((ARGUMENTS.into(), serde_json::to_string(&args).unwrap()));
            let arguments = TestProcess::child(launcher)
                .unwrap()
                .quiet()
                .invoke_with(|arguments| Ok::<_, std::io::Error>(arguments.to_vec()))
                .unwrap();
            let executable = std::env::current_exe().unwrap().display().to_string();
            let process = self.0.spawn(executable, arguments, options).await?;
            let output = process.stdout().ok_or("controlled child has no stdout")?;
            let mut prefix = Vec::new();
            while prefix.len() < HARNESS_START.len() {
                let bytes = output
                    .read()
                    .await?
                    .ok_or("controlled child did not start")?;
                prefix.extend(bytes);
            }
            if !prefix.starts_with(HARNESS_START) {
                return Err("controlled child did not start the selected test".into());
            }
            Ok(Rc::new(Process {
                process,
                output: Rc::new(Output {
                    output,
                    first: RefCell::new(Some(prefix)),
                }),
            }) as Rc<dyn ChildProcess>)
        })
    }
}

/// Native process controls with the already-observed startup bytes retained for its reader.
struct Process {
    /// The native lifecycle and descriptors.
    process: Rc<dyn ChildProcess>,
    /// The output whose first read was used to prove startup.
    output: Rc<dyn Readable>,
}

impl ChildProcess for Process {
    fn pid(&self) -> Option<u32> {
        self.process.pid()
    }
    fn stdin(&self) -> Option<Rc<dyn Writable>> {
        self.process.stdin()
    }
    fn stdout(&self) -> Option<Rc<dyn Readable>> {
        Some(Rc::clone(&self.output))
    }
    fn stderr(&self) -> Option<Rc<dyn Readable>> {
        self.process.stderr()
    }
    fn wait(&self) -> IoFuture<'_, ExitStatus> {
        self.process.wait()
    }
    fn kill(&self, signal: Option<String>) -> Result<bool, String> {
        self.process.kill(signal)
    }
}

/// Replays startup without changing the native reader's later byte boundaries.
struct Output {
    /// The original native pipe.
    output: Rc<dyn Readable>,
    /// Bytes consumed to prove startup, replayed exactly once.
    first: RefCell<Option<Vec<u8>>>,
}

impl Readable for Output {
    fn read(&self) -> IoFuture<'_, Option<Vec<u8>>> {
        Box::pin(async move {
            let first = self.first.borrow_mut().take();
            if first.is_some() {
                return Ok(first);
            }
            self.output.read().await
        })
    }
    fn close(&self) -> Result<(), String> {
        self.output.close()
    }
}

/// Blocks termination before exec, so every thread of the checked child inherits the mask.
#[test]
#[ignore = "runs only as a controlled child"]
fn launcher() {
    use nix::sys::signal::{SigSet, SigmaskHow, Signal, pthread_sigmask};
    use std::os::unix::process::CommandExt as _;
    let mut blocked = SigSet::empty();
    blocked.add(Signal::SIGTERM);
    pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&blocked), None).unwrap();
    let error = TestProcess::child(program)
        .unwrap()
        .quiet()
        .command()
        .unwrap()
        .exec();
    panic!("cannot execute the controlled child: {error}");
}

/// Validates the real launch operands and emits controlled JSON-print messages.
#[test]
#[ignore = "runs only as a controlled child"]
fn program() {
    let args: Vec<String> = serde_json::from_str(&std::env::var(ARGUMENTS).unwrap()).unwrap();
    assert_eq!(&args[..4], ["--mode", "json", "-p", "--no-session"]);
    let task = args.last().unwrap();
    if task == "Task: plain" {
        for flag in ["--model", "--tools", "--append-system-prompt"] {
            assert!(!args.iter().any(|arg| arg == flag), "{args:?}");
        }
        emit(
            &json!({"role":"assistant","content":[{"type":"text","text":"plain output"}],"model":"reported-model"}),
            false,
        );
        child::leave(0);
    }
    assert_eq!(operand(&args, "--model"), "controlled-model");
    assert_eq!(operand(&args, "--tools"), "read,ls");
    let role = std::fs::read_to_string(operand(&args, "--append-system-prompt")).unwrap();
    assert_eq!(role, "Private role prompt");
    match task.as_str() {
        "Task: signal" => nix::sys::signal::raise(nix::sys::signal::Signal::SIGKILL).unwrap(),
        "Task: cancel" => cancelled(),
        "Task: fractional" => fractional(),
        "Task: fail" => {
            child::write_stderr(b"controlled failure").unwrap();
            child::leave(7);
        }
        "Task: provider-error" | "Task: provider-abort" => provider_failure(task),
        _ => normal(task, &role),
    }
    child::leave(0);
}

/// The literal operand following a required launch flag.
fn operand<'a>(args: &'a [String], flag: &str) -> &'a str {
    let index = args.iter().position(|arg| arg == flag).unwrap();
    &args[index + 1]
}

/// Emits one message without imposing a newline on the final JSON record.
fn emit(message: &Value, newline: bool) {
    let mut bytes = serde_json::to_vec(&json!({"type":"message_end","message":message})).unwrap();
    if newline {
        bytes.push(b'\n');
    }
    child::write_stdout(&bytes).unwrap();
}

/// Reports readiness only after the launcher has blocked termination on every thread.
fn cancelled() {
    use nix::sys::signal::{SigSet, Signal};
    let mut blocked = SigSet::empty();
    blocked.add(Signal::SIGTERM);
    emit(
        &json!({"role":"assistant","content":[{"type":"text","text":"ready"}]}),
        true,
    );
    assert_eq!(blocked.wait().unwrap(), Signal::SIGTERM);
}

/// Two fractional records exercise usage accumulation without integer coercion.
fn fractional() {
    for (input, output) in [(0.5, 2.5), (1.25, 0.25)] {
        emit(
            &json!({"role":"assistant","content":[],"usage":{"input":input,"output":output,"cacheRead":0.125,"cacheWrite":0.75,"cost":{"total":0.125},"totalTokens":12.5}}),
            true,
        );
    }
}

/// A successful process exit can still carry a provider failure or abort.
fn provider_failure(task: &str) {
    emit(
        &json!({"role":"assistant","content":[],"usage":{"totalTokens":19}}),
        true,
    );
    let (reason, error) = if task.ends_with("error") {
        ("error", "controlled model failure")
    } else {
        ("aborted", "controlled model abort")
    };
    emit(
        &json!({"role":"assistant","content":[],"stopReason":reason,"errorMessage":error}),
        true,
    );
}

/// Invalid lines, tool results and successive assistant messages retain their original order.
fn normal(task: &str, role: &str) {
    child::write_stdout(b"not JSON\n\n\n").unwrap();
    emit(
        &json!({"role":"toolResult","toolCallId":"controlled-read","toolName":"read","isError":false,"timestamp":0,"content":[{"type":"text","text":"read result"}]}),
        true,
    );
    let cwd = std::env::current_dir().unwrap();
    let cwd = cwd.file_name().unwrap().to_str().unwrap();
    for (index, (input, output, cost)) in [(10, 3, 0.001), (2, 5, 0.002)].into_iter().enumerate() {
        let text = if index == 0 {
            "earlier".into()
        } else {
            format!("{task};role={role};cwd={cwd}")
        };
        emit(
            &json!({"role":"assistant","content":[{"type":"text","text":text},{"type":"text","text":"not the first block"}],"usage":{"input":input,"output":output,"cacheRead":2,"cacheWrite":3,"cost":{"total":cost},"totalTokens":19},"model":"reported-model","stopReason":"stop"}),
            index == 0,
        );
    }
    child::write_stderr(b"controlled stderr").unwrap();
}
