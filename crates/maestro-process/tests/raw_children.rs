//! Raw byte resources retain the captured-child owner's exit and EOF phases.
#![cfg(all(test, unix))]
use maestro_process::child_process::{RawChild, RawOutput};
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
/// Starts a literal Python producer with its three descriptors piped.
fn spawn(script: &str) -> (RawChild, impl Future<Output = ()> + 'static) {
    let mut command = maestro_process::command("python3");
    command.args(["-c", script]);
    RawChild::spawn(
        command,
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
        let (child, collect) = spawn(
            "import os; os.write(1,b'prefix\\x00\\xff'); os.close(1); os.write(2,b'ready'); os.read(0,1); os.write(2,b'last'); os._exit(7)",
        );
        let collector = tokio::task::spawn_local(collect);
        assert_eq!(drain(child.stdout.as_ref().unwrap()).await, b"prefix\0\xff");
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
        let (child, collect) = spawn(
            "import os,sys; os.write(1,b'ready'); data=sys.stdin.buffer.read(); os.write(1,data)",
        );
        let pid = child.pid().unwrap();
        let input = child.stdin.as_ref().unwrap().clone();
        let output = child.stdout.as_ref().unwrap().clone();
        let collector = tokio::task::spawn_local(collect);
        let mut prefix = Vec::new();
        while prefix.len() < 5 {
            prefix.extend(output.read().await.unwrap().unwrap());
        }
        assert_eq!(prefix, b"ready");
        drop(child);
        input.write(&[]).await.unwrap();
        input.write(&[0, 255, 1]).await.unwrap();
        input.end().await.unwrap();
        assert_eq!(drain(&output).await, [0, 255, 1]);
        collector.await.unwrap();
        assert!(!maestro_process::child_process::process_exists(pid));
    });
}
