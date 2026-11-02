//! Raw JSONL exchanges for application-client qualification.

use std::io::{self, BufRead};

use maestro_test_process::child::{read_input, write_stdout};
use maestro_test_process::rpc_peer::Scenario;
use serde_json::{Map, Value, json};

mod events;
mod interactive;
mod lifecycle;
mod transport;

/// A peer request whose external inputs have been parsed before effects.
pub(super) enum Record {
    /// The interactive session state parsed as a JSON object.
    Session(Value),
    /// State, marker and release socket for an idle interrupt.
    IdleInterrupt(Value, String, u16),
    /// A compiled scenario without additional data.
    Plain(Scenario),
    /// Raw commands answered with the supplied data, preserving absent fields.
    Serve(Map<String, Value>),
    /// Event records emitted after each reply.
    Burst(Vec<Value>),
    /// Full response records replayed with current request identifiers.
    Replay(Vec<Map<String, Value>>),
    /// Arbitrary JSON values emitted without a request.
    Records(Vec<Value>),
    /// A native launch observation with its marker path.
    Marked(Scenario, String),
    /// A raw-stream scenario released over an observation socket.
    Controlled(Scenario, u16),
}

impl Record {
    /// Require a known scenario and parse its complete input before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let fields: Vec<_> = text.lines().collect();
        let ["rpc-peer", scenario] = fields.as_slice() else {
            return Err(io::Error::other("expected rpc-peer and a scenario"));
        };
        match scenario.parse()? {
            Scenario::Session => Ok(Self::Session(Value::Object(environment_json(
                "PEER_STATE",
                "{}",
            )?))),
            Scenario::IdleInterrupt => Ok(Self::IdleInterrupt(
                Value::Object(environment_json("PEER_STATE", "{}")?),
                variable("PEER_MARKER")?,
                peer_port()?,
            )),
            Scenario::Serve => Ok(Self::Serve(environment_json("PEER_ANSWERS", "{}")?)),
            Scenario::Burst => Ok(Self::Burst(environment_json("PEER_RECORDS", "[]")?)),
            Scenario::Replay => Ok(Self::Replay(environment_json("PEER_REPLIES", "[]")?)),
            Scenario::Records => Ok(Self::Records(environment_json("PEER_RECORDS", "[]")?)),
            scenario @ (Scenario::Launch | Scenario::Report) => {
                Ok(Self::Marked(scenario, variable("PEER_MARKER")?))
            }
            scenario @ (Scenario::Tick
            | Scenario::Fragments
            | Scenario::Descendant
            | Scenario::PipeHolder) => {
                let port = peer_port()?;
                Ok(Self::Controlled(scenario, port))
            }
            scenario => Ok(Self::Plain(scenario)),
        }
    }
}

/// Require a nonzero release port before any connection or stream output.
fn peer_port() -> io::Result<u16> {
    let port = variable("PEER_PORT")?
        .parse::<u16>()
        .map_err(io::Error::other)?;
    if port == 0 {
        return Err(io::Error::other("peer port must not be zero"));
    }
    Ok(port)
}

/// Read one required text input from the isolated child's environment.
fn variable(name: &str) -> io::Result<String> {
    std::env::var(name).map_err(io::Error::other)
}

/// Parse supplied JSON into the shape required by the selected scenario.
fn environment_json<T: serde::de::DeserializeOwned>(name: &str, default: &str) -> io::Result<T> {
    let text = match std::env::var(name) {
        Ok(text) => text,
        Err(std::env::VarError::NotPresent) => default.to_owned(),
        Err(error) => return Err(io::Error::other(error)),
    };
    serde_json::from_str(&text).map_err(io::Error::other)
}

/// Write exactly one JSON value followed by LF, without harness framing.
fn emit(value: &Value) -> io::Result<()> {
    write_stdout(&serde_json::to_vec(value)?)?;
    write_stdout(b"\n")
}

/// Read complete commands while retaining each original newline.
struct Commands(std::io::StdinLock<'static>);

impl Commands {
    /// Lock the raw input stream for the duration of one peer scenario.
    fn new() -> Self {
        Self(std::io::stdin().lock())
    }

    /// Read a command together with its original wire representation.
    fn next(&mut self) -> io::Result<Option<(String, Value)>> {
        let mut raw = String::new();
        if self.0.read_line(&mut raw)? == 0 {
            return Ok(None);
        }
        let command = serde_json::from_str(&raw)?;
        Ok(Some((raw, command)))
    }

    /// Read the next required command, refusing premature EOF.
    fn required(&mut self) -> io::Result<Value> {
        self.next()?
            .map(|(_, command)| command)
            .ok_or_else(|| io::Error::other("expected another command"))
    }
}

/// A correlated successful response, with missing data distinct from JSON null.
fn respond(command: &Value, data: Option<&Value>) -> io::Result<()> {
    let mut reply = json!({"type": "response", "id": command["id"], "command": command["type"], "success": true});
    if let Some(data) = data {
        reply["data"] = data.clone();
    }
    emit(&reply)
}

/// A correlated refusal rather than a transport failure.
fn refuse(command: &Value, error: &str) -> io::Result<()> {
    emit(
        &json!({"type": "response", "id": command["id"], "command": command["type"], "success": false, "error": error}),
    )
}

/// A complete session state for delayed-answer scenarios.
fn state() -> Value {
    json!({"thinkingLevel": "off", "isStreaming": false, "isCompacting": false, "steeringMode": "all", "followUpMode": "all", "sessionId": "s", "autoCompactionEnabled": false, "messageCount": 0, "pendingMessageCount": 0})
}

/// Drain input until the client closes it, without pacing or polling.
fn drain() -> io::Result<()> {
    read_input().map(|_| ())
}

/// Execute the parsed behavior on the actual child's standard streams.
pub(super) fn run(record: &Record) -> io::Result<()> {
    match record {
        Record::Session(state) => interactive::session(state),
        Record::IdleInterrupt(state, marker, port) => interactive::idle(state, marker, *port),
        Record::Serve(answers) => transport::serve(answers),
        Record::Burst(records) => events::burst(records),
        Record::Replay(replies) => transport::replay(replies),
        Record::Records(records) => transport::records(records),
        Record::Marked(scenario, marker) => lifecycle::marked(*scenario, marker),
        Record::Controlled(scenario, port) => lifecycle::controlled(*scenario, *port),
        Record::Plain(scenario) => match scenario {
            Scenario::Entry
            | Scenario::BootFailure
            | Scenario::Kill
            | Scenario::Booting
            | Scenario::ForwardStderr
            | Scenario::DeliverStderr
            | Scenario::SplitStderr
            | Scenario::Resist
            | Scenario::Restart
            | Scenario::ClosedInput => lifecycle::run(*scenario),
            Scenario::Refuse | Scenario::Reversed | Scenario::Malformed => {
                transport::run(*scenario)
            }
            _ => events::run(*scenario),
        },
    }
}
