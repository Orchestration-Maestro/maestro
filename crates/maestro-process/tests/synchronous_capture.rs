//! Synchronous capture through the public native process interface.
#![cfg(unix)]
use maestro_process::spawn_sync;
use std::{os::unix::process::ExitStatusExt, time::Duration};
use tokio::process::Command;

#[test]
fn sync_capture_keeps_raw_stdout_status_and_closed_input() {
    let directory = std::env::temp_dir().canonicalize().unwrap();
    let script = r"
import os, sys
assert sys.stdin.buffer.read() == b''
os.write(2, b'ignored')
def null(fd): return str(os.fstat(fd).st_rdev == os.stat(os.devnull).st_rdev)
fields = sys.argv[2:] + [os.environ['MAESTRO_CAPTURE'], os.getcwd(), null(0), null(2)]
os.write(1, '|'.join(fields).encode() + b'\x00\xff')
sys.exit(int(sys.argv[1]))
";
    for code in [0, 7] {
        let mut command = Command::new("python3");
        command
            .args([
                "-c",
                script,
                &code.to_string(),
                "literal $HOME;|",
                "",
                "literal $HOME;|",
            ])
            .env("MAESTRO_CAPTURE", "chosen")
            .current_dir(&directory);
        let output = spawn_sync(&mut command, Duration::from_secs(5)).unwrap();
        let mut expected = format!(
            "literal $HOME;|||literal $HOME;||chosen|{}|True|True",
            directory.display()
        )
        .into_bytes();
        expected.extend_from_slice(b"\0\xff");
        assert_eq!(output.stdout, expected);
        assert!(output.stderr.is_empty());
        assert_eq!(output.status.code(), Some(code));
    }
    let mut signal = Command::new("python3");
    signal.args([
        "-c",
        "import os, signal; os.kill(os.getpid(), signal.SIGTERM)",
    ]);
    let output = spawn_sync(&mut signal, Duration::from_secs(5)).unwrap();
    assert_eq!(output.status.signal(), Some(15));
    assert!(output.stdout.is_empty());
}

#[test]
fn sync_capture_is_callable_inside_a_running_runtime() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut command = Command::new("printf");
            command.arg("nested");
            assert_eq!(
                spawn_sync(&mut command, Duration::from_secs(5))
                    .unwrap()
                    .stdout,
                b"nested"
            );
        });
}

#[test]
fn sync_capture_distinguishes_spawn_failure_from_completed_failure() {
    use maestro_process::SpawnSyncError;
    use std::{error::Error, io};
    let missing = std::env::temp_dir().join(format!("maestro-missing-{}", std::process::id()));
    for (path, kind) in [
        (missing, io::ErrorKind::NotFound),
        (std::env::temp_dir(), io::ErrorKind::PermissionDenied),
    ] {
        let native = std::process::Command::new(&path).spawn().unwrap_err();
        let error = spawn_sync(&mut Command::new(&path), Duration::from_secs(5)).unwrap_err();
        assert_eq!(error.to_string(), "Cannot start the process");
        let source = error.source().unwrap().downcast_ref::<io::Error>().unwrap();
        assert_eq!(source.raw_os_error(), native.raw_os_error());
        assert!(matches!(error, SpawnSyncError::Spawn(ref e) if e.kind() == kind));
    }
    let mut command = Command::new("sh");
    command.args(["-c", "printf done; exit 7"]);
    let output = spawn_sync(&mut command, Duration::from_secs(5)).unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, b"done");
}

#[test]
fn sync_capture_timeout_covers_inherited_stdout() {
    use maestro_process::SpawnSyncError;
    use std::{
        error::Error,
        io::{Read, Write},
        net::TcpListener,
    };
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let script = r"
import os, socket, sys
os.write(1, b'prefix')
gone, alive = os.pipe()
if os.fork(): os._exit(0)
os.close(alive); os.read(gone, 1); os.close(gone)
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
if s.recv(1):
    try: os.write(1, b'tail'); result = b'writable'
    except BrokenPipeError: result = b'closed'
    s.sendall(result)
s.close(); os._exit(0)
";
    let mut command = Command::new("python3");
    command.args([
        "-c",
        script,
        &listener.local_addr().unwrap().port().to_string(),
    ]);
    let result = spawn_sync(&mut command, Duration::from_millis(200));
    // Release before assertions so an unexpected disposition cannot strand the holder.
    let (mut holder, _) = listener.accept().unwrap();
    holder.write_all(b"!").unwrap();
    let mut report = Vec::new();
    holder.read_to_end(&mut report).unwrap();
    let error = result.unwrap_err();
    assert!(matches!(error, SpawnSyncError::Timeout));
    assert!(error.source().is_none());
    assert_eq!(error.to_string(), "The process timed out");
    assert_eq!(report, b"closed");
}
