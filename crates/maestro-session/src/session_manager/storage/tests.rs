//! Production contention witnesses.
#![cfg(test)]
#[cfg(target_os = "linux")]
use super::*;
#[cfg(target_os = "linux")]
use sha2::{Digest, Sha256};
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

#[cfg(target_os = "linux")]
#[test]
fn failed_save_keeps_the_record_another_process_appends_meanwhile() {
    use super::super::ReadonlySessionManager as _;
    let test = "failed_save_keeps_the_record_another_process_appends_meanwhile";
    if appended_as_child() {
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let directory = root.path().join("sessions");
    let mut manager =
        super::super::SessionManager::create("cwd", Some(directory.to_str().unwrap()), "").unwrap();
    manager
        .append_message(serde_json::json!({"role":"assistant","content":"saved"}))
        .unwrap();
    let path = std::path::PathBuf::from(manager.get_session_file().unwrap());
    let mut expected = fs::read(&path).unwrap();
    let parked = root.path().join("parked");
    fs::rename(&path, &parked).unwrap();
    fs::create_dir(&path).unwrap();
    let missed = manager.append_message(serde_json::json!({"role":"user","content":"missed"}));
    assert!(missed.is_err());
    fs::remove_dir(&path).unwrap();
    fs::rename(&parked, &path).unwrap();
    let launch = appender(test, &path);
    let mut witness = None;
    let error = manager
        .publish(|_, path, bytes| {
            let mut file = fs::OpenOptions::new().append(true).open(path)?;
            file.write_all(&bytes[..7])?;
            witness = Some(launch());
            Err(io::Error::from_raw_os_error(5))
        })
        .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(5));
    let (child, waited) = witness.unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    expected.extend(b"after\n");
    assert_eq!(fs::read(&path).unwrap(), expected);
    assert!(
        waited,
        "the append ran while the failing save held the file"
    );
}

/// Launches a real append only after the production rewrite has compared its hash.
#[cfg(target_os = "linux")]
fn contention(alias: bool, test: &str) {
    if appended_as_child() {
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
    let launch = appender(test, target);
    let (sender, receiver) = std::sync::mpsc::channel();
    AFTER_COMPARE.set(Some(Box::new(move || sender.send(launch()).unwrap())));
    receiver
}

/// Appends `after` as the launched second process; false in the test's own process.
#[cfg(target_os = "linux")]
fn appended_as_child() -> bool {
    let Ok(path) = std::env::var("MAESTRO_COMPARED_APPEND") else {
        return false;
    };
    let mut stream =
        TcpStream::connect(std::env::var("MAESTRO_COMPARED_ADDRESS").unwrap()).unwrap();
    stream.write_all(&std::process::id().to_be_bytes()).unwrap();
    super::super::persistence::append(&path, b"after\n").unwrap();
    stream.write_all(b"done").unwrap();
    true
}

/// Prepares a second process that appends to `target`.
///
/// The returned launch starts it and reports whether its append had to wait.
#[cfg(target_os = "linux")]
fn appender(
    test: &str,
    target: &Path,
) -> impl FnOnce() -> (maestro_test_process::CheckedChild, bool) + 'static {
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
    move || {
        let child = child.spawn().unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        let mut pid = [0; 4];
        stream.read_exact(&mut pid).unwrap();
        let blocked = observe_waiter(u32::from_be_bytes(pid), &stream);
        (child, blocked)
    }
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
