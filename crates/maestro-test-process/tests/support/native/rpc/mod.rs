//! Raw JSONL exchanges for application-client qualification.

use std::io::{self, BufRead};

use maestro_test_process::child::{read_input, write_stdout};
use maestro_test_process::rpc_peer::Scenario;
use serde_json::{Map, Value, json};

mod events;
#[cfg(unix)]
mod interactive;
#[cfg(unix)]
mod lifecycle;
mod sdk;
mod transport;
mod ui;

/// A peer request whose external inputs have been parsed before effects.
pub(super) enum Record {
    /// The interactive session state parsed as a JSON object.
    #[cfg(unix)]
    Session(Value),
    /// State, marker and release socket for an idle interrupt.
    #[cfg(unix)]
    IdleInterrupt(Value, String, u16),
    /// A lifecycle exchange without additional data.
    #[cfg(unix)]
    Lifecycle(lifecycle::Kind),
    /// An event exchange without additional data.
    Events(events::Kind),
    /// A transport exchange without additional data.
    Transport(transport::Kind),
    /// Raw commands answered with the supplied data, preserving absent fields.
    Serve(Map<String, Value>),
    /// Event records emitted after each reply.
    Burst(Vec<Value>),
    /// Full response records replayed with current request identifiers.
    Replay(Vec<Map<String, Value>>),
    /// Arbitrary JSON values emitted without a request.
    Records(Vec<Value>),
    /// A native launch observation with its marker path.
    #[cfg(unix)]
    Marked(lifecycle::Marked, String),
    /// A raw-stream scenario released over an observation socket.
    #[cfg(unix)]
    Controlled(lifecycle::Controlled, u16),
    /// The terminal client example's dialog interaction.
    Terminal(ui::Kind),
    /// The client example's protocol check.
    Client(sdk::Kind),
}

impl Record {
    /// Require a known scenario and parse its complete input before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let fields: Vec<_> = text.lines().collect();
        let ["rpc-peer", scenario] = fields.as_slice() else {
            return Err(io::Error::other("expected rpc-peer and a scenario"));
        };
        Self::from_scenario(scenario.parse()?)
    }

    /// Construct only the input states each module can execute.
    ///
    /// Both constructors name every scenario, so a new one fails to compile until it is placed.
    fn from_scenario(scenario: Scenario) -> io::Result<Self> {
        #[cfg(unix)]
        {
            if let Some(record) = native_scenario(scenario)? {
                return Ok(record);
            }
        }
        portable_scenario(scenario)
    }
}

/// Construct the peers that read native signals or sockets.
#[cfg(unix)]
fn native_scenario(scenario: Scenario) -> io::Result<Option<Record>> {
    use lifecycle::{Controlled, Kind as Lifecycle, Marked};

    Ok(Some(match scenario {
        Scenario::Session => Record::Session(session_state()?),
        Scenario::IdleInterrupt => {
            Record::IdleInterrupt(session_state()?, variable("PEER_MARKER")?, peer_port()?)
        }
        Scenario::Launch => Record::Marked(Marked::Launch, variable("PEER_MARKER")?),
        Scenario::Report => Record::Marked(Marked::Report, variable("PEER_MARKER")?),
        Scenario::Tick => Record::Controlled(Controlled::Tick, peer_port()?),
        Scenario::Fragments => Record::Controlled(Controlled::Fragments, peer_port()?),
        Scenario::Descendant => Record::Controlled(Controlled::Descendant, peer_port()?),
        Scenario::PipeHolder => Record::Controlled(Controlled::PipeHolder, peer_port()?),
        Scenario::TerminationMarker => Record::Terminal(ui::Kind::TerminationMarker),
        Scenario::Entry => Record::Lifecycle(Lifecycle::Entry),
        Scenario::BootFailure => Record::Lifecycle(Lifecycle::BootFailure),
        Scenario::Kill => Record::Lifecycle(Lifecycle::Kill),
        Scenario::Booting => Record::Lifecycle(Lifecycle::Booting),
        Scenario::ForwardStderr => Record::Lifecycle(Lifecycle::ForwardStderr),
        Scenario::DeliverStderr => Record::Lifecycle(Lifecycle::DeliverStderr),
        Scenario::SplitStderr => Record::Lifecycle(Lifecycle::SplitStderr),
        Scenario::Resist => Record::Lifecycle(Lifecycle::Resist),
        Scenario::Restart => Record::Lifecycle(Lifecycle::Restart),
        Scenario::ClosedInput => Record::Lifecycle(Lifecycle::ClosedInput),
        Scenario::DialogTour
        | Scenario::LocalDialogTour
        | Scenario::Notifications
        | Scenario::SelectionReplies
        | Scenario::DialogText
        | Scenario::MultilineCommand
        | Scenario::LiteralText
        | Scenario::PromptExit
        | Scenario::UnicodeState
        | Scenario::PromptAcceptance
        | Scenario::EndAfterAcceptance
        | Scenario::AcceptedEof
        | Scenario::ExitAfterReply
        | Scenario::RefusedState
        | Scenario::EarlyEof
        | Scenario::Serve
        | Scenario::Burst
        | Scenario::Replay
        | Scenario::Records
        | Scenario::Refuse
        | Scenario::Reversed
        | Scenario::Malformed
        | Scenario::LateState
        | Scenario::LateEnd
        | Scenario::LateAnswers
        | Scenario::LateExit
        | Scenario::Answered
        | Scenario::Collections
        | Scenario::Prompt
        | Scenario::RejectPrompt
        | Scenario::Cancelled
        | Scenario::Settle
        | Scenario::Linear
        | Scenario::Received
        | Scenario::Withhold => return Ok(None),
    }))
}

