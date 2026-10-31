//! Real signals, tracked detached children and the actual native output producers.
use super::super::super::lifecycle::{LifecycleEffects, LifecycleState};
use super::{Setup, ports};
use maestro_test_process::{CheckedChild, TestProcess, selection};
use maestro_tui::{Terminal, tui::TuiRuntime};
use std::{
    cell::{Cell, RefCell},
    io::{self, Read, Write},
    path::Path,
    process::Stdio,
    rc::Rc,
};

/// Native process effects with a closed, observable operation record at exit.
struct Native {
    /// The unchanged native registration, child cleanup and exit effects.
    inner: Rc<dyn LifecycleEffects>,
    /// Ordered operations, shared with terminal and application disposal.
    events: Rc<RefCell<Vec<String>>>,
    /// The file the parent reads after process termination.
    log: std::path::PathBuf,
}
impl LifecycleEffects for Native {
    fn now_ms(&self) -> i64 {
        self.inner.now_ms()
    }
    fn register_signals(&self, term: Rc<dyn Fn()>, hup: Rc<dyn Fn()>) -> io::Result<Box<dyn Fn()>> {
        let remove = self.inner.register_signals(term, hup)?;
        self.events.borrow_mut().push("register".into());
        let events = Rc::clone(&self.events);
        let removed = Cell::new(false);
        Ok(Box::new(move || {
            if !removed.replace(true) {
                events.borrow_mut().push("remove".into());
                remove();
            }
        }))
    }
    fn kill_tracked_detached_children(&self) {
        self.events.borrow_mut().push("kill".into());
        self.inner.kill_tracked_detached_children();
    }
    fn exit(&self, status: i32) {
        self.events.borrow_mut().push(format!("exit:{status}"));
        std::fs::write(&self.log, self.events.borrow().join("\n")).unwrap();
        self.inner.exit(status);
    }
}

/// Adds an actual awaited application shutdown callback, optionally left pending.
fn disposal(
    events: &Rc<RefCell<Vec<String>>>,
    pending: bool,
) -> maestro_app::ExtensionFactory<maestro_app::Local> {
    let events = Rc::clone(events);
    Rc::new(move |api| {
        let events = Rc::clone(&events);
        api.on::<maestro_app::SessionShutdownEvent, ()>(
            "session_shutdown".into(),
            Rc::new(move |_, _| {
                events.borrow_mut().push("dispose".into());
                eprintln!("DISPOSING");
                Box::pin(wait_disposal(pending))
            }),
        )
        .unwrap();
        Box::pin(async { Ok(()) })
    })
}

/// Runs the actual constructor/init and waits for the parent's first native signal.
fn child(directory: &Path, case: &str) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let events = Rc::new(RefCell::new(Vec::new()));
        let host: Rc<dyn TuiRuntime> = Rc::new(maestro_tui_crossterm::ProcessTuiRuntime::new(
            Rc::clone(&local),
        ));
        let recorded = super::super::RecordingTerminal::new(80, 24);
        super::trace_terminal(&recorded, &events);
        let terminal: Box<dyn Terminal> = if case == "stdout" {
            Box::new(maestro_tui_crossterm::ProcessTerminal::new(Rc::clone(
                &local,
            )))
        } else {
            Box::new(recorded)
        };
        let world = Setup {
            lifecycle: Rc::new(Native {
                inner: LifecycleState::default().effects,
                events: Rc::clone(&events),
                log: directory.join("operations"),
            }),
            terminal: Some((
                Rc::new(RefCell::new(ports::TerminalPort {
                    before_drain: Rc::default(),
                    inner: terminal,
                    output: ports::Output::default(),
                    drain_error: Rc::new(Cell::new(None)),
                    events: Rc::clone(&events),
                })),
                Rc::clone(&host),
            )),
            extension: Some(disposal(&events, case == "second")),
            ..Setup::default()
        }
        .build(directory);
        world.mode.init().await.unwrap();
        tracked(&world, directory).await;
        ready(directory, case);
        if matches!(case, "stdout" | "stderr") {
            break_output(&world.mode, &host, case);
        }
        std::future::pending::<()>().await;
    });
}

/// Fails the real native writer after initialization, not its callback or error formatter.
fn break_output(mode: &crate::InteractiveMode, host: &Rc<dyn TuiRuntime>, case: &str) {
    if case == "stdout" {
        let _ = mode.ui.terminal().borrow_mut().write("closed output");
    } else {
        let (reader, writer) = rustix::pipe::pipe().unwrap();
        drop(reader);
        rustix::stdio::dup2_stderr(&writer).unwrap();
        host.spawn_local(Box::pin(async {
            Err(Box::new(io::Error::other("work failed")) as Box<dyn std::error::Error>)
        }));
    }
}

