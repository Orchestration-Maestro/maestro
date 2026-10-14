//! Native tree cleanup observed through live descendant control channels.
#![cfg(test)]
#![cfg(unix)]
use maestro_process::child_process::{CapturedChild, WaitOptions, wait_for_child_process};
use maestro_tools::utils::shell::{
    kill_process_tree, kill_tracked_detached_children, track_detached_child_pid,
};
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
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let script = "import os,socket,sys\nchild=os.fork()\ns=socket.create_connection(('127.0.0.1',int(sys.argv[1])))\ns.sendall(b'READY')\ns.recv(1)\nif child: os.waitpid(child,0)";
        let mut command = tokio::process::Command::new("python3");
        command.args(["-c", script, &listener.local_addr().unwrap().port().to_string()]);
        let mut child = CapturedChild::spawn_detached(command).unwrap();
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
        let output = wait_for_child_process(&mut child, WaitOptions::default(), std::future::pending(), None).await.unwrap();
        assert!(!output.status.success());
        drop(guard);

        let mut direct = tokio::process::Command::new("python3");
        direct.args(["-c", "import socket,sys,time; s=socket.create_connection(('127.0.0.1',int(sys.argv[1]))); s.sendall(b'READY'); s.recv(1)", &listener.local_addr().unwrap().port().to_string()]);
        let mut child = CapturedChild::spawn(&mut direct).unwrap();
        let guard = Cleanup(child.id().unwrap());
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut greeting = [0; 5];
        socket.read_exact(&mut greeting).await.unwrap();
        assert_eq!(&greeting, b"READY");
        kill_process_tree(child.id().unwrap());
        let mut bytes = Vec::new();
        socket.read_to_end(&mut bytes).await.unwrap();
        assert!(bytes.is_empty());
        let output = wait_for_child_process(&mut child, WaitOptions::default(), std::future::pending(), None).await.unwrap();
        assert!(!output.status.success());
        drop(guard);
    });
}
