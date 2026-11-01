//! A controlled child that stands in for Maestro in RPC mode: an installed executable that
//! runs the Python program a test passes in its `PEER_SCRIPT` variable.
use crate::child::Scratch;
use crate::native::install_executable;
use maestro_app::{RpcClient, RpcClientOptions};
use serde_json::Value;
use std::{path::Path, rc::Rc};
use tokio::sync::mpsc;

/// The executable: helpers for the test's program, then the program.
const STUB: &str = r"#!/usr/bin/env python3
import json, os, signal, sys, time
def emit(value):
    sys.stdout.write(json.dumps(value) + '\n'); sys.stdout.flush()
def commands():
    for raw in sys.stdin.buffer:
        yield raw.decode()
MISSING = object()
def respond(command, data=MISSING, error=None):
    reply = {'type': 'response', 'id': command['id'], 'command': command['type']}
    if error is None:
        reply['success'] = True
        if data is not MISSING: reply['data'] = data
    else:
        reply['success'] = False; reply['error'] = error
    emit(reply)
exec(os.environ['PEER_SCRIPT'])
";

/// Answers every command, after reporting its raw line; `PEER_ANSWERS` maps command types
/// to the data they answer with.
pub const SERVE: &str = r"
answers = json.loads(os.environ.get('PEER_ANSWERS', '{}'))
emit({'type': 'peer_ready'})
for line in commands():
    command = json.loads(line)
    emit({'type': 'wire_capture', 'raw': line})
    respond(command, answers.get(command['type'], MISSING))
";

/// The installed executable and the directory that holds it.
pub struct Peer {
    /// The directory, removed with the peer.
    pub scratch: Scratch,
    /// The executable's path.
    pub entry: String,
}

impl Peer {
    /// Installs the executable in a fresh directory.
    pub fn new() -> Self {
        let scratch = Scratch::new();
        let source = scratch.join("peer.py");
        std::fs::write(&source, STUB).unwrap();
        let entry = scratch.join("peer");
        install_executable(Path::new(&source), Path::new(&entry));
        Self { scratch, entry }
    }

    /// Options that launch the executable running `script`.
    pub fn options(&self, script: &str) -> RpcClientOptions {
        let mut options = RpcClientOptions {
            cli_path: Some(self.entry.clone()),
            ..RpcClientOptions::default()
        };
        options
            .env
            .insert("PEER_SCRIPT".to_owned(), script.to_owned());
        options
    }
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

/// Keeps a paused clock from advancing on its own, as a running blocking task does, until
/// the returned sender is used.
pub fn hold_clock() -> (std::sync::mpsc::Sender<()>, tokio::task::JoinHandle<()>) {
    let (release, released) = std::sync::mpsc::channel::<()>();
    let hold = tokio::task::spawn_blocking(move || {
        let _ = released.recv();
    });
    (release, hold)
}