/// Reads one readiness record; the producer waits for a signal or explicit input next.
fn line(child: &mut CheckedChild) -> String {
    let mut value = Vec::new();
    let stderr = child.stderr.as_mut().unwrap();
    let mut byte = [0];
    while stderr.read(&mut byte).unwrap() != 0 {
        if byte[0] == b'\n' {
            break;
        }
        value.push(byte[0]);
    }
    String::from_utf8(value).unwrap()
}

/// Sends a signal to the exact child identity from its readiness record.
fn signal(pid: &str, signal: &str) {
    assert!(
        std::process::Command::new("kill")
            .args(["-s", signal, pid])
            .status()
            .unwrap()
            .success()
    );
}

/// Runs one fresh checked process and proves its complete exit outcome and operation set.
pub(super) fn run(test: &str, case: &str, first: &str, second: Option<&str>) {
    if selection().is_child() {
        if selection().runs_case(case) {
            let directory = std::env::var(&*super::super::ENV_AGENT_DIR).unwrap();
            child(Path::new(&directory), case);
        }
        return;
    }
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let directory = scratch.path();
    let mut child = TestProcess::current(test)
        .unwrap()
        .env_clear()
        .case(case)
        .nocapture()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env(&*super::super::ENV_AGENT_DIR, directory)
        .temp_dir(directory)
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let ready = line(&mut child);
    if ready.is_empty() {
        let output = child.wait_with_output().unwrap();
        panic!("native child ended before readiness: {output:?}");
    }
    let pid = ready
        .strip_prefix("READY ")
        .unwrap_or_else(|| panic!("initialized with a live tracked child: {ready}"));
    if case == "stdout" {
        drop(child.stdout.take());
    }
    if matches!(case, "stdout" | "stderr") {
        std::os::unix::net::UnixStream::connect(directory.join("control"))
            .unwrap()
            .write_all(b"FAIL")
            .unwrap();
    } else {
        signal(pid, first);
    }
    if let Some(second) = second {
        assert_eq!(line(&mut child), "DISPOSING");
        signal(pid, second);
    }
    let output = child.wait_with_output().unwrap();
    verify(&output, directory, case, second);
}

/// Keeps the awaited extension boundary pending when selected.
async fn wait_disposal(
    pending: bool,
) -> Result<Option<()>, Box<dyn std::error::Error + Send + Sync>> {
    if pending {
        std::future::pending::<()>().await;
    }
    Ok(None)
}

/// Starts a tracked detached child and waits for its actual output before readiness.
async fn tracked(world: &super::super::World, directory: &Path) {
    let (sent, mut chunks) = tokio::sync::mpsc::unbounded_channel();
    let bash = world.host.session().execute_bash(
        "echo $$; exec python3 -c 'import signal; signal.pause()'",
        Some(Box::new(move |chunk| {
            sent.send(chunk.to_owned()).unwrap();
        })),
        maestro_app::agent_session::BashRunOptions::default(),
    );
    drop(bash);
    let pid = chunks.recv().await.unwrap();
    std::fs::write(directory.join("tracked"), pid.trim()).unwrap();
}

/// Verifies the native exit and the final tracked-child observation.
fn verify(output: &std::process::Output, directory: &Path, case: &str, second: Option<&str>) {
    use std::os::unix::process::ExitStatusExt;
    if let Some(second) = second {
        assert_eq!(
            (output.status.code(), output.status.signal()),
            (None, Some(if second == "TERM" { 15 } else { 1 }))
        );
    } else {
        let graceful = matches!(case, "term" | "hup");
        assert_eq!(
            output.status.code(),
            Some(if graceful { 0 } else { 129 }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let log = std::fs::read_to_string(directory.join("operations")).unwrap();
        assert_eq!(
            log,
            if graceful {
                "register\nkill\nremove\ndispose\ndrain\nstop\nexit:0"
            } else {
                "register\nremove\nkill\nexit:129"
            }
        );
    }
    let tracked = std::fs::read_to_string(directory.join("tracked")).unwrap();
    let status = std::process::Command::new("ps")
        .args(["-p", tracked.trim(), "-o", "stat="])
        .output()
        .unwrap();
    assert!(
        status.stdout.is_empty()
            || String::from_utf8_lossy(&status.stdout)
                .trim_start()
                .starts_with('Z'),
        "tracked child terminated"
    );
}

/// Announces readiness before waiting on a separate control socket, never terminal input.
fn ready(directory: &Path, case: &str) {
    if matches!(case, "stdout" | "stderr") {
        let listener = std::os::unix::net::UnixListener::bind(directory.join("control")).unwrap();
        eprintln!("READY {}", std::process::id());
        let (mut stream, _) = listener.accept().unwrap();
        let mut release = [0; 4];
        stream.read_exact(&mut release).unwrap();
        assert_eq!(&release, b"FAIL");
    } else {
        eprintln!("READY {}", std::process::id());
    }
}
