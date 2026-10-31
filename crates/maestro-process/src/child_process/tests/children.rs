//! Controlled children the owner's unit tests start by re-running this test executable.
//!
//! Each program is an ignored test that only a parent test selects. Its arguments are
//! variables, and it ends through `std::process::exit` so no harness epilogue follows its
//! output. The harness writes [`HARNESS_START`] to standard output before the program runs.
use maestro_test_process::{HARNESS_START, TestProcess};
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

/// The standard output a program produces when it writes `body`.
pub(in crate::child_process) fn harness(body: &[u8]) -> Vec<u8> {
    [HARNESS_START, body].concat()
}

/// The text of variable `name`.
fn variable(name: &str) -> String {
    std::env::var(name).unwrap()
}

/// Writes `bytes` to standard output and flushes them.
fn write_stdout(bytes: &[u8]) -> std::io::Result<()> {
    let mut out = std::io::stdout();
    out.write_all(bytes).and_then(|()| out.flush())
}

/// Ends the process with `code` after flushing standard output.
fn leave(code: i32) -> ! {
    let _ = std::io::stdout().flush();
    std::process::exit(code)
}

/// Everything standard input holds until it ends.
fn read_input() -> Vec<u8> {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    input
}

/// Waits for one byte from `socket`; `None` once the peer closed it.
fn receive(socket: &mut TcpStream) -> Option<u8> {
    let mut byte = [0];
    (socket.read(&mut byte).unwrap() == 1).then_some(byte[0])
}

/// A connection to the port variable `name` names on this host.
fn connect_to(name: &str) -> TcpStream {
    TcpStream::connect(("127.0.0.1", variable(name).parse().unwrap())).unwrap()
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

/// Waits until the process that started this descendant ended.
fn await_parent_exit() {
    receive(&mut connect_to("CHILD_LINK_PORT"));
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
    receive(&mut socket);
    if std::env::var("CHILD_AFTER").as_deref() == Ok("echo") {
        write_stdout(&[read_input(), b" complete".to_vec()].concat()).unwrap();
        std::io::stderr().write_all(b"diag").unwrap();
    }
    leave(0)
}

/// Writes to both output streams, leaves a descendant that keeps them, and ends.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn inherited_output() {
    write_stdout(b"prefix").unwrap();
    std::io::stderr().write_all(b"diag").unwrap();
    let port = variable("CHILD_PORT");
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
    if receive(&mut socket).is_some() {
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
        let port = variable("CHILD_PORT");
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
    hold_input(&variable("PARENT_PID"));
}

/// Reports `pid`, waits for release, and with `CHILD_OVERFLOW` overflows and finishes.
fn hold_input(pid: &str) -> ! {
    let mut socket = connect_to("CHILD_PORT");
    socket.write_all(format!("{pid}\n").as_bytes()).unwrap();
    receive(&mut socket);
    if !std::env::var("CHILD_OVERFLOW")
        .unwrap_or_default()
        .is_empty()
    {
        write_stdout(b"overflow").unwrap();
        close_streams("12");
        receive(&mut socket);
        socket.write_all(b"done").unwrap();
    }
    leave(0)
}

/// Prints readiness and waits for its input to end.
#[test]
#[ignore = "runs only inside a controlled child"]
pub(in crate::child_process) fn ready_then_wait() {
    write_stdout(b"ready\n").unwrap();
    read_input();
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
