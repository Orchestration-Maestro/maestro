//! Raw streaming through the captured-child lifecycle.
#![cfg(unix)]
#![cfg(test)]
use maestro_process::child_process::{CapturedChild, WaitOptions, wait_for_child_process};

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
            let mut sink = |bytes: &[u8]| chunks.extend_from_slice(bytes);
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
                    CapturedChild::spawn_detached(&mut command)
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
            let mut command = tokio::process::Command::new("python3");
            command.args(["-c", "import os; print(os.getsid(0),os.getpgrp())"]);
            let output =
                maestro_process::spawn_sync(&mut command, std::time::Duration::from_secs(5))
                    .unwrap();
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                format!("{parent_session} {parent_group}\n")
            );
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
            let mut child = CapturedChild::spawn_detached(&mut command).unwrap();
            let pid = child.id().unwrap();
            let (tx, rx) = tokio::sync::oneshot::channel();
            let mut tx = Some(tx);
            let mut bytes = Vec::new();
            let mut sink = |chunk: &[u8]| {
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
