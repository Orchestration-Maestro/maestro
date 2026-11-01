//! Detached native children keep their original collector after actual guest replacement.
#![cfg(test)]
use super::faults::{answer, session};
use crate::support::Root;
use maestro_extensions_wasmtime::native_io::{
    ChildProcess, ExitStatus, IoFuture, NativeIo, Readable, SpawnOptions, Writable,
};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Observes only native controls, never a guest callback, context or Store.
struct ObservedIo {
    /// The unchanged application adapter schedules the real process collector.
    native: Rc<dyn NativeIo>,
    /// Native-only waiter observation retained independently of guest replacement.
    child: RefCell<Option<Rc<dyn ChildProcess>>>,
    /// Completion of old guest control release, without retaining those controls.
    released: RefCell<Option<tokio::sync::oneshot::Receiver<()>>>,
}
impl NativeIo for ObservedIo {
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: SpawnOptions,
    ) -> IoFuture<'_, Rc<dyn ChildProcess>> {
        Box::pin(async move {
            assert!(options.detached);
            let child = self.native.spawn(command, args, options).await?;
            *self.child.borrow_mut() = Some(Rc::clone(&child));
            let (released, receipt) = tokio::sync::oneshot::channel();
            *self.released.borrow_mut() = Some(receipt);
            Ok(Rc::new(GuestControls {
                child,
                released: Some(released),
            }) as Rc<dyn ChildProcess>)
        })
    }
    fn kill(&self, pid: i32, signal: Option<String>) -> Result<(), String> {
        self.native.kill(pid, signal)
    }
    fn fetch(
        &self,
        request: maestro_extensions_wasmtime::native_io::Request,
    ) -> IoFuture<'_, Rc<dyn maestro_extensions_wasmtime::native_io::Response>> {
        self.native.fetch(request)
    }
    fn watch(
        &self,
        path: String,
        listener: Rc<dyn Fn()>,
    ) -> Result<Rc<dyn maestro_extensions_wasmtime::native_io::FsWatcher>, String> {
        self.native.watch(path, listener)
    }
}

/// Delegates unchanged native controls while acknowledging their final guest release.
struct GuestControls {
    /// Original application child; the independent observer retains this, not this wrapper.
    child: Rc<dyn ChildProcess>,
    /// One teardown acknowledgement, with no guest state in its receiver.
    released: Option<tokio::sync::oneshot::Sender<()>>,
}
impl Drop for GuestControls {
    fn drop(&mut self) {
        let _ = self.released.take().unwrap().send(());
    }
}
impl ChildProcess for GuestControls {
    fn pid(&self) -> Option<u32> {
        self.child.pid()
    }
    fn stdin(&self) -> Option<Rc<dyn Writable>> {
        self.child.stdin()
    }
    fn stdout(&self) -> Option<Rc<dyn Readable>> {
        self.child.stdout()
    }
    fn stderr(&self) -> Option<Rc<dyn Readable>> {
        self.child.stderr()
    }
    fn wait(&self) -> IoFuture<'_, ExitStatus> {
        self.child.wait()
    }
    fn kill(&self, signal: Option<String>) -> Result<bool, String> {
        self.child.kill(signal)
    }
}

#[test]
fn detached_native_child_survives_reload_and_is_reaped() {
    if maestro_test_process::run_alone("detached_native_child_survives_reload_and_is_reaped")
        .unwrap()
    {
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = Root::new();
        let io = Rc::new(ObservedIo {
            native: maestro_app::extension_native_io::extension_native_io(&local),
            child: RefCell::default(),
            released: RefCell::default(),
        });
        let session = session(&root, &local, io.clone()).await;
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let producer = "import os,socket,sys; s=socket.create_connection(('127.0.0.1',int(sys.argv[1]))); s.sendall(os.getpid().to_bytes(8,'big')+os.getsid(0).to_bytes(8,'big')); nonce=s.makefile('rb').read(16); s.sendall(os.getpid().to_bytes(8,'big')+nonce); s.recv(1); sys.exit(23)";
        let reply = answer(&session,json!({"op":"spawn","command":"python3","args":["-c",producer,listener.local_addr().unwrap().port().to_string()],"detached":true,"stdio":["ignore","ignore","ignore"]})).await;
        let pid = reply["value"].as_u64().unwrap();
        let (mut peer, _) = listener.accept().await.unwrap();
        assert_eq!(peer.read_u64().await.unwrap(), pid);
        assert_eq!(peer.read_u64().await.unwrap(), pid, "detached native session");
        let command = session.extension_runner().get_command("native-child").unwrap();
        let retired = Rc::downgrade(&command.command.handler);
        drop(command);
        session.reload().await.unwrap();
        assert!(retired.upgrade().is_none(), "old guest command must be released");
        let released = io.released.borrow_mut().take().unwrap();
        released.await.unwrap();
        assert_eq!(answer(&session,json!({"op":"descriptors"})).await,json!({"error":"no retained child"}), "replacement guest is healthy and has fresh controls");
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos().to_be_bytes();
        peer.write_all(&nonce).await.unwrap();
        assert_eq!(peer.read_u64().await.unwrap(), pid);
        let mut echoed = [0; 16];
        peer.read_exact(&mut echoed).await.unwrap();
        assert_eq!(echoed, nonce, "same child answers a fresh post-reload challenge");
        let child = io.child.borrow_mut().take().unwrap();
        let mut wait = Box::pin(child.wait());
        assert!(std::future::Future::poll(wait.as_mut(), &mut std::task::Context::from_waker(std::task::Waker::noop())).is_pending());
        peer.write_all(&[1]).await.unwrap();
        let status = wait.await.unwrap();
        assert_eq!(status.code, Some(23));
        assert_eq!(status.signal, None);
        assert!(!child.kill(None).unwrap(), "original waiter observed native exit");
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists(), "wait completion includes native reaping");
        session.dispose();
    });
}
