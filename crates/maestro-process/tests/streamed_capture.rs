//! Raw streaming through the captured-child lifecycle.
#![cfg(unix)]
#![cfg(test)]
use crate::child_programs::{self, async_command, harness, without_harness};
use maestro_process::child_process::{
    CapturedChild, OutputMode, WaitOptions, wait_for_child_process,
};
use maestro_test_process::HARNESS_START;
use std::io::Write as _;

/// Collects the harness start bytes from the first chunks and releases the child once they
/// all arrived; reports whether `bytes` belonged to them.
fn within_start(starts: &mut Vec<u8>, bytes: &[u8], socket: &mut std::net::TcpStream) -> bool {
    if starts.len() >= HARNESS_START.len() {
        return false;
    }
    starts.extend_from_slice(bytes);
    if starts.len() == HARNESS_START.len() {
        socket.write_all(b"g").unwrap();
    }
    true
}

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
                let mut command = async_command(child_programs::session_ids, &[]);
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
                let text = String::from_utf8(without_harness(&output.stdout)).unwrap();
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
            let command = async_command(child_programs::prefix_then_wait, &[]);
            let mut child = CapturedChild::spawn_detached(command).unwrap();
            let pid = child.id().unwrap();
            let (tx, rx) = tokio::sync::oneshot::channel();
            let mut tx = Some(tx);
            let mut bytes = Vec::new();
            let mut sink = |_, chunk: &[u8]| {
                bytes.extend_from_slice(chunk);
                if bytes.ends_with(b"before-kill") {
                    tx.take().unwrap().send(()).unwrap();
                }
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
            assert_eq!(bytes, harness(b"before-kill"));
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
    let mut command = async_command(child_programs::sync_session_ids, &[]);
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
        String::from_utf8(without_harness(&output.stdout)).unwrap(),
        format!("{parent_session} {parent_group}\n")
    );
}

#[test]
fn streamed_wait_tags_streams_without_reordering() {
    use maestro_process::child_process::OutputStream;
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut command = async_command(
                child_programs::acknowledged_writer,
                &[("CHILD_PORT", &port.to_string())],
            );
            let mut child = CapturedChild::spawn(&mut command).unwrap();
            let (mut socket, _) = listener.accept().unwrap();
            let mut transcript = Vec::new();
            let mut acknowledged = 0;
            let mut starts = Vec::new();
            let mut sink = |stream, bytes: &[u8]| {
                if within_start(&mut starts, bytes, &mut socket) {
                    return;
                }
                transcript.extend(bytes.iter().map(|byte| (stream, *byte)));
                if [2, 3, 4]
                    .get(acknowledged)
                    .is_some_and(|boundary| transcript.len() >= *boundary)
                {
                    socket.write_all(b"g").unwrap();
                    acknowledged += 1;
                }
            };
            let output = wait_for_child_process(
                &mut child,
                WaitOptions::default(),
                std::future::pending(),
                Some(&mut sink),
            )
            .await
            .unwrap();
            assert!(output.status.success());
            assert!(output.stdout.is_empty() && output.stderr.is_empty());
            assert_eq!(starts, HARNESS_START);
            assert_eq!(
                transcript,
                [
                    (OutputStream::Stdout, b'a'),
                    (OutputStream::Stdout, 0),
                    (OutputStream::Stderr, b'b'),
                    (OutputStream::Stdout, b'c')
                ]
            );
            let mut tail = Vec::new();
            socket.read_to_end(&mut tail).unwrap();
            assert!(tail.is_empty());
            assert_eq!(child.id(), None);
        });
}

/// The fixture file.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    cases: Vec<Case>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    id: String,
    input: serde_json::Value,
    expected: Expected,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    seen: Vec<(String, String)>,
    code: i32,
}

/// The one recorded case of the local output sink fixture.
fn sink_case() -> Case {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/local-output-sink.json")).unwrap();
    assert_eq!(fixture.cases.len(), 1);
    let case = fixture.cases.into_iter().next().unwrap();
    assert_eq!(case.test, "local_process_sink_keeps_descriptor_delivery");
    assert_eq!(case.id, "native");
    assert_eq!(case.input, serde_json::json!({}));
    case
}

#[test]
fn local_process_sink_keeps_descriptor_delivery() {
    use maestro_process::child_process::{OutputStream, wait_for_child_process_with_sink};
    use std::{cell::RefCell, io::Write, rc::Rc};
    let case = &sink_case();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut command = async_command(
                child_programs::two_stream_writer,
                &[("CHILD_PORT", &port.to_string())],
            );
            let mut child = CapturedChild::spawn(&mut command).unwrap();
            let (mut socket, _) = listener.accept().unwrap();
            let seen = Rc::new(RefCell::new(Vec::new()));
            let mut starts = Vec::new();
            let mut sink = |stream, bytes: &[u8]| {
                if within_start(&mut starts, bytes, &mut socket) {
                    return;
                }
                seen.borrow_mut().push((
                    match stream {
                        OutputStream::Stdout => "stdout",
                        OutputStream::Stderr => "stderr",
                    }
                    .to_owned(),
                    String::from_utf8(bytes.to_vec()).unwrap(),
                ));
                if stream == OutputStream::Stdout {
                    assert_eq!(seen.borrow()[0], case.expected.seen[0]);
                    socket.write_all(b"g").unwrap();
                }
            };
            let output = wait_for_child_process_with_sink(
                &mut child,
                WaitOptions::default(),
                std::future::pending(),
                &mut sink,
            )
            .await
            .unwrap();
            assert_eq!(starts, HARNESS_START);
            assert_eq!(*seen.borrow(), case.expected.seen);
            assert_eq!(output.status.code(), Some(case.expected.code));
            assert!(output.stdout.is_empty() && output.stderr.is_empty());
            assert_eq!(child.take_output(), (vec![], vec![]));
            assert_eq!(child.id(), None);
        });
}

#[test]
fn default_waiter_keeps_send_and_none_inference() {
    fn send(_: &impl Send) {}
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for observed in [false, true] {
                let mut command = tokio::process::Command::new("/bin/sh");
                command.args(["-c", "printf output"]);
                let mut child = CapturedChild::spawn(&mut command).unwrap();
                let mut bytes = Vec::new();
                let mut sink = |_, data: &[u8]| bytes.extend_from_slice(data);
                let output = if observed {
                    let future = wait_for_child_process(
                        &mut child,
                        WaitOptions::default(),
                        std::future::pending(),
                        Some(&mut sink),
                    );
                    send(&future);
                    future.await.unwrap()
                } else {
                    let future = wait_for_child_process(
                        &mut child,
                        WaitOptions::default(),
                        std::future::pending(),
                        None,
                    );
                    send(&future);
                    future.await.unwrap()
                };
                assert!(output.status.success());
                assert!(output.stderr.is_empty());
                assert_eq!(
                    output.stdout,
                    if observed {
                        b"".to_vec()
                    } else {
                        b"output".to_vec()
                    }
                );
                assert_eq!(
                    bytes,
                    if observed {
                        b"output".to_vec()
                    } else {
                        b"".to_vec()
                    }
                );
            }
        });
}
