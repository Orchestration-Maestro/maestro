//! Controlled children the process tests start by re-running this test executable.
//!
//! Each program is an ignored test that only a parent test selects. It reads its arguments
//! from variables and its trailing operands, and ends through `std::process::exit` so no
//! harness epilogue follows its output. The harness writes the start bytes to standard output
//! before the program runs; parents compare output through `maestro_test_process::child`.
#![cfg(test)]
#![cfg(unix)]
use maestro_test_process::TestProcess;
use maestro_test_process::child::{
    Stream, announce, close_streams, connect, is_null, leave, port, read_input, receive, variable,
    write_stderr, write_stdout,
};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};

/// The command that runs `program` with `variables` added to its environment.
pub fn command(program: impl Fn(), variables: &[(&str, &str)]) -> Command {
    TestProcess::child(program)
        .unwrap()
        .envs(variables.iter().copied())
        .command()
        .unwrap()
}

/// [`command`] as the asynchronous command the process owner takes.
pub fn async_command(program: impl Fn(), variables: &[(&str, &str)]) -> tokio::process::Command {
    command(program, variables).into()
}

/// A connection to the parent's listener.
fn parent() -> TcpStream {
    connect(port("CHILD_PORT").unwrap()).unwrap()
}

/// An operation that writes bytes to one output stream.
type Writer = fn(&[u8]) -> std::io::Result<()>;

/// How [`held_child`] holds its output and ends, named by `CHILD_MODE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldMode {
    /// Closes the streams named in `CHILD_CLOSED` while still running, holds, then writes the tail.
    Live,
    /// Ends at once and leaves a descendant that closes those streams and holds.
    Exit,
    /// Waits for a termination request, then writes `terminated`.
    Term,
    /// Waits for a termination request, then floods standard output.
    Flood,
    /// Leaves a holder and waits for a kill.
    Kill,
}

impl HeldMode {
    /// The text a parent passes in `CHILD_MODE`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Exit => "exit",
            Self::Term => "term",
            Self::Flood => "flood",
            Self::Kill => "kill",
        }
    }

    /// The mode `CHILD_MODE` names.
    fn from_variable() -> Self {
        let name = variable("CHILD_MODE").unwrap();
        [Self::Live, Self::Exit, Self::Term, Self::Flood, Self::Kill]
            .into_iter()
            .find(|mode| mode.name() == name)
            .unwrap_or_else(|| panic!("unknown held mode `{name}`"))
    }
}

/// The streams `CHILD_CLOSED` names, none when it is unset.
fn closed_streams() -> Vec<Stream> {
    let text = std::env::var("CHILD_CLOSED").unwrap_or_default();
    Stream::parse_list(&text).unwrap()
}

/// Raises `signal` on this process and waits for it to take effect.
fn kill_self(signal: rustix::process::Signal) -> ! {
    rustix::process::kill_process(rustix::process::getpid(), signal).unwrap();
    wait_for_signal()
}

/// Waits for a signal to end this process, which never ends on its own: a signal that
/// arrives while a process exits cuts the coverage profile it is writing.
fn wait_for_signal() -> ! {
    loop {
        std::thread::park();
    }
}

/// A descendant that outlives this process and holds its output descriptors: the program
/// starts with a pipe on standard input that closes when this process ends.
pub fn spawn_descendant(program: impl Fn(), variables: &[(&str, &str)]) -> std::process::Child {
    let mut command = command(program, variables);
    command.stdin(Stdio::piped());
    command.spawn().unwrap()
}

/// Waits for the process that started this descendant to end.
fn await_parent_exit() {
    read_input().unwrap();
}

/// Reports readiness to the parent's listener and returns the connection.
fn ready() -> TcpStream {
    let mut socket = parent();
    socket.write_all(b"READY").unwrap();
    socket
}

/// Holds until the parent releases the connection.
fn hold() {
    receive(&mut ready()).unwrap();
}

/// Writes `tail` to each standard descriptor not named in `closed` and ends.
fn finish(closed: &[Stream]) -> ! {
    let writers: [(Stream, Writer); 2] = [
        (Stream::Output, write_stdout),
        (Stream::Error, write_stderr),
    ];
    for (stream, write) in writers {
        if !closed.contains(&stream) {
            write(b"tail").unwrap();
        }
    }
    leave(0)
}

/// Reports the null-device and byte checks the synchronous input tests expect.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn echo_input() {
    let kind: &[u8] = if is_null("/dev/stdin").unwrap() {
        b"null:"
    } else {
        b"pipe:"
    };
    let input = read_input().unwrap();
    write_stdout(&[kind, &input].concat()).unwrap();
    write_stderr(b"diag\0\xff").unwrap();
    leave(variable("CHILD_EXIT").unwrap().parse().unwrap())
}

