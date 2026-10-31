//! Production comparison-boundary contention witnesses.
#![cfg(test)]
#[cfg(target_os = "linux")]
use super::*;
use std::cell::RefCell;
#[cfg(target_os = "linux")]
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

thread_local! {
    static AFTER_COMPARE: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::default();
}

/// Runs the per-thread observer at the actual final comparison boundary.
pub(super) fn compared() {
    let callback = AFTER_COMPARE.take();
    if let Some(callback) = callback {
        callback();
    }
}

#[cfg(target_os = "linux")]
#[test]
fn append_after_final_comparison_waits_and_survives() {
    contention(false, "append_after_final_comparison_waits_and_survives");
}

#[cfg(target_os = "linux")]
#[test]
fn symlink_alias_rewrite_excludes_an_append_to_the_real_target() {
    contention(
        true,
        "symlink_alias_rewrite_excludes_an_append_to_the_real_target",
    );
}

/// Launches a real append only after the production rewrite has compared its hash.
#[cfg(target_os = "linux")]
fn contention(alias: bool, test: &str) {
    if let Ok(path) = std::env::var("MAESTRO_COMPARED_APPEND") {
        let mut stream =
            TcpStream::connect(std::env::var("MAESTRO_COMPARED_ADDRESS").unwrap()).unwrap();
        stream.write_all(&std::process::id().to_be_bytes()).unwrap();
        super::super::persistence::append(&path, b"after\n").unwrap();
        stream.write_all(b"done").unwrap();
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let target = root.path().join("target.jsonl");
    fs::write(&target, b"before\n").unwrap();
    let path = if alias {
        let link = root.path().join("alias.jsonl");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        link
    } else {
        target.clone()
    };
    let receiver = observe_comparison(test, &target);
    assert_eq!(
        rewrite_session_file(
            path.to_str().unwrap(),
            &Sha256::digest(b"before\n").into(),
            b"rewritten\n"
        )
        .unwrap(),
        None
    );
    let (child, blocked) = receiver.recv().unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        blocked,
        "append bypassed the production rewrite's exclusion"
    );
    assert_eq!(fs::read(&target).unwrap(), b"rewritten\nafter\n");
}

/// Installs a process witness that begins at the actual comparison checkpoint.
#[cfg(target_os = "linux")]
fn observe_comparison(
    test: &str,
    target: &Path,
) -> std::sync::mpsc::Receiver<(maestro_test_process::CheckedChild, bool)> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let child = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .env("MAESTRO_COMPARED_APPEND", target)
        .env(
            "MAESTRO_CODING_AGENT_DIR",
            target.parent().unwrap().join("other-config-root"),
        )
        .env(
            "MAESTRO_COMPARED_ADDRESS",
            listener.local_addr().unwrap().to_string(),
        );
    let (sender, receiver) = std::sync::mpsc::channel();
    AFTER_COMPARE.set(Some(Box::new(move || {
        let child = child.spawn().unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        let mut pid = [0; 4];
        stream.read_exact(&mut pid).unwrap();
        let blocked = observe_waiter(u32::from_be_bytes(pid), &stream);
        sender.send((child, blocked)).unwrap();
    })));
    receiver
}

/// Distinguishes native contention from a completed append without a timer.
#[cfg(target_os = "linux")]
fn observe_waiter(pid: u32, stream: &TcpStream) -> bool {
    stream.set_nonblocking(true).unwrap();
    loop {
        let locks = fs::read_to_string("/proc/locks").unwrap();
        if locks.lines().any(|line| {
            line.contains("->") && line.split_whitespace().any(|word| word == pid.to_string())
        }) {
            return true;
        }
        match stream.peek(&mut [0; 1]) {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => std::thread::yield_now(),
            result => {
                result.unwrap();
                return false;
            }
        }
    }
}
