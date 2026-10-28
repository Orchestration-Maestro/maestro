//! Cross-process appends against a held session publication lock.
#![cfg(target_os = "linux")]
use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
use maestro_test_process::TestProcess;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

#[test]
fn append_waits_for_one_replacement_and_preserves_its_entry() {
    if let Ok(path) = std::env::var("MAESTRO_SESSION_APPEND_PATH") {
        append_child(&path);
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let directory = root.path().to_str().unwrap();
    let mut manager = SessionManager::create("/work", Some(directory), "unused").unwrap();
    manager
        .append_message(serde_json::json!({"role":"assistant","content":"before"}))
        .unwrap();
    let path = manager.get_session_file().unwrap();
    let original = fs::read(path).unwrap();
    let sidecar = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(format!("{path}.lock"))
        .unwrap();
    let holder = maestro_lock::acquire_waiting(sidecar).unwrap();
    assert_eq!(fs::read(path).unwrap(), original);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let child = TestProcess::current("append_waits_for_one_replacement_and_preserves_its_entry")
        .unwrap()
        .nocapture()
        .env("MAESTRO_SESSION_APPEND_PATH", path)
        .env(
            "MAESTRO_SESSION_APPEND_ADDRESS",
            listener.local_addr().unwrap().to_string(),
        )
        .spawn()
        .unwrap();
    let (mut stream, _) = listener.accept().unwrap();
    let mut pid = [0; 4];
    stream.read_exact(&mut pid).unwrap();
    let blocked = observe_waiter(u32::from_be_bytes(pid), &stream);
    maestro_lock::replace(
        std::path::Path::new(path),
        &original,
        maestro_lock::Replace::default(),
    )
    .unwrap();
    drop(holder);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        blocked,
        "append completed before the held replacement finished"
    );
    let entries = maestro_session::session_manager::load_entries_from_file(path).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[2]["message"]["content"], "after");
}

/// Reports native lock blocking or a completed append, without a timer or scheduler guess.
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
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::yield_now();
            }
            result => {
                result.unwrap();
                return false;
            }
        }
    }
}

/// Starts an ordinary append after reporting the child's identity to the lock holder.
fn append_child(path: &str) {
    let mut manager = SessionManager::open(path, None, Some("/work")).unwrap();
    let address = std::env::var("MAESTRO_SESSION_APPEND_ADDRESS").unwrap();
    let mut stream = TcpStream::connect(address).unwrap();
    stream.write_all(&std::process::id().to_be_bytes()).unwrap();
    manager
        .append_message(serde_json::json!({"role":"user","content":"after"}))
        .unwrap();
    stream.write_all(b"done").unwrap();
}