/// Writes a large body while reading a large input.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn concurrent_io() {
    write_stdout(&vec![b'y'; 262_144]).unwrap();
    let data = read_input().unwrap();
    write_stdout(&data[..2]).unwrap();
    write_stderr(data.len().to_string().as_bytes()).unwrap();
    leave(0)
}

/// Reports its pid, waits for release, then returns the input it consumed.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn socket_consumer() {
    let mut socket = parent();
    announce(&mut socket, &std::process::id().to_string()).unwrap();
    assert_eq!(receive(&mut socket).unwrap(), Some(b'!'));
    socket.write_all(&read_input().unwrap()).unwrap();
    leave(0)
}

/// Closes its input, reports its pid, waits for release and reports an effect.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn closed_input_effect() {
    close_streams(&[Stream::Input]).unwrap();
    let mut socket = parent();
    announce(&mut socket, &std::process::id().to_string()).unwrap();
    assert_eq!(receive(&mut socket).unwrap(), Some(b'!'));
    socket.write_all(b"effect").unwrap();
    leave(0)
}

/// Closes its input, reports its pid and ends.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn closed_input_report() {
    close_streams(&[Stream::Input]).unwrap();
    announce(&mut parent(), &std::process::id().to_string()).unwrap();
    leave(0)
}

/// Reports its pid, waits for release, then reports its own process group and its parent's.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn process_groups() {
    use rustix::process::{getpgid, getppid};
    let mut socket = parent();
    announce(&mut socket, &std::process::id().to_string()).unwrap();
    assert_eq!(receive(&mut socket).unwrap(), Some(b'!'));
    let own = getpgid(None).unwrap().as_raw_nonzero();
    let parent = getpgid(getppid()).unwrap().as_raw_nonzero();
    socket
        .write_all(format!("{own} {parent}").as_bytes())
        .unwrap();
    leave(0)
}

/// Checks the launch it inherited and ends through the status `CHILD_EXIT` names.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn inherited_launch_target() {
    assert_eq!(read_input().unwrap(), b"inherited-input");
    let arguments: Vec<String> = std::env::args().collect();
    let operands = &arguments[arguments.len() - 4..];
    assert_eq!(operands, ["literal $HOME;|", "", "repeat", "repeat"]);
    assert_eq!(variable("MAESTRO_OPERAND").unwrap(), "chosen");
    let directory = std::fs::canonicalize(variable("MAESTRO_DIRECTORY").unwrap()).unwrap();
    assert_eq!(std::env::current_dir().unwrap(), directory);
    write_stdout(b"inherited-out\0\xff").unwrap();
    write_stderr(b"inherited-err").unwrap();
    match variable("CHILD_EXIT").unwrap().as_str() {
        "signal" => kill_self(rustix::process::Signal::TERM),
        code => leave(code.parse().unwrap()),
    }
}

/// Leaves a descendant holding its output after the direct child ends.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn discarded_output_parent() {
    assert_eq!(read_input().unwrap(), b"copied");
    let pid = std::process::id().to_string();
    let port = variable("CHILD_PORT").unwrap();
    let variables = [("PARENT_PID", pid.as_str()), ("CHILD_PORT", port.as_str())];
    let descendant = spawn_descendant(discarded_output_descendant, &variables);
    std::mem::forget(descendant);
    leave(7)
}

/// The descendant of [`discarded_output_parent`]: it reports which device its output is.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn discarded_output_descendant() {
    await_parent_exit();
    let mut socket = parent();
    announce(&mut socket, &variable("PARENT_PID").unwrap()).unwrap();
    assert_eq!(receive(&mut socket).unwrap(), Some(b'?'));
    socket
        .write_all(if is_null("/dev/stdout").unwrap() {
            b"N"
        } else {
            b"P"
        })
        .unwrap();
    assert_eq!(receive(&mut socket).unwrap(), Some(b'!'));
    write_stdout(b"descendant output").unwrap();
    leave(0)
}

