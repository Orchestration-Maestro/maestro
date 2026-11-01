//! Direct Windows child termination leaves independently running descendants alive.
#![cfg(all(test, windows))]
use maestro_process::child_process::{RawChild, signal_pid};
use maestro_test_process::TestProcess;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Stdio,
};

/// The single checked helper body both parent and grandchild invocations select.
const HELPER: &str = "windows_children::windows_direct_kill_helper";
/// The helper's explicitly supplied role.
const ROLE: &str = "MAESTRO_CHILD_KILL_HELPER";
/// The private socket shared only with this helper tree.
const PORT: &str = "MAESTRO_CHILD_KILL_PORT";

/// Announces this exact native process and its selected helper role.
fn announce(role: u8) -> TcpStream {
    let port = std::env::var(PORT).unwrap().parse::<u16>().unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(&[role]).unwrap();
    stream
        .write_all(&u64::from(std::process::id()).to_be_bytes())
        .unwrap();
    stream
}

#[test]
fn windows_direct_kill_helper() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    if role == "parent" {
        let _grandchild = TestProcess::named(HELPER)
            .unwrap()
            .nocapture()
            .env(ROLE, "grandchild")
            .env(PORT, std::env::var(PORT).unwrap())
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
    } else {
        assert_eq!(role, "grandchild");
    }
    let mut stream = announce(if role == "parent" { b'P' } else { b'G' });
    let mut command = [0];
    while stream.read_exact(&mut command).is_ok() {
        if command[0] == 2 {
            break;
        }
        assert_eq!(command[0], 1);
        stream.write_all(b"A").unwrap();
    }
    // Exit skips destructors; close the control socket first.
    drop(stream);
    std::process::exit(0);
}

/// A control socket that releases the grandchild even if an assertion fails.
struct Grandchild(TcpStream);
impl Drop for Grandchild {
    fn drop(&mut self) {
        let _ = self.0.write_all(&[2]);
        let _ = self.0.shutdown(std::net::Shutdown::Write);
    }
}

/// Reads the two coupled process acknowledgements without assuming their arrival order.
fn children(listener: &TcpListener, pid: u32) -> (TcpStream, Grandchild) {
    let mut parent = None;
    let mut grandchild = None;
    for _ in 0..2 {
        let (mut socket, _) = listener.accept().unwrap();
        let mut role = [0];
        let mut child = [0; 8];
        socket.read_exact(&mut role).unwrap();
        socket.read_exact(&mut child).unwrap();
        let announced = u64::from_be_bytes(child);
        if role[0] == b'P' {
            assert_eq!(announced, u64::from(pid));
            parent = Some(socket);
        } else {
            assert_eq!(role[0], b'G');
            assert_ne!(announced, u64::from(pid));
            grandchild = Some(Grandchild(socket));
        }
    }
    (parent.unwrap(), grandchild.unwrap())
}

/// Starts the existing raw owner, proving the selected helper's one-test prologue.
async fn launch(port: u16) -> (RawChild, tokio::task::JoinHandle<()>) {
    let (child, collect) = TestProcess::named(HELPER)
        .unwrap()
        .nocapture()
        .invoke_with(|args| {
            let mut command = maestro_process::command(std::env::current_exe()?);
            command
                .args(args)
                .env(ROLE, "parent")
                .env(PORT, port.to_string());
            RawChild::spawn(
                command,
                [Stdio::piped(), Stdio::piped(), Stdio::piped()],
                false,
            )
        })
        .unwrap();
    let collector = tokio::task::spawn_local(collect);
    let mut prologue = Vec::new();
    while !prologue
        .windows(b"running 1 test\n".len())
        .any(|bytes| bytes == b"running 1 test\n")
    {
        prologue.extend(
            child
                .stdout
                .as_ref()
                .unwrap()
                .read()
                .await
                .unwrap()
                .unwrap(),
        );
    }
    (child, collector)
}

#[test]
fn windows_child_kill_leaves_grandchild_running() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for signal in [None, Some("SIGTERM"), Some("SIGKILL"), Some("SIGINT")] {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let (child, collector) = launch(listener.local_addr().unwrap().port()).await;
            let (parent, mut grandchild) = children(&listener, child.pid().unwrap());
            assert!(signal_pid(-i32::try_from(child.pid().unwrap()).unwrap(), signal).is_err());
            assert!(child.kill(signal).unwrap());
            grandchild.0.write_all(&[1]).unwrap();
            let mut alive = [0];
            grandchild.0.read_exact(&mut alive).unwrap();
            assert_eq!(&alive, b"A");
            grandchild.0.write_all(&[2]).unwrap();
            assert_eq!(grandchild.0.read(&mut [0]).unwrap(), 0);
            let status = child.wait().await.unwrap();
            assert!(!status.success());
            assert_eq!(status.code(), None);
            assert_eq!(status.signal(), Some(signal.unwrap_or("SIGTERM")));
            assert!(!child.kill(None).unwrap());
            drop(parent);
            collector.await.unwrap();
        }
    });
}

#[test]
fn windows_invalid_signal_leaves_the_child_running() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let (child, collector) = launch(listener.local_addr().unwrap().port()).await;
        let (mut parent, mut grandchild) = children(&listener, child.pid().unwrap());
        for name in ["invalid-signal", "sigterm", "", "SIGHUP"] {
            for result in [
                signal_pid(i32::try_from(child.pid().unwrap()).unwrap(), Some(name)),
                child.kill(Some(name)).map(|_| ()),
            ] {
                let error = result.unwrap_err();
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
                assert!(error.to_string().contains(name));
                parent.write_all(&[1]).unwrap();
                let mut alive = [0];
                parent.read_exact(&mut alive).unwrap();
                assert_eq!(&alive, b"A");
            }
        }
        assert!(child.kill(Some("SIGQUIT")).unwrap());
        grandchild.0.write_all(&[2]).unwrap();
        assert_eq!(grandchild.0.read(&mut [0]).unwrap(), 0);
        assert_eq!(child.wait().await.unwrap().signal(), Some("SIGQUIT"));
        drop(parent);
        collector.await.unwrap();
    });
}
