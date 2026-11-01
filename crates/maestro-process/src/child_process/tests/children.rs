//! Controlled children the owner's unit tests start by re-running this test executable.
//!
//! Each program is an ignored test that only a parent test selects. Its arguments are
//! variables, and it ends through `std::process::exit` so no harness epilogue follows its
//! output. The harness writes the start bytes to standard output before the program runs.
use maestro_test_process::TestProcess;
use maestro_test_process::child::{
    announce, connect, leave, port, read_input, receive, variable, write_stderr, write_stdout,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};

/// The command that runs `program` with `variables` added to its environment.
pub(in crate::child_process) fn command(program: impl Fn(), variables: &[(&str, &str)]) -> Command {
    TestProcess::child(program)
        .unwrap()
        .envs(variables.iter().copied())
        .command()
        .unwrap()
}

/// [`command`] as the asynchronous command the owner takes.
pub(in crate::child_process) fn async_command(
    program: impl Fn(),
    variables: &[(&str, &str)],
) -> tokio::process::Command {
    command(program, variables).into()
}

/// A connection to the port variable `name` names on this host.
fn connect_to(name: &str) -> TcpStream {
    connect(port(name).unwrap()).unwrap()
}

/// Starts `program` as a descendant that keeps this process's three descriptors, and
/// returns the link that closes when this process ends.
///
/// The descendant waits on [`await_parent_exit`] for that closure.
fn spawn_linked(program: impl Fn(), variables: &[(&str, &str)]) -> TcpStream {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let mut command = command(program, variables);
    command
        .env("CHILD_LINK_PORT", &port)
        .stdin(Stdio::inherit());
    std::mem::forget(command.spawn().unwrap());
    listener.accept().unwrap().0
}

/// The link to the process that started this descendant.
fn parent_link() -> TcpStream {
    connect_to("CHILD_LINK_PORT")
}

/// Waits until the process that started this descendant ended.
fn await_parent_exit() {
    receive(&mut parent_link()).unwrap();
}

/// An operation that writes bytes to one output stream.
type Writer = fn(&[u8]) -> std::io::Result<()>;

/// Replaces each standard descriptor named in `streams` with the null device, so the
/// parent observes the end of the pipe it held.
fn close_streams(streams: &str) {
    let null = std::fs::File::open("/dev/null").unwrap();
    for stream in streams.chars() {
        match stream {
            '1' => rustix::stdio::dup2_stdout(&null),
            _ => rustix::stdio::dup2_stderr(&null),
        }
        .unwrap();
    }
}

/// Reports readiness, answers each termination signal with `T` while surviving it, and
/// ends on the first byte the parent sends; `CHILD_AFTER` set to `echo` then echoes its
/// input.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn stubborn() {
    let (reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
    signal_hook::low_level::pipe::register(signal_hook::consts::SIGTERM, writer).unwrap();
    let mut socket = connect_to("CHILD_PORT");
    let mut answers = socket.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut reader = reader;
        while reader.read_exact(&mut [0]).is_ok() {
            answers.write_all(b"T").unwrap();
        }
    });
    socket.write_all(b"READY").unwrap();
    receive(&mut socket).unwrap();
    if std::env::var("CHILD_AFTER").as_deref() == Ok("echo") {
        write_stdout(&[read_input().unwrap(), b" complete".to_vec()].concat()).unwrap();
        write_stderr(b"diag").unwrap();
    }
    leave(0)
}

/// Writes to both output streams, leaves a descendant that keeps them, and ends.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn inherited_output() {
    write_stdout(b"prefix").unwrap();
    write_stderr(b"diag").unwrap();
    let port = variable("CHILD_PORT").unwrap();
    spawn_linked(inherited_output_descendant, &[("CHILD_PORT", &port)]);
    leave(0)
}

