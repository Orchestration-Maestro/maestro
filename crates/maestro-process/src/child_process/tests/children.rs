//! Controlled children the crate's unit tests start by re-running this test executable.
//!
//! Each program is an ignored test that only a parent test selects. Its arguments are
//! variables, and it ends through `std::process::exit` so no harness epilogue follows its
//! output. The harness writes the start bytes to standard output before the program runs.
use maestro_test_process::child::{
    Stream, announce, close_streams, connect, hex_variable, leave, port, read_input, receive,
    test_child_command, variable, write_stderr, write_stdout,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};

/// The command the owner builds for the checked child `program` with `variables` added.
pub(crate) fn command(program: impl Fn(), variables: &[(&str, &str)]) -> Command {
    test_child_command(program, variables).unwrap()
}

/// [`command`](children::command) as the asynchronous command the owner takes.
pub(crate) fn async_command(
    program: impl Fn(),
    variables: &[(&str, &str)],
) -> tokio::process::Command {
    test_child_command(program, variables).unwrap().into()
}

/// A connection to the port variable `name` names on this host.
fn connect_to(name: &str) -> TcpStream {
    connect(port(name).unwrap()).unwrap()
}

/// Starts `program` as a descendant that keeps this process's three descriptors, linked to
/// this process by a connection this process holds until it ends; returns its process id.
///
/// The descendant waits on [`await_parent_exit`] for that connection to close.
fn spawn_linked_child(program: impl Fn(), variables: &[(&str, &str)]) -> u32 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let mut command = test_child_command(program, variables).unwrap();
    command
        .env("CHILD_LINK_PORT", &port)
        .stdin(Stdio::inherit());
    let child = command.spawn().unwrap();
    std::mem::forget(listener.accept().unwrap().0);
    let pid = child.id();
    std::mem::forget(child);
    pid
}

/// [`spawn_linked_child`] for a descendant whose process id no test reads.
fn spawn_linked(program: impl Fn(), variables: &[(&str, &str)]) {
    let _ = spawn_linked_child(program, variables);
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

/// Reports readiness, answers each termination signal with `T` while surviving it, and
/// ends on the first byte the parent sends; `CHILD_AFTER` set to `echo` then echoes its
/// input.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(crate) fn stubborn() {
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
pub(crate) fn inherited_output() {
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
pub(crate) fn inherited_output_descendant() {
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

/// Writes the bytes `CHILD_OUT` and `CHILD_ERR` name in hex to the two output streams, then
/// ends through the status `CHILD_EXIT` names or the signal `CHILD_SIGNAL` numbers.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(crate) fn output_writer() {
    write_stderr(&hex_variable("CHILD_ERR").unwrap()).unwrap();
    write_stdout(&hex_variable("CHILD_OUT").unwrap()).unwrap();
    if let Ok(number) = variable("CHILD_SIGNAL") {
        signal_hook::low_level::raise(number.parse().unwrap()).unwrap();
        hold_forever()
    }
    leave(variable("CHILD_EXIT").map_or(0, |code| code.parse().unwrap()))
}

/// Writes to standard output until it is killed.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(crate) fn endless_writer() {
    loop {
        write_stdout(&[b'y'; 4096]).unwrap();
    }
}

/// Writes `READY`, then waits for its input to end.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(crate) fn ready_then_drain() {
    write_stdout(b"READY").unwrap();
    read_input().unwrap();
    leave(0)
}

/// Reports on the port `CHILD_PORT` names the process id of a descendant holding both
/// output pipes, writes `diag` to standard error, then ends once `CHILD_GATE` can be read.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(crate) fn pipe_holder() {
    let mut report = connect_to("CHILD_PORT");
    let descendant = spawn_linked_child(pipe_holder_descendant, &[]);
    announce(&mut report, &descendant.to_string()).unwrap();
    write_stderr(b"diag").unwrap();
    std::fs::read(variable("CHILD_GATE").unwrap()).unwrap();
    leave(0)
}

/// The descendant of [`pipe_holder`]: it keeps both output pipes until the parent test ends
/// it.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(crate) fn pipe_holder_descendant() {
    await_parent_exit();
    hold_forever()
}

/// Waits for a signal to end this process, which never ends on its own.
fn hold_forever() -> ! {
    loop {
        std::thread::park();
    }
}

/// Writes a prefix, then waits for a signal.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(crate) fn prefix_then_wait() {
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
pub(crate) fn input_hold() {
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
pub(crate) fn input_hold_descendant() {
    await_parent_exit();
    if std::env::var("CHILD_OVERFLOW")
        .unwrap_or_default()
        .is_empty()
    {
        close_streams(&Stream::OUTPUTS).unwrap();
    }
    hold_input(&variable("PARENT_PID").unwrap());
}

/// Reports `pid`, waits for release, and with `CHILD_OVERFLOW` overflows and finishes.
fn hold_input(pid: &str) -> ! {
    let mut socket = connect_to("CHILD_PORT");
    announce(&mut socket, pid).unwrap();
    receive(&mut socket).unwrap();
    if !std::env::var("CHILD_OVERFLOW")
        .unwrap_or_default()
        .is_empty()
    {
        write_stdout(b"overflow").unwrap();
        close_streams(&Stream::OUTPUTS).unwrap();
        receive(&mut socket).unwrap();
        socket.write_all(b"done").unwrap();
    }
    leave(0)
}

/// Prints readiness and waits for its input to end.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(crate) fn ready_then_wait() {
    write_stdout(b"ready\n").unwrap();
    read_input().unwrap();
    leave(0)
}

/// Writes a prefix and a sibling note, then waits for one byte of input.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(crate) fn prefix_and_sibling() {
    write_stdout(b"prefix").unwrap();
    std::io::stderr().write_all(b"sibling").unwrap();
    std::io::stdin().read_exact(&mut [0]).unwrap();
    leave(0)
}

/// Starts [`link_probe`] as a linked descendant, tells the parent test, and ends once the
/// parent test releases it.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(crate) fn link_holder() {
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
pub(crate) fn link_probe() {
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
