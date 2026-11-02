//! Raw byte resources retain the captured-child owner's exit and EOF phases.
#![cfg(all(test, unix))]
use crate::child_programs::{self, async_command};
use maestro_process::child_process::{RawChild, RawOutput};
use maestro_test_process::HARNESS_START;
use maestro_test_process::child::harness;
use std::{
    process::Stdio,
    task::{Context, Waker},
};

/// Drives the process owner's collection work on a caller-owned local executor.
fn run(action: impl Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, action);
}
/// Starts a controlled producer with its three descriptors piped.
fn spawn(program: impl Fn()) -> (RawChild, impl Future<Output = ()> + 'static) {
    RawChild::spawn(
        async_command(program, &[]),
        [Stdio::piped(), Stdio::piped(), Stdio::piped()],
        false,
    )
    .unwrap()
}
/// Consumes one descriptor to its own EOF, without asserting chunk sizes.
async fn drain(output: &RawOutput) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Some(chunk) = output.read().await.unwrap() {
        bytes.extend(chunk);
    }
    bytes
}

#[test]
fn raw_child_output_close_is_independent_of_exit() {
    run(async {
        let (child, collect) = spawn(child_programs::closed_output_then_exit);
        let collector = tokio::task::spawn_local(collect);
        assert_eq!(
            drain(child.stdout.as_ref().unwrap()).await,
            harness(b"prefix\0\xff")
        );
        let mut waiting = std::pin::pin!(child.wait());
        assert!(
            waiting
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        child.stdin.as_ref().unwrap().write(&[1]).await.unwrap();
        assert_eq!(waiting.await.unwrap().code(), Some(7));
        assert_eq!(drain(child.stderr.as_ref().unwrap()).await, b"readylast");
        collector.await.unwrap();
        assert!(!child.kill(None).unwrap());
    });
}

#[test]
fn raw_child_pipes_survive_control_drop_and_reap_once() {
    run(async {
        let (child, collect) = spawn(child_programs::ready_then_echo);
        let pid = child.pid().unwrap();
        let input = child.stdin.as_ref().unwrap().clone();
        let output = child.stdout.as_ref().unwrap().clone();
        let collector = tokio::task::spawn_local(collect);
        let mut prefix = Vec::new();
        while prefix.len() < HARNESS_START.len() + 5 {
            prefix.extend(output.read().await.unwrap().unwrap());
        }
        assert_eq!(prefix, harness(b"ready"));
        drop(child);
        input.write(&[]).await.unwrap();
        input.write(&[0, 255, 1]).await.unwrap();
        input.end().await.unwrap();
        assert_eq!(drain(&output).await, [0, 255, 1]);
        collector.await.unwrap();
        assert!(!maestro_process::child_process::process_exists(pid));
    });
}
