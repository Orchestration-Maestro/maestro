//! A collector error observation is consumed at the public descriptor boundary.
#![cfg(unix)]
use super::*;
use crate::child_process::wait::Observer;

/// Reads a producer acknowledgement without assuming its chunk boundaries.
async fn prefix(output: &RawOutput, length: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    while bytes.len() < length {
        bytes.extend(output.read().await.unwrap().unwrap());
    }
    bytes
}

#[test]
fn raw_read_failure_keeps_prefix_and_sibling_controls() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let mut command = crate::command("python3");
        command.args([
            "-c",
            "import os; os.write(1,b'prefix'); os.write(2,b'sibling'); os.read(0,1)",
        ]);
        let (child, work) = RawChild::spawn(
            command,
            [Stdio::piped(), Stdio::piped(), Stdio::piped()],
            false,
        )
        .unwrap();
        let worker = tokio::task::spawn_local(work);
        assert_eq!(prefix(child.stdout.as_ref().unwrap(), 6).await, b"prefix");
        let mut producer = Capture {
            stdout: child.stdout.as_ref().map(|output| Rc::downgrade(&output.0)),
            stderr: child.stderr.as_ref().map(|output| Rc::downgrade(&output.0)),
            completion: child.completion.clone(),
        };
        assert!(producer.failed(OutputStream::Stdout, &io::Error::other("pipe read failed")));
        assert_eq!(
            child
                .stdout
                .as_ref()
                .unwrap()
                .read()
                .await
                .unwrap_err()
                .to_string(),
            "pipe read failed"
        );
        assert_eq!(prefix(child.stderr.as_ref().unwrap(), 7).await, b"sibling");
        assert!(child.kill(None).unwrap());
        let status = child.wait().await.unwrap();
        assert_eq!(status.code(), None);
        assert_eq!(status.signal(), Some("SIGTERM"));
        worker.await.unwrap();
    });
}
