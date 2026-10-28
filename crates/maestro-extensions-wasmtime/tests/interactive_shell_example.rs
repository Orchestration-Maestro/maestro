//! Interactive user commands through the shipped component and real extension runner.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, create_extension_runtime, load_extension_from_factory,
};
use std::{rc::Rc, sync::RwLock};

/// Loads the shipped component without a UI or provider.
async fn loaded(
    io: Option<Rc<dyn maestro_extensions::native_io::NativeIo>>,
) -> ExtensionRunner<Local> {
    let path = std::env::var("MAESTRO_INTERACTIVE_SHELL_COMPONENT").unwrap();
    let bus = super::components::TracedBus::new();
    let runtime = create_extension_runtime::<Local>();
    let factory = io.map_or_else(
        || crate::artifact(path.clone()),
        |io| {
            maestro_extensions_wasmtime::artifact_factory_with_io(
                path.clone(),
                super::support::widgets(),
                maestro_extensions_wasmtime::Installation {
                    version: "controlled-shell-host".into(),
                    agent_dir: "/controlled-agent".into(),
                },
                io,
            )
        },
    );
    let owner = load_extension_from_factory(
        factory,
        "/session".into(),
        bus.controller.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session("/session").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    )
}

/// Dispatches a literal user shell event without executing a model request.
async fn command(
    runner: &ExtensionRunner<Local>,
    command: &str,
) -> Option<maestro_extensions::BashResult> {
    let mut event = maestro_extensions::UserBashEvent {
        command: command.into(),
        cwd: "/event-directory".into(),
        exclude_from_context: false,
    };
    runner.emit_user_bash::<Rc<dyn maestro_extensions::BashOperations<Local>>, maestro_extensions::BashResult>(&mut event)
        .await.unwrap().map(|result| {
            assert!(result.operations.is_none());
            match result.result { maestro_extensions::Presence::Present(value) => value, _ => panic!("missing result") }
        })
}

/// Detects defaults and force prefixes, without intercepting ordinary commands or name prefixes.
#[test]
fn interactive_shell_headless_detection() {
    if isolated("interactive_shell_headless_detection", &[]) {
        return;
    }
    super::wasi_support::run(async {
        let runner = loaded(None).await;
        for text in [
            "vim file",
            " NVIM\tfile ",
            "git add -p",
            "cat file | less",
            "i echo forced",
            "i\techo forced",
        ] {
            let result = command(&runner, text).await.expect(text);
            assert_eq!(result.output, "(interactive commands require TUI)");
            assert_eq!(result.exit_code, Some(1.0));
            assert!(!result.cancelled && !result.truncated);
        }
        for text in [
            "echo ordinary",
            "vimdiff file",
            "git status",
            "printf less",
            "cat | lessish",
            "i",
        ] {
            assert!(command(&runner, text).await.is_none(), "{text}");
        }
    });
}

/// Isolates environment configuration without changing another test's process state.
fn isolated(name: &str, environment: &[(&str, &str)]) -> bool {
    if std::env::var_os("MAESTRO_INTERACTIVE_CHILD").is_some() {
        return false;
    }
    let component = std::env::var("MAESTRO_INTERACTIVE_SHELL_COMPONENT").unwrap();
    let mut child = maestro_test_process::TestProcess::current(name)
        .unwrap()
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("MAESTRO_INTERACTIVE_CHILD", "1")
        .env("MAESTRO_INTERACTIVE_SHELL_COMPONENT", component);
    for (key, value) in environment {
        child = child.env(key, value);
    }
    let output = child.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    true
}

/// Includes configured prefixes, excludes them case-insensitively and keeps force available.
#[test]
fn interactive_shell_configured_prefixes() {
    if isolated(
        "interactive_shell_configured_prefixes",
        &[
            (
                "INTERACTIVE_COMMANDS",
                " Custom Tool , custom-editor, ,vim ",
            ),
            ("INTERACTIVE_EXCLUDE", " VIM , CUSTOM-EDITOR "),
        ],
    ) {
        return;
    }
    super::wasi_support::run(async {
        let runner = loaded(None).await;
        for text in [
            "Custom Tool file",
            "cat | custom tool file",
            "i vim file",
            "i\tcustom-editor file",
            "nvim file",
        ] {
            assert!(command(&runner, text).await.is_some(), "{text}");
        }
        for text in [
            "vim file",
            "custom-editor file",
            "custom toolbox",
            "echo ordinary",
        ] {
            assert!(command(&runner, text).await.is_none(), "{text}");
        }
    });
}

