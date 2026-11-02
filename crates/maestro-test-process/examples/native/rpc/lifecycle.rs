//! Native launch observations, diagnostics, signals and descendant-held pipes.

use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};

use maestro_test_process::child::{
    Stream, announce, close_streams, connect, leave, receive, write_stderr,
};
use maestro_test_process::rpc_peer::Scenario;
use serde_json::json;

use super::{Commands, drain, emit, respond, variable};

/// More diagnostic bytes than an unread native pipe holds.
const FILL: usize = 1 << 20;

/// Report a launch without altering the native operands or environment.
pub(super) fn marked(scenario: Scenario, marker: &str) -> io::Result<()> {
    writeln!(
        std::fs::File::options()
            .create(true)
            .append(true)
            .open(marker)?,
        "{}",
        if scenario == Scenario::Launch {
            "launched"
        } else {
            "started"
        }
    )?;
    if scenario == Scenario::Launch {
        emit(&json!({"type": "peer_ready"}))?;
    } else {
        emit(
            &json!({"type": "peer_ready", "argv": std::env::args().skip(1).collect::<Vec<_>>(),
            "cwd": std::env::current_dir()?, "path": std::env::var("PATH").ok(), "home": std::env::var("HOME").ok()}),
        )?;
    }
    drain()
}

/// Queue TERM notifications before publishing readiness so an early signal cannot be missed.
pub(super) fn termination_pipe() -> io::Result<UnixStream> {
    let (writer, reader) = UnixStream::pair()?;
    signal_hook::low_level::pipe::register(signal_hook::consts::SIGTERM, writer)?;
    Ok(reader)
}

/// Retain the signal pipe before readiness, then report each TERM from a blocking reader.
fn termination(chunks: &'static [&'static [u8]]) -> io::Result<()> {
    let mut signals = termination_pipe()?;
    std::thread::Builder::new().spawn(move || -> io::Result<()> {
        while signals.read(&mut [0])? != 0 {
            for chunk in chunks {
                write_stderr(chunk)?;
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Write the diagnostic corpus, acknowledging commands without ending on TERM.
fn forwarding() -> io::Result<()> {
    termination(&[b"terminating\xc3", b"\xa9\n"])?;
    write_stderr(b"boot\xff\n")?;
    write_stderr(&vec![b'x'; FILL])?;
    emit(&json!({"type": "peer_ready"}))?;
    let mut commands = Commands::new();
    while let Some((_, command)) = commands.next()? {
        respond(&command, None)?;
    }
    Ok(())
}

/// Capture restart commands while leaving state requests unanswered.
fn restarting() -> io::Result<()> {
    write_stderr(b"child\n")?;
    emit(&json!({"type": "peer_ready"}))?;
    let mut commands = Commands::new();
    while let Some((raw, command)) = commands.next()? {
        emit(&json!({"type": "wire_capture", "raw": raw}))?;
        if command["type"] != "get_state" {
            respond(&command, None)?;
        }
    }
    Ok(())
}

/// Keep the first byte of a diagnostic character pending until a command arrives.
fn split_stderr() -> io::Result<()> {
    write_stderr(b"a\xc3")?;
    emit(&json!({"type": "peer_ready"}))?;
    let mut commands = Commands::new();
    while let Some((_, command)) = commands.next()? {
        write_stderr(b"\xa9\n")?;
        respond(&command, None)?;
    }
    Ok(())
}

/// Start a real descendant, awaiting its readiness before exiting independently of its pipes.
fn descendant(port: u16) -> io::Result<()> {
    let holder = variable("PEER_HOLDER")?;
    let ready = TcpListener::bind(("127.0.0.1", 0))?;
    emit(&json!({"type": "peer_ready", "pid": std::process::id()}))?;
    Commands::new().required()?;
    let child = Command::new(holder)
        .env("PEER_PORT", port.to_string())
        .env("PEER_READY_PORT", ready.local_addr()?.port().to_string())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut socket = ready.accept()?.0;
    if receive(&mut socket)?.is_none() {
        return Err(io::Error::other("descendant ended before readiness"));
    }
    // The child deliberately outlives this process and retains the inherited output descriptors.
    emit(&json!({"type": "descendant_ready", "pid": child.id()}))?;
    drop(child);
    leave(0)
}

/// Hold real inherited pipes until the test releases the descendant through its socket.
fn pipe_holder(port: u16) -> io::Result<()> {
    let ready = variable("PEER_READY_PORT")?
        .parse()
        .map_err(io::Error::other)?;
    let mut control = connect(port)?;
    announce(&mut control, &std::process::id().to_string())?;
    connect(ready)?.write_all(b"r")?;
    receive(&mut control)?;
    Ok(())
}

/// Emit acknowledged ticks, including one final write after TERM before a normal exit.
fn ticking(port: u16) -> io::Result<()> {
    let mut signals = termination_pipe()?;
    std::thread::Builder::new().spawn(move || {
        if matches!(signals.read(&mut [0]), Ok(1)) {
            // Detachment can close stdout; the native child must still exit normally.
            let _ = emit(&json!({"type": "tick"}));
            leave(0);
        }
    })?;
    let mut control = connect(port)?;
    emit(&json!({"type": "peer_ready"}))?;
    while receive(&mut control)?.is_some() {
        emit(&json!({"type": "tick"}))?;
    }
    Ok(())
}

/// Run socket-released native stream scenarios.
pub(super) fn controlled(scenario: Scenario, port: u16) -> io::Result<()> {
    match scenario {
        Scenario::Fragments => super::transport::fragments(port),
        Scenario::Descendant => descendant(port),
        Scenario::PipeHolder => pipe_holder(port),
        Scenario::Tick => ticking(port),
        _ => Err(io::Error::other("not a socket-controlled scenario")),
    }
}

/// Run native lifecycle variants without a shell or interpreted payload.
pub(super) fn run(scenario: Scenario) -> io::Result<()> {
    match scenario {
        Scenario::Entry => {
            emit(&json!({"type": "peer_ready", "entry": std::env::current_exe()?}))?;
            drain()
        }
        Scenario::BootFailure => {
            write_stderr(b"boot failed\n")?;
            leave(7)
        }
        Scenario::Kill => {
            rustix::process::kill_process(
                rustix::process::getpid(),
                rustix::process::Signal::KILL,
            )?;
            Err(io::Error::other("kill did not terminate the peer"))
        }
        Scenario::Booting => {
            write_stderr(b"booting\n")?;
            emit(&json!({"type": "peer_ready"}))?;
            drain()
        }
        Scenario::ForwardStderr => forwarding(),
        Scenario::DeliverStderr => {
            write_stderr(&vec![b'x'; FILL])?;
            write_stderr(b"end\n")?;
            emit(&json!({"type": "peer_ready", "pid": std::process::id()}))?;
            drain()
        }
        Scenario::SplitStderr => split_stderr(),
        Scenario::Resist => {
            termination(&[b"terminating\n"])?;
            emit(&json!({"type": "peer_ready", "pid": std::process::id()}))?;
            drain()
        }
        Scenario::Restart => restarting(),
        Scenario::ClosedInput => {
            close_streams(&[Stream::Input])?;
            emit(&json!({"type": "peer_ready"}))?;
            let (_release, held) = std::sync::mpsc::channel::<()>();
            held.recv().map_err(io::Error::other)
        }
        _ => Err(io::Error::other("not a lifecycle scenario")),
    }
}
