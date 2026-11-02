//! A prepared native child serving finite application RPC scenarios.

use crate::child::Scratch;
use maestro_app::{RpcClient, RpcClientOptions};
pub use maestro_test_process::rpc_peer::Scenario;
use serde_json::Value;
use std::{
    io::Write,
    net::{TcpListener, TcpStream},
    path::Path,
    rc::Rc,
};
use tokio::sync::mpsc;

/// The installed executable, release socket and their disposable directory.
pub struct Peer {
    /// The directory, removed with the peer.
    pub scratch: Scratch,
    /// The executable's path.
    pub entry: String,
    /// Out-of-band release for raw fragments, ticks or a pipe-holding descendant.
    control: TcpListener,
}

impl Peer {
    /// Install the prepared executable in a fresh directory, without discovering build outputs.
    pub fn new() -> Self {
        let scratch = Scratch::new();
        let entry = scratch.join("peer");
        let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
            .expect("run just prepare-tests, then cargo nextest");
        maestro_test_process::install_executable(Path::new(&artifact), Path::new(&entry)).unwrap();
        Self {
            scratch,
            entry,
            control: TcpListener::bind("127.0.0.1:0").unwrap(),
        }
    }

    /// Options that launch one compiled scenario on the actual client transport.
    pub fn options(&self, scenario: Scenario) -> RpcClientOptions {
        if scenario == Scenario::Descendant {
            let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE").unwrap();
            let holder = self.scratch.join("holder");
            maestro_test_process::install_executable(Path::new(&artifact), Path::new(&holder))
                .unwrap();
            std::fs::write(format!("{holder}.fixture"), "rpc-peer\nPipeHolder\n").unwrap();
        }
        std::fs::write(
            format!("{}.fixture", self.entry),
            format!("rpc-peer\n{}\n", scenario.name()),
        )
        .unwrap();
        let mut options = RpcClientOptions {
            cli_path: Some(self.entry.clone()),
            ..RpcClientOptions::default()
        };
        options.env.insert(
            "PEER_PORT".into(),
            self.control.local_addr().unwrap().port().to_string(),
        );
        options
            .env
            .insert("PEER_HOLDER".into(), self.scratch.join("holder"));
        options
    }

    /// Accept a native peer's release connection, without a timed receive or polling.
    pub fn control(&self) -> TcpStream {
        self.control.accept().unwrap().0
    }
}

/// Release exactly one native operation through its existing observation socket.
pub fn release(control: &mut TcpStream) {
    control.write_all(b"r").unwrap();
}

/// Every event the client delivers from now on, in order.
pub fn events(client: &RpcClient) -> mpsc::UnboundedReceiver<Value> {
    let (sender, receiver) = mpsc::unbounded_channel();
    // The registration stays for the client's lifetime.
    let _remove = client.on_event(Rc::new(move |event: &Value| {
        let _ = sender.send(event.clone());
        Ok(())
    }));
    receiver
}

/// The next delivered event whose `type` is `kind`.
pub async fn next_of(events: &mut mpsc::UnboundedReceiver<Value>, kind: &str) -> Value {
    loop {
        let event = events.recv().await.unwrap();
        if event["type"] == kind {
            return event;
        }
    }
}

/// Keep a paused clock from advancing on its own until the returned sender is used.
pub fn hold_clock() -> (std::sync::mpsc::Sender<()>, tokio::task::JoinHandle<()>) {
    let (release, released) = std::sync::mpsc::channel::<()>();
    let hold = tokio::task::spawn_blocking(move || {
        let _ = released.recv();
    });
    (release, hold)
}