/// Child effects that observe the real frame writer rather than stopping a substitute.
struct Io {
    /// The actual terminal's lifecycle and output.
    terminal: super::recording_terminal::RecordingTerminal,
    /// Selected numeric status, or signal termination.
    code: Option<i32>,
    /// A launch or wait failure, otherwise empty.
    fault: &'static str,
    /// Literal command after force-prefix removal.
    command: &'static str,
    /// Number of admitted launches.
    launches: std::cell::Cell<usize>,
}
impl maestro_extensions::native_io::NativeIo for Io {
    fn spawn(
        &self,
        shell: String,
        args: Vec<String>,
        options: maestro_extensions::native_io::SpawnOptions,
    ) -> maestro_extensions::native_io::IoFuture<
        '_,
        Rc<dyn maestro_extensions::native_io::ChildProcess>,
    > {
        self.launches.set(self.launches.get() + 1);
        assert_eq!(self.terminal.events().last(), Some(&"stop"));
        assert!(
            self.terminal
                .writes()
                .iter()
                .any(|text| text == "\x1b[2J\x1b[H")
        );
        assert_eq!(
            shell,
            std::env::var("SHELL")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "/bin/sh".into())
        );
        assert_eq!(args, ["-c", self.command]);
        assert_eq!(options.cwd.as_deref(), Some("/session"));
        assert!(options.env.is_none() && !options.detached);
        assert_eq!(
            options.stdio,
            [maestro_extensions::native_io::Stdio::Inherit; 3]
        );
        let child: Rc<dyn maestro_extensions::native_io::ChildProcess> = Rc::new(Child {
            code: self.code,
            fault: self.fault,
        });
        Box::pin(async move {
            if self.fault == "spawn" {
                Err("controlled spawn failure".into())
            } else {
                Ok(child)
            }
        })
    }
    fn watch(
        &self,
        _: String,
        _: Rc<dyn Fn()>,
    ) -> Result<Rc<dyn maestro_extensions::native_io::FsWatcher>, String> {
        panic!("no watcher")
    }
    fn fetch(
        &self,
        _: maestro_extensions::native_io::Request,
    ) -> maestro_extensions::native_io::IoFuture<'_, Rc<dyn maestro_extensions::native_io::Response>>
    {
        panic!("no network")
    }
    fn kill(&self, _: i32, _: Option<String>) -> Result<(), String> {
        panic!("no signal")
    }
}

/// A retained child with one controlled completion.
struct Child {
    /// The host's selected status.
    code: Option<i32>,
    /// Whether waiting fails.
    fault: &'static str,
}
impl maestro_extensions::native_io::ChildProcess for Child {
    fn pid(&self) -> Option<u32> {
        Some(123)
    }
    fn stdin(&self) -> Option<Rc<dyn maestro_extensions::native_io::Writable>> {
        None
    }
    fn stdout(&self) -> Option<Rc<dyn maestro_extensions::native_io::Readable>> {
        None
    }
    fn stderr(&self) -> Option<Rc<dyn maestro_extensions::native_io::Readable>> {
        None
    }
    fn kill(&self, _: Option<String>) -> Result<bool, String> {
        panic!("no signal")
    }
    fn wait(
        &self,
    ) -> maestro_extensions::native_io::IoFuture<'_, maestro_extensions::native_io::ExitStatus>
    {
        Box::pin(async move {
            if self.fault == "wait" {
                return Err("controlled wait failure".into());
            }
            Ok(maestro_extensions::native_io::ExitStatus {
                code: self.code,
                signal: self.code.is_none().then(|| "SIGTERM".into()),
            })
        })
    }
}

