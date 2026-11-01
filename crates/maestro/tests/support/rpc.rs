//! The test's end of the shipped executable's standard-stream connection.
use super::Root;
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdout, Stdio},
};

/// A live root process with protocol output kept separate from startup diagnostics.
pub struct Rpc {
    /// Child process and its pipes.
    child: Child,
    /// Protocol line reader.
    output: BufReader<ChildStdout>,
    /// Correlation number of the next command.
    next: u64,
    /// Uncorrelated events observed while awaiting responses.
    pub events: Vec<Value>,
}
impl Rpc {
    /// The actual root process whose inherited descriptors a child receives.
    #[cfg(target_os = "linux")]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Start RPC over the production startup entry and concrete adapter.
    pub fn start(root: &Root) -> Self {
        Self::command(&mut root.command(&[
            "--mode",
            "rpc",
            "--provider",
            "startup",
            "--model",
            "controlled",
        ]))
    }
    /// Serve a supplied root invocation through its real process pipes.
    pub fn command(command: &mut std::process::Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            output,
            next: 0,
            events: Vec::new(),
        }
    }
    /// Send a command and retain all events before its correlated response.
    pub fn request(&mut self, mut command: Value) -> Value {
        self.next += 1;
        let id = self.next.to_string();
        command["id"] = id.clone().into();
        self.send(&command);
        loop {
            let record = self.read();
            if record["type"] == "response" && record["id"] == id {
                assert_eq!(record["success"], true, "{record}");
                return record["data"].clone();
            }
            self.events.push(record);
        }
    }
    /// Sends a supplied correlated command or extension UI reply without awaiting it.
    pub fn send(&mut self, record: &Value) {
        let input = self.child.stdin.as_mut().unwrap();
        let line = maestro_app::serialize_json_line(record).unwrap();
        input.write_all(line.as_bytes()).unwrap();
        input.flush().unwrap();
    }
    /// Read one actual framed record, failing on premature process exit.
    pub fn read(&mut self) -> Value {
        let mut line = String::new();
        if self.output.read_line(&mut line).unwrap() == 0 {
            let status = self.child.wait().unwrap();
            let mut errors = String::new();
            self.child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut errors)
                .unwrap();
            panic!("RPC ended before a record: {status}: {errors}");
        }
        serde_json::from_str(&line).unwrap()
    }
    /// Complete one prompt and observe the following session command as a persistence barrier.
    pub fn prompt(&mut self, text: &str) {
        let before = self.events.len();
        self.request(serde_json::json!({"type":"prompt", "message":text}));
        while !self.events[before..]
            .iter()
            .any(|event| event["type"] == "agent_end")
        {
            let record = self.read();
            self.events.push(record);
        }
        self.request(serde_json::json!({"type":"get_state"}));
    }
    /// Close input and require successful EOF settlement, with no diagnostic failures.
    pub fn finish(mut self) {
        drop(self.child.stdin.take());
        let mut remaining = String::new();
        self.output.read_to_string(&mut remaining).unwrap();
        let status = self.child.wait().unwrap();
        let mut errors = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut errors)
            .unwrap();
        assert!(status.success(), "{status}: {errors}");
        assert!(!errors.contains("Error:"), "{errors}");
        assert!(
            remaining
                .lines()
                .all(|line| serde_json::from_str::<Value>(line).is_ok())
        );
    }
}
impl Drop for Rpc {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            self.child.kill().unwrap();
            self.child.wait().unwrap();
        }
    }
}