/// Behaviours of the held children of the lifecycle tests, selected by `CHILD_MODE`.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn held_child() {
    let mode = HeldMode::from_variable();
    match mode {
        HeldMode::Live | HeldMode::Exit => {
            let closed = closed_streams();
            write_stdout(b"early").unwrap();
            write_stderr(b"diag").unwrap();
            if mode == HeldMode::Live {
                close_streams(&closed).unwrap();
                hold();
                finish(&closed);
            }
            let port = variable("CHILD_PORT").unwrap();
            let closed_text = Stream::list_text(&closed);
            let variables = [
                ("CHILD_PORT", port.as_str()),
                ("CHILD_CLOSED", &closed_text),
                ("CHILD_AFTER_EXIT", "1"),
            ];
            std::mem::forget(spawn_descendant(held_descendant, &variables));
            leave(0)
        }
        HeldMode::Term | HeldMode::Flood => {
            let (reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
            signal_hook::low_level::pipe::register(signal_hook::consts::SIGTERM, writer).unwrap();
            let _socket = ready();
            (&reader).read_exact(&mut [0]).unwrap();
            let body = if mode == HeldMode::Term {
                b"terminated".to_vec()
            } else {
                [vec![b'x'; 200_000], b"end".to_vec()].concat()
            };
            write_stdout(&body).unwrap();
            leave(0)
        }
        HeldMode::Kill => {
            let port = variable("CHILD_PORT").unwrap();
            let descendant = command(held_descendant, &[("CHILD_PORT", port.as_str())])
                .spawn()
                .unwrap();
            std::mem::forget(descendant);
            hold();
            leave(0)
        }
    }
}

/// A holder that outlives the direct child of [`held_child`]: with `CHILD_AFTER_EXIT` it
/// waits for that child to end, closes the streams `CHILD_CLOSED` names, then holds.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn held_descendant() {
    if std::env::var_os("CHILD_AFTER_EXIT").is_some() {
        await_parent_exit();
    }
    if std::env::var_os("CHILD_CLOSED").is_none() {
        hold();
        leave(0)
    }
    let closed = closed_streams();
    close_streams(&closed).unwrap();
    hold();
    finish(&closed)
}

/// Checks its stream, directory and environment, then writes raw bytes to both streams.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn raw_stream_writer() {
    assert!(read_input().unwrap().is_empty());
    let mut diagnostic = b"diag\0\xff".to_vec();
    diagnostic.extend(vec![b'e'; 300_000]);
    write_stderr(&diagnostic).unwrap();
    let argument = std::env::args().next_back().unwrap();
    let directory = std::env::current_dir().unwrap();
    let mut head = format!(
        "{argument}|{}|{}",
        directory.display(),
        variable("PROBE").unwrap()
    )
    .into_bytes();
    head.extend_from_slice(b"\0\xe2");
    write_stdout(&head).unwrap();
    write_stdout(&[vec![b'o'; 300_000], b"\x82\xac\xff".to_vec()].concat()).unwrap();
    leave(0)
}

/// Writes the combined bytes `CHILD_OUT` and `CHILD_ERR` name and ends; with
/// `CHILD_STOPPED` set it then waits for the signal that stops it instead.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn sized_writer() {
    let size = |name| variable(name).unwrap().parse::<usize>().unwrap();
    write_stdout(&vec![b'x'; size("CHILD_OUT")]).unwrap();
    write_stderr(&vec![b'y'; size("CHILD_ERR")]).unwrap();
    if std::env::var_os("CHILD_STOPPED").is_some() {
        wait_for_signal()
    }
    leave(0)
}

/// Overflows a bounded capture after it reported its pid and was released.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn overflowing_writer() {
    let mut socket = parent();
    announce(&mut socket, &std::process::id().to_string()).unwrap();
    receive(&mut socket).unwrap();
    write_stdout(&vec![b'x'; 100_000]).unwrap();
    receive(&mut socket).unwrap();
    leave(0)
}

/// Reports the fields the synchronous capture test expects.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn capture_probe() {
    assert!(read_input().unwrap().is_empty());
    write_stderr(b"ignored").unwrap();
    let arguments: Vec<String> = std::env::args().collect();
    let flag = |null| if null { "True" } else { "False" };
    let mut fields = arguments[arguments.len() - 3..].to_vec();
    fields.extend([
        variable("MAESTRO_CAPTURE").unwrap(),
        std::env::current_dir().unwrap().display().to_string(),
        flag(is_null("/dev/stdin").unwrap()).to_owned(),
        flag(is_null("/dev/stderr").unwrap()).to_owned(),
    ]);
    let mut body = fields.join("|").into_bytes();
    body.extend_from_slice(b"\0\xff");
    write_stdout(&body).unwrap();
    leave(variable("CHILD_EXIT").unwrap().parse().unwrap())
}

/// Ends through a termination signal it raises on itself.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn self_terminating() {
    kill_self(rustix::process::Signal::TERM)
}

/// Prints its pid, session and process group.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn session_ids() {
    let session = rustix::process::getsid(None).unwrap();
    let group = rustix::process::getpgrp();
    let ids = (session.as_raw_nonzero(), group.as_raw_nonzero());
    write_stdout(format!("{} {} {}\n", std::process::id(), ids.0, ids.1).as_bytes()).unwrap();
    leave(0)
}