/// Constructs the guest's custom factory on the real terminal, then waits for its close.
struct Ui(maestro_tui::TUI);
impl maestro_extensions::types::ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn custom_component(
        &self,
        factory: maestro_extensions::types::CustomComponentFactory,
        options: Option<maestro_extensions::types::CustomUIOptions>,
    ) -> maestro_extensions::types::ExtensionUIFuture<()> {
        assert!(options.is_none());
        let (send, receive) = tokio::sync::oneshot::channel();
        let send = std::cell::RefCell::new(Some(send));
        let close = Rc::new(move || {
            Ok(send
                .borrow_mut()
                .take()
                .is_some_and(|send| send.send(()).is_ok()))
        });
        let keys = maestro_extensions::widgets::ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
            reload: None,
        };
        let building = factory.call((self.0.clone(), self.theme(), keys, close));
        Box::pin(async move {
            let built = match building? {
                maestro_extensions::types::CustomFactoryOutcome::Ready(built) => built,
                maestro_extensions::types::CustomFactoryOutcome::Async(building) => {
                    building.await?
                }
            };
            assert!(built.component.try_render(40).unwrap().is_empty());
            receive
                .await
                .map_err(|error| maestro_extensions::EventError::from(error.to_string()))?;
            if let Some(dispose) = built.dispose {
                dispose.call(())?;
            }
            Ok(())
        })
    }
}

/// Expected status, failure point, user input, shell operand and result text.
type ShellCase = (
    Option<i32>,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

/// Successful, unsuccessful, signalled and failed process completions.
const SHELL_CASES: [ShellCase; 5] = [
    (
        Some(0),
        "",
        "i echo controlled",
        "echo controlled",
        "(interactive command completed successfully)",
    ),
    (
        Some(7),
        "",
        "i\techo controlled",
        "echo controlled",
        "(interactive command exited with code 7)",
    ),
    (
        None,
        "",
        "vim file",
        "vim file",
        "(interactive command exited with code null)",
    ),
    (
        None,
        "spawn",
        "i echo controlled",
        "echo controlled",
        "(interactive command failed: controlled spawn failure)",
    ),
    (
        None,
        "wait",
        "i echo controlled",
        "echo controlled",
        "(interactive command failed: controlled wait failure)",
    ),
];

/// Covers inherited descriptors, cwd, force removal, statuses and restoration after native failure.
#[test]
fn interactive_shell_restores_terminal_and_returns_child_status() {
    if std::env::var_os("MAESTRO_INTERACTIVE_CHILD").is_none() {
        for env in [
            &[][..],
            &[("SHELL", "")][..],
            &[("SHELL", "/controlled-shell")][..],
        ] {
            assert!(isolated(
                "interactive_shell_restores_terminal_and_returns_child_status",
                env
            ));
        }
        return;
    }
    super::wasi_support::run(async {
        for (code, fault, input, stripped, output) in SHELL_CASES {
            let terminal = super::recording_terminal::RecordingTerminal::new(40, 12);
            let clock = super::manual_runtime::ManualRuntime::new();
            let tui = maestro_tui::TUI::new(
                terminal.handle(),
                clock.handle(),
                maestro_tui::TerminalImage::default(),
                Some(false),
            );
            let io = Rc::new(Io {
                terminal: terminal.clone(),
                code,
                fault,
                command: stripped,
                launches: std::cell::Cell::new(0),
            });
            let runner = loaded(Some(io.clone())).await;
            runner.set_ui_context(Some(maestro_extensions::ExtensionUIContext::new(Rc::new(
                Ui(tui.clone()),
            ))));
            tui.start().unwrap();
            clock.settle().unwrap();
            let redraws = tui.full_redraws();
            let result = command(&runner, input).await.expect(input);
            assert_eq!(result.output, output);
            assert_eq!(result.exit_code, Some(f64::from(code.unwrap_or(1))));
            assert!(!result.cancelled && !result.truncated);
            assert_eq!(io.launches.get(), 1);
            assert_eq!(terminal.events(), ["start", "stop", "start"]);
            clock.settle().unwrap();
            assert_eq!(tui.full_redraws(), redraws + 1);
            tui.stop().unwrap();
        }
    });
}
