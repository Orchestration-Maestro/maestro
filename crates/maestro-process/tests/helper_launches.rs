//! Public helper launch semantics.
#![cfg(test)]
#![cfg(unix)]
use maestro_process::{OutputMode, SpawnSyncOptions, spawn_sync};
use tokio::process::Command;

#[test]
fn sync_input_roundtrips_bytes_and_closes_the_pipe() {
    let script = r"
import os, sys
null = os.fstat(0).st_rdev == os.stat(os.devnull).st_rdev
os.write(1, (b'null:' if null else b'pipe:') + sys.stdin.buffer.read())
os.write(2, b'diag\x00\xff')
sys.exit(int(sys.argv[1]))
";
    for input in [None, Some(&b""[..]), Some(&b"\0\xff\r\n\xe2\x82\xac"[..])] {
        let mut expected = if input.is_some() {
            b"pipe:".to_vec()
        } else {
            b"null:".to_vec()
        };
        expected.extend_from_slice(input.unwrap_or_default());
        for (stdout, stderr, code) in [
            (OutputMode::Capture, OutputMode::Capture, 0),
            (OutputMode::Capture, OutputMode::Capture, 7),
            (OutputMode::Capture, OutputMode::Discard, 0),
            (OutputMode::Capture, OutputMode::Discard, 7),
            (OutputMode::Discard, OutputMode::Capture, 0),
            (OutputMode::Discard, OutputMode::Capture, 7),
        ] {
            let mut command = Command::new("python3");
            command.args(["-c", script, &code.to_string()]);
            let output = spawn_sync(
                &mut command,
                SpawnSyncOptions::Capture {
                    max_buffer_bytes: None,
                    stdout,
                    input,
                    timeout: None,
                    stderr,
                },
            )
            .unwrap();
            assert_eq!(
                output.stdout,
                if stdout == OutputMode::Capture {
                    &expected[..]
                } else {
                    b""
                }
            );
            assert_eq!(
                output.stderr,
                if stderr == OutputMode::Capture {
                    &b"diag\0\xff"[..]
                } else {
                    b""
                }
            );
            assert_eq!(output.status.code(), Some(code));
        }
    }
}

#[test]
fn sync_input_and_output_progress_concurrently() {
    let mut command = Command::new("python3");
    command.args(["-c", "import os,sys; os.write(1,b'y'*262144); data=sys.stdin.buffer.read(); os.write(1,data[:2]); os.write(2,str(len(data)).encode())"]);
    let input = vec![b'x'; 262_144];
    let output = spawn_sync(
        &mut command,
        SpawnSyncOptions::Capture {
            max_buffer_bytes: None,
            stdout: OutputMode::Capture,
            input: Some(&input),
            timeout: None,
            stderr: OutputMode::Capture,
        },
    )
    .unwrap();
    let mut expected = vec![b'y'; 262_144];
    expected.extend_from_slice(b"xx");
    assert_eq!(output.stdout, expected);
    assert_eq!(output.stderr, b"262144");
    assert!(output.status.success());
}

/// A child control socket, also used as a completion witness.
fn controlled(script: &str) -> (Command, std::net::TcpListener) {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let mut command = Command::new("python3");
    command.args([
        "-c",
        script,
        &listener.local_addr().unwrap().port().to_string(),
    ]);
    (command, listener)
}

/// Receive a ready child's pid before releasing it.
fn ready(listener: &std::net::TcpListener) -> (std::net::TcpStream, rustix::process::Pid) {
    use std::io::{BufRead, BufReader};
    let (socket, _) = listener.accept().unwrap();
    let mut pid = String::new();
    BufReader::new(&socket).read_line(&mut pid).unwrap();
    (
        socket,
        rustix::process::Pid::from_raw(pid.trim().parse().unwrap()).unwrap(),
    )
}

/// Native wait after the owner's joined worker must find no waitable child.
fn reaped(pid: rustix::process::Pid) {
    assert_eq!(
        rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG).unwrap_err(),
        rustix::io::Errno::CHILD
    );
}

/// A ready child waits for release, consumes stdin to EOF, and reports its bytes.
const CONSUMER: &str = r"
import os, socket, sys
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
s.sendall(str(os.getpid()).encode() + b'\n')
assert s.recv(1) == b'!'
data = sys.stdin.buffer.read()
s.sendall(data)
s.close()
";