/// The descendant of [`inherited_output`]: after its parent ended and the control socket
/// released it, it reports whether each output stream still has a reader.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn inherited_output_descendant() {
    await_parent_exit();
    let mut socket = connect_to("CHILD_PORT");
    if receive(&mut socket).unwrap().is_some() {
        let mut result = Vec::new();
        let writers: [Writer; 2] = [
            |bytes| write_stdout(bytes),
            |bytes| std::io::stderr().write_all(bytes),
        ];
        for write in writers {
            let closed = write(b"tail").is_err_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe);
            result.extend_from_slice(if closed { b"closed" } else { b"writable" });
        }
        socket.write_all(&result).unwrap();
    }
    leave(0)
}

/// Writes a prefix, then waits for a signal.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn prefix_then_wait() {
    write_stdout(b"prefix").unwrap();
    loop {
        std::thread::park();
    }
}

/// Reports its pid and holds its input unread; with `CHILD_EXITED` set it first ends and
/// leaves that holder as a descendant, and with `CHILD_OVERFLOW` set the holder writes
/// output and closes its streams once released.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn input_hold() {
    let pid = std::process::id().to_string();
    if std::env::var_os("CHILD_EXITED").is_some() {
        let port = variable("CHILD_PORT").unwrap();
        let overflow = std::env::var("CHILD_OVERFLOW").unwrap_or_default();
        let variables = [
            ("CHILD_PORT", port.as_str()),
            ("CHILD_OVERFLOW", overflow.as_str()),
            ("PARENT_PID", pid.as_str()),
        ];
        spawn_linked(input_hold_descendant, &variables);
        leave(0)
    }
    hold_input(&pid);
}

/// The descendant of [`input_hold`]: once its parent ended it holds the inherited input.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn input_hold_descendant() {
    await_parent_exit();
    if std::env::var("CHILD_OVERFLOW")
        .unwrap_or_default()
        .is_empty()
    {
        close_streams("12");
    }
    hold_input(&variable("PARENT_PID").unwrap());
}

/// Reports `pid`, waits for release, and with `CHILD_OVERFLOW` overflows and finishes.
fn hold_input(pid: &str) -> ! {
    let mut socket = connect_to("CHILD_PORT");
    socket.write_all(format!("{pid}\n").as_bytes()).unwrap();
    receive(&mut socket).unwrap();
    if !std::env::var("CHILD_OVERFLOW")
        .unwrap_or_default()
        .is_empty()
    {
        write_stdout(b"overflow").unwrap();
        close_streams("12");
        receive(&mut socket).unwrap();
        socket.write_all(b"done").unwrap();
    }
    leave(0)
}

/// Prints readiness and waits for its input to end.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn ready_then_wait() {
    write_stdout(b"ready\n").unwrap();
    read_input().unwrap();
    leave(0)
}

/// Writes a prefix and a sibling note, then waits for one byte of input.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn prefix_and_sibling() {
    write_stdout(b"prefix").unwrap();
    std::io::stderr().write_all(b"sibling").unwrap();
    std::io::stdin().read_exact(&mut [0]).unwrap();
    leave(0)
}

/// Starts [`link_probe`] as a linked descendant, tells the parent test, and ends once the
/// parent test releases it.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn link_holder() {
    let mut control = connect_to("CHILD_PORT");
    let port = variable("CHILD_PORT").unwrap();
    spawn_linked(link_probe, &[("CHILD_PORT", &port)]);
    announce(&mut control, "linked").unwrap();
    receive(&mut control).unwrap();
    leave(0)
}

/// The descendant of [`link_holder`]: once the parent test says so it reports `H` when its
/// link to the parent is still open and `D` when the link already ended.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn link_probe() {
    let link = parent_link();
    let mut control = connect_to("CHILD_PORT");
    assert_eq!(receive(&mut control).unwrap(), Some(b'p'));
    link.set_nonblocking(true).unwrap();
    let state = match (&link).read(&mut [0]) {
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => b'H',
        Ok(0) => b'D',
        other => panic!("unexpected link read: {other:?}"),
    };
    control.write_all(&[state]).unwrap();
    leave(0)
}