/// Prints its session and process group.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn sync_session_ids() {
    let session = rustix::process::getsid(None).unwrap();
    let group = rustix::process::getpgrp();
    write_stdout(format!("{} {}\n", session.as_raw_nonzero(), group.as_raw_nonzero()).as_bytes())
        .unwrap();
    leave(0)
}

/// Writes a prefix, then waits for a signal.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn prefix_then_wait() {
    write_stdout(b"before-kill").unwrap();
    wait_for_signal()
}

/// Writes each `(descriptor, bytes)` pair of `CHILD_WRITES` once the parent saw the
/// harness start bytes and acknowledged the previous write.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn acknowledged_writer() {
    let mut socket = parent();
    let writes = [
        (Stream::Output, &b"a\0"[..]),
        (Stream::Error, b"b"),
        (Stream::Output, b"c"),
    ];
    for (index, (stream, bytes)) in writes.into_iter().enumerate() {
        assert_eq!(receive(&mut socket).unwrap(), Some(b'g'), "write {index}");
        match stream {
            Stream::Error => write_stderr(bytes),
            _ => write_stdout(bytes),
        }
        .unwrap();
    }
    assert_eq!(receive(&mut socket).unwrap(), Some(b'g'));
    leave(0)
}

/// Writes `one` to standard output and `two` to standard error, each after a release.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn two_stream_writer() {
    let mut socket = parent();
    assert_eq!(receive(&mut socket).unwrap(), Some(b'g'));
    write_stdout(b"one").unwrap();
    assert_eq!(receive(&mut socket).unwrap(), Some(b'g'));
    write_stderr(b"two").unwrap();
    leave(0)
}

/// Prints readiness, reads one line, answers on both streams and ends with status 3.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn line_answerer() {
    write_stdout(b"ready\n").unwrap();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
    write_stderr(b"diag\xff").unwrap();
    let arguments: Vec<String> = std::env::args().collect();
    let mut fields = vec![line.trim().to_owned()];
    fields.extend(arguments[arguments.len() - 2..].iter().cloned());
    fields.extend([
        std::env::current_dir().unwrap().display().to_string(),
        variable("PROBE").unwrap(),
    ]);
    write_stdout(fields.join("|").as_bytes()).unwrap();
    leave(3)
}

/// Reports readiness and waits for its input to end, surviving a termination request when
/// `CHILD_SURVIVES` is set.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn terminable() {
    let survived = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if std::env::var_os("CHILD_SURVIVES").is_some() {
        signal_hook::flag::register(signal_hook::consts::SIGTERM, survived).unwrap();
    }
    write_stdout(b"ready\n").unwrap();
    read_input().unwrap();
    leave(0)
}

/// Ends at once with no output of its own.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn immediate_exit() {
    leave(0)
}

/// Writes raw bytes, closes its output while still running, then reports on its error
/// stream around one byte of input and ends with status 7.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn closed_output_then_exit() {
    write_stdout(b"prefix\0\xff").unwrap();
    close_streams(&[Stream::Output]).unwrap();
    write_stderr(b"ready").unwrap();
    std::io::stdin().read_exact(&mut [0]).unwrap();
    write_stderr(b"last").unwrap();
    leave(7)
}

/// Announces readiness, then echoes its input.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn ready_then_echo() {
    write_stdout(b"ready").unwrap();
    write_stdout(&read_input().unwrap()).unwrap();
    leave(0)
}

/// Blocks the signals the pseudo-terminal launch must reset, then replaces itself with
/// the test executable and the arguments `CHILD_EXEC_ARGS` carries, so the blocked mask
/// reaches the launch.
#[test]
#[ignore = "runs only inside a controlled child"]
pub fn blocked_signals_launcher() {
    use nix::sys::signal::{SigSet, SigmaskHow, Signal, sigprocmask};
    let mut blocked = SigSet::empty();
    for signal in [
        Signal::SIGHUP,
        Signal::SIGINT,
        Signal::SIGQUIT,
        Signal::SIGALRM,
        Signal::SIGTERM,
        Signal::SIGCHLD,
    ] {
        blocked.add(signal);
    }
    sigprocmask(SigmaskHow::SIG_BLOCK, Some(&blocked), None).unwrap();
    let program = std::ffi::CString::new(
        std::env::current_exe()
            .unwrap()
            .into_os_string()
            .into_encoded_bytes(),
    )
    .unwrap();
    let mut arguments = vec![program.clone()];
    for argument in variable("CHILD_EXEC_ARGS").unwrap().split('\u{1f}') {
        arguments.push(std::ffi::CString::new(argument).unwrap());
    }
    nix::unistd::execv(&program, &arguments).unwrap();
}
