//! Raw streaming through the captured-child lifecycle.
#![cfg(unix)]
#![cfg(test)]
use maestro_process::child_process::{
    CapturedChild, OutputMode, WaitOptions, wait_for_child_process,
};

#[test]
fn streamed_wait_delivers_chunks_without_retaining_them() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut command = tokio::process::Command::new("/bin/sh");
            command.args(["-c", "printf 'raw\\000bytes'; printf 'diagnostic' >&2"]);
            let mut child = CapturedChild::spawn(&mut command).unwrap();
            let mut chunks = Vec::new();
            let mut sink = |_, bytes: &[u8]| chunks.extend_from_slice(bytes);
            let output = wait_for_child_process(
                &mut child,
                WaitOptions::default(),
                std::future::pending(),
                Some(&mut sink),
            )
            .await
            .unwrap();
            assert!(output.status.success());
            assert!(output.stdout.is_empty());
            assert!(output.stderr.is_empty());
            assert!(chunks.windows(9).any(|bytes| bytes == b"raw\0bytes"));
            assert!(chunks.windows(10).any(|bytes| bytes == b"diagnostic"));
            assert_eq!(chunks.len(), 19);
        });
}

#[test]
fn session_spawn_leaves_ordinary_children_unchanged() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let parent_session = rustix::process::getsid(None)
                .unwrap()
                .as_raw_nonzero()
                .get();
            let parent_group = rustix::process::getpgrp().as_raw_nonzero().get();
            for detached in [false, true] {
                let mut command = tokio::process::Command::new("python3");
                command.args([
                    "-c",
                    "import os; print(os.getpid(),os.getsid(0),os.getpgrp())",
                ]);
                let mut child = if detached {
                    CapturedChild::spawn_detached(command)
                } else {
                    CapturedChild::spawn(&mut command)
                }
                .unwrap();
                let pid = i32::try_from(child.id().unwrap()).unwrap();
                let output = wait_for_child_process(
                    &mut child,
                    WaitOptions::default(),
                    std::future::pending(),
                    None,
                )
                .await
                .unwrap();
                let text = String::from_utf8(output.stdout).unwrap();
                let ids: Vec<i32> = text
                    .split_whitespace()
                    .map(|value| value.parse().unwrap())
                    .collect();
                assert_eq!(
                    ids,
                    if detached {
                        vec![pid, pid, pid]
                    } else {
                        vec![pid, parent_session, parent_group]
                    }
                );
                assert_eq!(child.id(), None);
            }
            assert_sync_session(parent_session, parent_group);
        });
}

#[test]
fn streamed_wait_continues_after_external_tree_kill() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut command = tokio::process::Command::new("python3");
            command.args([
                "-c",
                "import os,signal; os.write(1,b'before-kill'); signal.pause()",
            ]);
            let mut child = CapturedChild::spawn_detached(command).unwrap();
            let pid = child.id().unwrap();
            let (tx, rx) = tokio::sync::oneshot::channel();
            let mut tx = Some(tx);
            let mut bytes = Vec::new();
            let mut sink = |_, chunk: &[u8]| {
                bytes.extend_from_slice(chunk);
                tx.take().unwrap().send(()).unwrap();
            };
            let output = wait_for_child_process(
                &mut child,
                WaitOptions::default(),
                async {
                    rx.await.unwrap();
                    maestro_process::child_process::kill_process_group(pid).unwrap();
                    std::future::pending().await
                },
                Some(&mut sink),
            )
            .await
            .unwrap();
            assert_eq!(bytes, b"before-kill");
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert!(output.stderr.is_empty());
        });
}

#[test]
fn streamed_wait_delivers_stderr_even_when_capture_discards_it() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut command = tokio::process::Command::new("/bin/sh");
            command.args(["-c", "printf diagnostic >&2"]);
            let mut child = CapturedChild::spawn(&mut command).unwrap();
            let mut bytes = Vec::new();
            let mut sink = |_, chunk: &[u8]| bytes.extend_from_slice(chunk);
            let output = wait_for_child_process(
                &mut child,
                WaitOptions {
                    stderr: maestro_process::child_process::OutputMode::Discard,
                    ..WaitOptions::default()
                },
                std::future::pending(),
                Some(&mut sink),
            )
            .await
            .unwrap();
            assert_eq!(bytes, b"diagnostic");
            assert!(output.stderr.is_empty());
            assert!(child.take_output().1.is_empty());
        });
}

/// The synchronous launch preserves the parent session and process group.
fn assert_sync_session(parent_session: i32, parent_group: i32) {
    let mut command = tokio::process::Command::new("python3");
    command.args(["-c", "import os; print(os.getsid(0),os.getpgrp())"]);
    let output = maestro_process::spawn_sync(
        &mut command,
        maestro_process::SpawnSyncOptions::Capture {
            max_buffer_bytes: None,
            stdout: OutputMode::Capture,
            input: None,
            timeout: Some(std::time::Duration::from_secs(5)),
            stderr: OutputMode::Discard,
        },
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{parent_session} {parent_group}\n")
    );
}

#[test]
fn streamed_wait_tags_streams_without_reordering() {
    use maestro_process::child_process::OutputStream;
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let mut command=tokio::process::Command::new("python3");
        command.args(["-c",&format!("import socket,os\ns=socket.create_connection(('127.0.0.1',{port}))\nfor fd,data in [(1,b'a\\x00'),(2,b'b'),(1,b'c')]:\n os.write(fd,data)\n assert s.recv(1)==b'x'\n")]);
        let mut child=CapturedChild::spawn(&mut command).unwrap();
        let (mut socket,_)=listener.accept().unwrap();
        let mut transcript=Vec::new();
        let mut acknowledged=0;
        let mut sink=|stream,bytes:&[u8]| {
            transcript.extend(bytes.iter().map(|byte|(stream,*byte)));
            if [2,3,4].get(acknowledged).is_some_and(|boundary| transcript.len() >= *boundary) {
                socket.write_all(b"x").unwrap(); acknowledged+=1;
            }
        };
        let output=wait_for_child_process(&mut child,WaitOptions::default(),std::future::pending(),Some(&mut sink)).await.unwrap();
        assert!(output.status.success());assert!(output.stdout.is_empty() && output.stderr.is_empty());
        assert_eq!(transcript,[(OutputStream::Stdout,b'a'),(OutputStream::Stdout,0),(OutputStream::Stderr,b'b'),(OutputStream::Stdout,b'c')]);
        let mut tail=Vec::new();socket.read_to_end(&mut tail).unwrap();assert!(tail.is_empty());
        assert_eq!(child.id(),None);
    });
}
