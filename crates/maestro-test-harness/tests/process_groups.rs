//! Long-running trees settle when the owning test stops, panics or is killed.
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};

use maestro_test_harness::{ProcessGroup, TestProcess, child};

/// End an isolated owner after both generations announce readiness.
fn assert_tree_stops(ending: &str) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let mut owner = TestProcess::child(tree_owner)
        .unwrap()
        .env(
            "MAESTRO_TEST_TREE_PORT",
            listener.local_addr().unwrap().port().to_string(),
        )
        .env("MAESTRO_TEST_TREE_END", ending)
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut peers = ready_peers(&listener);

    owner.stdin.take().unwrap().write_all(b"x").unwrap();
    // The guardian retains stdout until its group is stopped, even if the owner dies.
    let mut output = Vec::new();
    owner
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut output)
        .unwrap();
    owner.wait().unwrap();
    assert_closed_and_release(&mut peers, ending);
}

/// Accept both generations and retain their independent liveness connections.
fn ready_peers(listener: &TcpListener) -> [(TcpStream, u8); 2] {
    let peers = std::array::from_fn(|_| {
        let mut peer = listener.accept().unwrap().0;
        let mut role = [0];
        peer.read_exact(&mut role).unwrap();
        (peer, role[0])
    });
    let mut roles = peers.each_ref().map(|(_, role)| *role);
    roles.sort_unstable();
    assert_eq!(roles, [b'd', b'w']);
    peers
}

/// Observe Linux settlement immediately; other Unix targets await the signalled sockets' EOF.
fn assert_closed_and_release(peers: &mut [(TcpStream, u8); 2], ending: &str) {
    // Only Linux subreaping makes guardian EOF a full-descendant settlement signal.
    #[cfg(target_os = "linux")]
    let stopped = peers.each_mut().map(|(peer, _)| {
        peer.set_nonblocking(true).unwrap();
        match peer.read(&mut [0]) {
            Ok(0) => true,
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => true,
            _ => false,
        }
    });
    for (peer, _) in peers {
        peer.set_nonblocking(false).unwrap();
        // Release survivors before a Linux red assertion; other Unix targets await exit.
        #[cfg(target_os = "linux")]
        let _ = peer.shutdown(std::net::Shutdown::Write);
        let mut rest = Vec::new();
        if let Err(error) = peer.read_to_end(&mut rest) {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset,
                "{ending}"
            );
        }
    }
    #[cfg(target_os = "linux")]
    assert_eq!(
        stopped,
        [true, true],
        "{ending} left a live watch descendant"
    );
}

/// Prepare the exact ignored watcher through the same guardian callers use.
fn guarded_watch() -> Command {
    TestProcess::child(watch)
        .unwrap()
        .invoke_with(|arguments| {
            let mut command = ProcessGroup::command(std::env::current_exe()?)?;
            command.args(arguments);
            Ok::<_, std::io::Error>(command)
        })
        .unwrap()
}

#[test]
fn an_exited_watch_stops_its_descendants_before_the_guard_is_dropped() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let (mut output, writer) = std::os::unix::net::UnixStream::pair().unwrap();
    let stdout: std::os::fd::OwnedFd = writer.into();
    let mut command = guarded_watch();
    command
        .env(
            "MAESTRO_TEST_TREE_PORT",
            listener.local_addr().unwrap().port().to_string(),
        )
        .env("MAESTRO_TEST_TREE_LEADER_EXIT", "1")
        .stdout(stdout)
        .stderr(Stdio::null());
    let mut group = ProcessGroup::spawn(command).unwrap();
    let mut peers = ready_peers(&listener);
    for (peer, role) in &mut peers {
        if *role == b'w' {
            peer.write_all(b"x").unwrap();
        }
    }

    output.read_to_end(&mut Vec::new()).unwrap();
    assert_closed_and_release(&mut peers, "exited watch");
    group.stop().unwrap();
}

#[test]
fn killing_the_test_stops_its_watch_tree() {
    assert_tree_stops("kill");
}

#[test]
fn panicking_the_test_stops_its_watch_tree() {
    assert_tree_stops("panic");
}

#[test]
fn stopping_the_watch_stops_its_descendants() {
    assert_tree_stops("stop");
}

#[test]
#[ignore = "runs only inside a controlled child"]
fn tree_owner() {
    let mut group = ProcessGroup::spawn(guarded_watch()).unwrap();
    let mut byte = [0];
    std::io::stdin().read_exact(&mut byte).unwrap();
    match std::env::var("MAESTRO_TEST_TREE_END").unwrap().as_str() {
        "kill" => {
            rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
                .unwrap();
        }
        "panic" => panic!("controlled owner panic"),
        "stop" => group.stop().unwrap(),
        other => panic!("unknown ending {other}"),
    }
}

/// Retain a socket until released; its EOF proves the process stopped.
fn hold_connection(role: u8) {
    let port = std::env::var("MAESTRO_TEST_TREE_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let mut peer = child::connect(port).unwrap();
    peer.write_all(&[role]).unwrap();
    let _ = peer.read(&mut [0]);
}

#[test]
#[ignore = "runs only inside a controlled child"]
fn watch() {
    child::close_streams(&child::Stream::OUTPUTS).unwrap();
    let mut descendant = TestProcess::child(watch_descendant)
        .unwrap()
        .command()
        .unwrap()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    hold_connection(b'w');
    if std::env::var_os("MAESTRO_TEST_TREE_LEADER_EXIT").is_some() {
        // The guardian must adopt the descendant when its direct parent exits first.
        std::process::exit(0);
    }
    descendant.wait().unwrap();
}

#[test]
#[ignore = "runs only inside a controlled child"]
fn watch_descendant() {
    hold_connection(b'd');
}