/// Construct the peers whose behavior needs no native signal or socket.
fn portable_scenario(scenario: Scenario) -> io::Result<Record> {
    use events::{Delayed, Kind as Event, Response};
    use transport::Kind as Transport;

    Ok(match scenario {
        Scenario::DialogTour => Record::Terminal(ui::Kind::DialogTour),
        Scenario::LocalDialogTour => Record::Terminal(ui::Kind::LocalDialogTour),
        Scenario::Notifications => Record::Terminal(ui::Kind::Notifications),
        Scenario::SelectionReplies => Record::Terminal(ui::Kind::SelectionReplies),
        Scenario::DialogText => Record::Terminal(ui::Kind::DialogText),
        Scenario::MultilineCommand => Record::Terminal(ui::Kind::MultilineCommand),
        Scenario::LiteralText => Record::Terminal(ui::Kind::LiteralText),
        Scenario::PromptExit => Record::Terminal(ui::Kind::PromptExit),
        Scenario::UnicodeState => Record::Client(sdk::Kind::UnicodeState),
        Scenario::PromptAcceptance => Record::Client(sdk::Kind::PromptAcceptance),
        Scenario::EndAfterAcceptance => Record::Client(sdk::Kind::EndAfterAcceptance),
        Scenario::AcceptedEof => Record::Client(sdk::Kind::AcceptedEof),
        Scenario::ExitAfterReply => Record::Client(sdk::Kind::ExitAfterReply),
        Scenario::RefusedState => Record::Client(sdk::Kind::RefusedState),
        Scenario::EarlyEof => Record::Client(sdk::Kind::EarlyEof),
        Scenario::Serve => Record::Serve(environment_json("PEER_ANSWERS", "{}")?),
        Scenario::Burst => Record::Burst(environment_json("PEER_RECORDS", "[]")?),
        Scenario::Replay => Record::Replay(environment_json("PEER_REPLIES", "[]")?),
        Scenario::Records => Record::Records(environment_json("PEER_RECORDS", "[]")?),
        Scenario::Refuse => Record::Transport(Transport::Refuse),
        Scenario::Reversed => Record::Transport(Transport::Reversed),
        Scenario::Malformed => Record::Transport(Transport::Malformed),
        Scenario::LateState => Record::Events(Event::Delayed(Delayed::State)),
        Scenario::LateEnd => Record::Events(Event::Delayed(Delayed::End)),
        Scenario::LateAnswers => Record::Events(Event::Delayed(Delayed::Answers)),
        Scenario::LateExit => Record::Events(Event::Delayed(Delayed::Exit)),
        Scenario::Answered => Record::Events(Event::Answered),
        Scenario::Collections => Record::Events(Event::Collections),
        Scenario::Prompt => Record::Events(Event::Responding(Response::Prompt)),
        Scenario::RejectPrompt => Record::Events(Event::Responding(Response::RejectPrompt)),
        Scenario::Cancelled => Record::Events(Event::Responding(Response::Cancelled)),
        Scenario::Settle => Record::Events(Event::Responding(Response::Settle)),
        Scenario::Linear => Record::Events(Event::Responding(Response::Linear)),
        Scenario::Received => Record::Events(Event::Received),
        Scenario::Withhold => Record::Events(Event::Withhold),
        Scenario::Session
        | Scenario::IdleInterrupt
        | Scenario::Launch
        | Scenario::Report
        | Scenario::Tick
        | Scenario::Fragments
        | Scenario::Descendant
        | Scenario::PipeHolder
        | Scenario::TerminationMarker
        | Scenario::Entry
        | Scenario::BootFailure
        | Scenario::Kill
        | Scenario::Booting
        | Scenario::ForwardStderr
        | Scenario::DeliverStderr
        | Scenario::SplitStderr
        | Scenario::Resist
        | Scenario::Restart
        | Scenario::ClosedInput => return Err(io::Error::other("scenario needs a Unix host")),
    })
}

/// Parse the interactive session's object state before starting its readers.
#[cfg(unix)]
fn session_state() -> io::Result<Value> {
    environment_json("PEER_STATE", "{}").map(Value::Object)
}

/// Require a nonzero release port before any connection or stream output.
#[cfg(unix)]
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
#[cfg(unix)]
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
        Record::Terminal(scenario) => ui::run(*scenario),
        Record::Client(scenario) => sdk::run(*scenario),
        #[cfg(unix)]
        Record::Session(state) => interactive::session(state),
        #[cfg(unix)]
        Record::IdleInterrupt(state, marker, port) => interactive::idle(state, marker, *port),
        Record::Serve(answers) => transport::serve(answers),
        Record::Burst(records) => events::burst(records),
        Record::Replay(replies) => transport::replay(replies),
        Record::Records(records) => transport::records(records),
        #[cfg(unix)]
        Record::Marked(scenario, marker) => lifecycle::marked(*scenario, marker),
        #[cfg(unix)]
        Record::Controlled(scenario, port) => lifecycle::controlled(*scenario, *port),
        #[cfg(unix)]
        Record::Lifecycle(scenario) => lifecycle::run(*scenario),
        Record::Transport(scenario) => transport::run(*scenario),
        Record::Events(scenario) => events::run(*scenario),
    }
}