#[test]
fn sync_input_error_waits_for_child_completion() {
    use std::io::{Read, Write};
    let script = r"
import os, socket, sys
os.close(0)
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
s.sendall(str(os.getpid()).encode() + b'\n')
assert s.recv(1) == b'!'
s.sendall(b'effect')
s.close()
";
    let (mut command, listener) = controlled(script);
    let worker = std::thread::spawn(move || {
        let input = vec![b'x'; 4_194_304];
        spawn_sync(
            &mut command,
            SpawnSyncOptions::Capture {
                max_buffer_bytes: None,
                stdout: OutputMode::Capture,
                input: Some(&input),
                timeout: None,
                stderr: OutputMode::Discard,
            },
        )
    });
    let (mut socket, pid) = ready(&listener);
    socket.write_all(b"!").unwrap();
    let error = worker.join().unwrap().unwrap_err();
    assert!(
        matches!(error, maestro_process::SpawnSyncError::Wait(ref error) if error.kind() == std::io::ErrorKind::BrokenPipe)
    );
    let mut effect = Vec::new();
    socket.read_to_end(&mut effect).unwrap();
    assert_eq!(effect, b"effect");
    reaped(pid);
}

#[test]
fn detached_launch_returns_before_input_consumption_and_reaps() {
    use std::io::{Read, Write};
    let (command, listener) = controlled(CONSUMER);
    let input = vec![b'x'; 262_144];
    let worker = maestro_process::spawn(command, Some(input.clone())).unwrap();
    let (mut socket, pid) = ready(&listener);
    socket.write_all(b"!").unwrap();
    let mut observed = Vec::new();
    socket.read_to_end(&mut observed).unwrap();
    worker.join().unwrap();
    assert_eq!(observed, input);
    reaped(pid);
}

/// A ready child reports its own process group and its parent's, once released.
const GROUPS: &str = r"
import os, socket, sys
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
s.sendall(str(os.getpid()).encode() + b'\n')
assert s.recv(1) == b'!'
s.sendall(f'{os.getpgid(0)} {os.getpgid(os.getppid())}'.encode())
s.close()
";

#[test]
fn detached_launch_runs_in_its_own_session_outside_the_callers_process_group() {
    use std::io::{Read, Write};
    let (command, listener) = controlled(GROUPS);
    let worker = maestro_process::spawn(command, None).unwrap();
    let (mut socket, pid) = ready(&listener);
    socket.write_all(b"!").unwrap();
    let mut groups = String::new();
    socket.read_to_string(&mut groups).unwrap();
    worker.join().unwrap();
    let (own, parent) = groups.split_once(' ').unwrap();
    let (own, parent): (i32, i32) = (own.parse().unwrap(), parent.parse().unwrap());
    assert_eq!(own, pid.as_raw_nonzero().get());
    assert_ne!(own, parent);
    reaped(pid);
}

#[test]
fn detached_input_closure_does_not_prevent_reaping() {
    let script = r"
import os, socket, sys
os.close(0)
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
s.sendall(str(os.getpid()).encode() + b'\n')
s.close()
";
    let (command, listener) = controlled(script);
    let worker = maestro_process::spawn(command, Some(vec![b'x'; 4_194_304])).unwrap();
    let (_, pid) = ready(&listener);
    worker.join().unwrap();
    reaped(pid);
}

#[test]
fn detached_spawn_errors_return_without_a_handle() {
    use std::io::ErrorKind;
    let missing =
        std::env::temp_dir().join(format!("maestro-missing-detached-{}", std::process::id()));
    for (path, kind) in [
        (&missing, ErrorKind::NotFound),
        (&std::env::temp_dir(), ErrorKind::PermissionDenied),
    ] {
        let native = std::process::Command::new(path).spawn().unwrap_err();
        let error = maestro_process::spawn(Command::new(path), None).unwrap_err();
        assert_eq!(error.kind(), kind);
        assert_eq!(error.raw_os_error(), native.raw_os_error());
    }
    let mut command = Command::new("python3");
    command.current_dir(missing);
    assert_eq!(
        maestro_process::spawn(command, None).unwrap_err().kind(),
        ErrorKind::NotFound
    );
}

#[test]
fn detached_worker_survives_a_dropped_handle() {
    use std::io::{Read, Write};
    let (command, listener) = controlled(CONSUMER);
    let input = b"dropped\0\xff".to_vec();
    let worker = maestro_process::spawn(command, Some(input.clone())).unwrap();
    let (mut socket, _) = ready(&listener);
    drop(worker);
    socket.write_all(b"!").unwrap();
    let mut observed = Vec::new();
    socket.read_to_end(&mut observed).unwrap();
    assert_eq!(observed, input);
}

