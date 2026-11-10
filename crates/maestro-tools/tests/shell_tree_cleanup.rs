//! Native tree cleanup observed through live descendant control channels.
#![cfg(test)]
#![cfg(unix)]
use crate::read_support::{self, Scratch};
use crate::runtime::run;
use maestro_process::children::{CapturedChild, WaitOptions, wait_for_child_process};
use maestro_tools::utils::environment::{
    kill_process_tree, kill_tracked_detached_children, track_detached_child_pid,
};
use serde_json::json;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

/// Kills the fixture's session even when an assertion unwinds.
struct Cleanup(u32);
impl Drop for Cleanup {
    fn drop(&mut self) {
        kill_process_tree(self.0);
    }
}

#[test]
fn native_tree_kill_terminates_descendants() {
    // The registry drain below would otherwise reach shells tracked by concurrent tests.
    if !crate::read_support::isolated("native_tree_kill_terminates_descendants", "drain", &[]) {
        return;
    }
    run(async {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let root = Scratch::new().unwrap();
        let program = read_support::payload(
            root.path(),
            "tree",
            &json!({"scenario": "tree-root", "port": port}),
        );
        let mut child =
            CapturedChild::spawn_detached(tokio::process::Command::new(&program)).unwrap();
        let pid = child.id().unwrap();
        let guard = Cleanup(pid);
        let mut sockets = Vec::new();
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut greeting = [0; 5];
            socket.read_exact(&mut greeting).await.unwrap();
            assert_eq!(&greeting, b"READY");
            sockets.push(socket);
        }
        track_detached_child_pid(pid);
        kill_tracked_detached_children();
        for mut socket in sockets {
            let mut bytes = Vec::new();
            socket.read_to_end(&mut bytes).await.unwrap();
            assert!(bytes.is_empty());
        }
        assert_killed(&mut child).await;
        drop(guard);

        let direct = read_support::payload(
            root.path(),
            "leaf",
            &json!({"scenario": "tree-leaf", "port": port}),
        );
        let mut child = CapturedChild::spawn(&mut tokio::process::Command::new(direct)).unwrap();
        let guard = Cleanup(child.id().unwrap());
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut greeting = [0; 5];
        socket.read_exact(&mut greeting).await.unwrap();
        assert_eq!(&greeting, b"READY");
        kill_process_tree(child.id().unwrap());
        let mut bytes = Vec::new();
        socket.read_to_end(&mut bytes).await.unwrap();
        assert!(bytes.is_empty());
        assert_killed(&mut child).await;
        drop(guard);
    });
}

/// Wait for `child` and check that it ended by a signal or a failing status.
async fn assert_killed(child: &mut CapturedChild) {
    let output =
        wait_for_child_process(child, WaitOptions::default(), std::future::pending(), None)
            .await
            .unwrap();
    assert!(!output.status.success());
}