#[test]
fn inherited_launch_keeps_descriptors_operands_and_status() {
    use std::{io::Write, process::Stdio};
    const CHILD: &str = "MAESTRO_INHERIT_CHILD";
    if std::env::var_os(CHILD).is_some() {
        inherited_child();
        return;
    }
    for code in ["0", "7", "signal"] {
        let mut child = maestro_test_process::TestProcess::current(
            "inherited_launch_keeps_descriptors_operands_and_status",
        )
        .unwrap()
        .nocapture()
        .env(CHILD, "1")
        .env("MAESTRO_EXIT", code)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"inherited-input")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "isolated inherited launch: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output
                .stdout
                .windows(b"inherited-out\0\xff".len())
                .any(|part| part == b"inherited-out\0\xff")
        );
        assert_eq!(output.stderr, b"inherited-err");
    }
}

/// Run the inherited launch inside the isolated descriptor owner.
fn inherited_child() {
    use std::os::unix::process::ExitStatusExt;
    let code = std::env::var("MAESTRO_EXIT").unwrap();
    let directory = std::env::temp_dir().canonicalize().unwrap();
    let mut command = Command::new("python3");
    command
        .args([
            "-c",
            r"
import os, signal, sys
assert sys.stdin.buffer.read() == b'inherited-input'
assert sys.argv[2:] == ['literal $HOME;|', '', 'repeat', 'repeat']
assert os.environ['MAESTRO_OPERAND'] == 'chosen'
assert os.getcwd() == os.path.realpath(os.environ['MAESTRO_DIRECTORY'])
os.write(1, b'inherited-out\x00\xff')
os.write(2, b'inherited-err')
if sys.argv[1] == 'signal': os.kill(os.getpid(), signal.SIGTERM)
sys.exit(int(sys.argv[1]))
",
            &code,
            "literal $HOME;|",
            "",
            "repeat",
            "repeat",
        ])
        .current_dir(&directory)
        .env("MAESTRO_DIRECTORY", &directory)
        .env("MAESTRO_OPERAND", "chosen");
    let output = spawn_sync(&mut command, SpawnSyncOptions::Inherit).unwrap();
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    if code == "signal" {
        assert_eq!(output.status.signal(), Some(15));
    } else {
        assert_eq!(output.status.code(), Some(code.parse().unwrap()));
    }
    assert!(matches!(
        spawn_sync(
            &mut Command::new("/maestro-missing-inherited"),
            SpawnSyncOptions::Inherit
        ),
        Err(maestro_process::SpawnSyncError::Spawn(_))
    ));
}

#[test]
fn sync_discarded_stdout_does_not_wait_for_a_descendant() {
    use std::io::{Read, Write};
    let script = r"
import os, socket, sys
assert sys.stdin.buffer.read() == b'copied'
pid = os.getpid()
gone, alive = os.pipe()
if os.fork():
    os.close(gone)
    sys.exit(7)
os.close(alive); os.read(gone, 1); os.close(gone)
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
s.sendall(str(pid).encode() + b'\n')
assert s.recv(1) == b'?'
null = os.fstat(1).st_rdev == os.stat(os.devnull).st_rdev
s.sendall(b'N' if null else b'P')
assert s.recv(1) == b'!'
os.write(1, b'descendant output')
s.close(); os._exit(0)
";
    let (mut command, listener) = controlled(script);
    let worker = std::thread::spawn(move || {
        spawn_sync(
            &mut command,
            SpawnSyncOptions::Capture {
                max_buffer_bytes: None,
                stdout: OutputMode::Discard,
                input: Some(b"copied"),
                timeout: None,
                stderr: OutputMode::Discard,
            },
        )
    });
    let (mut socket, pid) = ready(&listener);
    socket.write_all(b"?").unwrap();
    let mut descriptor = [0];
    socket.read_exact(&mut descriptor).unwrap();
    // Release a captured-pipe mutant before joining so it cannot strand the holder.
    if descriptor != *b"N" {
        socket.write_all(b"!").unwrap();
    }
    let output = worker.join().unwrap().unwrap();
    if descriptor == *b"N" {
        reaped(pid);
        socket.write_all(b"!").unwrap();
    }
    let mut rest = Vec::new();
    socket.read_to_end(&mut rest).unwrap();
    assert_eq!(
        descriptor, *b"N",
        "discarded stdout must use the null device"
    );
    assert_eq!(output.status.code(), Some(7));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(rest.is_empty());
}
