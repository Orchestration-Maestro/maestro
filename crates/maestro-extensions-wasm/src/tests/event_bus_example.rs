//! The shipped event-bus factory exercised through both hosts.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::example_support::{
    Command, invoke_command_on_component, released, released_component, start,
};
use super::host::{Harness, borrow, bus_emit};
use super::observed::Observed;
use crate::component_adapter::{Capabilities, Exports, Imports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/event_bus.rs"]
mod event_bus;

/// The same factory exported by the shipped component.
struct EventBus;

impl Extension for EventBus {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        event_bus::factory(api)
    }
}

/// Working directory of the session the recorded cases report through.
const FIRST: &str = "first";

/// Working directory a command context is bound to.
const COMMAND: &str = "/command";

/// The recorded registration and case observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// What the factory registers.
    metadata: Metadata,
    /// The recorded cases.
    rows: Vec<Row>,
}

/// The registrations the factory makes, in the order the host received them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    /// Every registration in order.
    registrations: Vec<Registration>,
    /// The session-start handler names.
    #[serde(rename = "eventHandlers")]
    event_handlers: Vec<String>,
    /// The registered commands.
    command: Vec<Command>,
}

/// One registration operation.
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
enum Registration {
    /// A session event handler.
    Event {
        /// Registered event name.
        name: String,
    },
    /// An event-bus listener.
    Listener {
        /// Subscribed channel.
        name: String,
    },
    /// A registered command.
    Command {
        /// Command name.
        name: String,
    },
}

/// One recorded case: what it does, what it supplies and what it must observe.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Case name in the recorded corpus.
    name: String,
    /// Operation the case performs.
    operation: Operation,
    /// Supplied argument, payload or session list.
    input: Value,
    /// The observations it must produce.
    expected: Expected,
}

/// The operation a recorded case performs.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Operation {
    /// Runs the emit command.
    Command,
    /// Delivers a peer payload through the host bus.
    External,
    /// Runs both startup callbacks for each supplied session.
    Lifecycle,
    /// Runs the emit command before any session started.
    BeforeStart,
}

/// The observations one case must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Emissions the host bus received, in order.
    emissions: Vec<Emission>,
    /// Notifications the listener reported, in order.
    notifications: Vec<Notification>,
}

/// One emission the host bus received.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Emission {
    /// Emitted channel.
    channel: String,
    /// Emitted payload.
    data: Value,
}

/// One notification, and the working directory of the context that received it.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Notification {
    /// Working directory of the context the notification reached.
    owner: String,
    /// Reported message.
    message: String,
    /// Reported severity.
    kind: Option<String>,
}

/// Where the transcript, the emissions and the UI acquisitions currently end.
#[derive(Clone, Copy)]
struct Mark {
    /// Lines observed so far.
    lines: usize,
    /// Emissions observed so far.
    emissions: usize,
    /// UI acquisitions observed so far.
    acquired: usize,
}

impl Mark {
    /// The start of what the host observed.
    const START: Self = Self {
        lines: 0,
        emissions: 0,
        acquired: 0,
    };

    /// The current end of what the host observed.
    fn of(observed: &Observed) -> Self {
        Self {
            lines: observed.transcript.len(),
            emissions: observed.bus.emissions.len(),
            acquired: observed.ui.acquired.len(),
        }
    }
}

/// One adapter driven through the recorded cases.
trait Host {
    /// Runs both startup callbacks with a context in `cwd`, in registration order.
    fn start_session(&mut self, cwd: &str) -> Result<(), String>;
    /// Runs the emit command with `args`.
    fn command(&mut self, args: &str) -> Result<(), String>;
    /// Emits `data` on `channel` through the host bus operation.
    fn send(&mut self, channel: &str, data: &Value) -> Result<(), String>;
    /// Where the transcript, the emissions and the UI acquisitions currently end.
    fn mark(&self) -> Mark;
    /// The emissions the host bus received since `mark`.
    fn emissions(&self, mark: Mark) -> Vec<Emission>;
    /// The notifications reported since `mark`.
    fn notifications(&self, mark: Mark) -> Vec<Notification>;
    /// What the host registered, as the factory made it.
    fn registrations(&self) -> Registered;
    /// Releases every registration and returns their names in order.
    fn release(&mut self) -> Result<Vec<String>, String>;
    /// Resources the host lent that the extension has not dropped.
    fn live(&self) -> usize;
}

/// The registrations the host received, as it recorded them.
struct Registered {
    /// How many session-start handlers were registered.
    starts: usize,
    /// Subscribed channel and listener identity, in order.
    listeners: Vec<(String, u32)>,
    /// Command name, description and whether completions were registered.
    commands: Vec<(String, Option<String>, bool)>,
}

/// The controlled adapter with the callbacks the shipped factory registered.
struct ControlledHost {
    /// The host that recorded what the factory asked for.
    host: Controlled,
    /// The adapter serving the factory's callbacks.
    exports: Exports<Controlled>,
    /// The two session-start callbacks in registration order.
    starts: [u32; 2],
    /// The emit command.
    command: u32,
}

impl ControlledHost {
    /// Starts the shipped factory and looks up the callbacks it registered.
    fn new() -> Result<Self, String> {
        let (host, exports) = super::block_on(start::<EventBus>())?;
        let starts = session_starts(&host.observed())?;
        let command = host.observed().callback("command emit")?;
        Ok(Self {
            host,
            exports,
            starts,
            command,
        })
    }
}

impl Host for ControlledHost {
    fn start_session(&mut self, cwd: &str) -> Result<(), String> {
        let event = session_start();
        for key in self.starts {
            let capabilities = Capabilities {
                ctx: self.host.context(cwd),
                signal: None,
            };
            super::block_on(self.exports.invoke_event(key, event.clone(), capabilities))?;
        }
        Ok(())
    }

    fn command(&mut self, args: &str) -> Result<(), String> {
        let context = self.host.command_context(COMMAND);
        super::block_on(
            self.exports
                .invoke_command(self.command, args.into(), context),
        )
    }

    fn send(&mut self, channel: &str, data: &Value) -> Result<(), String> {
        self.host.callback_emit(
            &super::bus_host::Emitter { key: 0 },
            channel,
            &data.to_string(),
        )
    }

    fn mark(&self) -> Mark {
        Mark::of(&self.host.observed())
    }

    fn emissions(&self, mark: Mark) -> Vec<Emission> {
        emissions_since(&self.host.observed(), mark)
    }

    fn notifications(&self, mark: Mark) -> Vec<Notification> {
        notifications_since(&self.host.observed(), mark)
    }

    fn registrations(&self) -> Registered {
        registered(&self.host.observed())
    }

    fn release(&mut self) -> Result<Vec<String>, String> {
        Ok(released(&self.host))
    }

    fn live(&self) -> usize {
        self.host.observed().live
    }
}

/// The component host and the callbacks the built factory registered.
struct ComponentHost {
    /// The instantiated component.
    harness: Harness,
    /// The two session-start callbacks in registration order.
    starts: [u32; 2],
    /// The emit command.
    command: u32,
}

impl ComponentHost {
    /// Instantiates the built example and looks up the callbacks it registered.
    fn new() -> Result<Self, String> {
        let path = example()?;
        let (engine, component) =
            super::build::compile(&path).map_err(|error| error.to_string())?;
        let mut harness = super::block_on(Harness::from_compiled(&engine, &component))
            .map_err(|error| error.to_string())?;
        let starts = session_starts(harness.observed())?;
        let command = harness
            .callback("command emit")
            .map_err(|error| error.to_string())?;
        Ok(Self {
            harness,
            starts,
            command,
        })
    }
}

impl Host for ComponentHost {
    fn start_session(&mut self, cwd: &str) -> Result<(), String> {
        let event = session_start();
        for key in self.starts {
            let capabilities = super::host_family::guest::Capabilities {
                ctx: self
                    .harness
                    .ordinary(cwd)
                    .map_err(|error| error.to_string())?,
                signal: None,
            };
            let exports = self.harness.exports.clone();
            let outcome = super::block_on(self.harness.store.run_concurrent(async |accessor| {
                exports
                    .call_invoke_event(accessor, borrow(key), event.clone(), capabilities)
                    .await
            }))
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
            outcome?;
        }
        Ok(())
    }

    fn command(&mut self, args: &str) -> Result<(), String> {
        invoke_command_on_component(&mut self.harness, self.command, args, false)
    }

    fn send(&mut self, channel: &str, data: &Value) -> Result<(), String> {
        bus_emit(&mut self.harness, channel, &data.to_string())
    }

    fn mark(&self) -> Mark {
        Mark::of(&self.harness.store.data().observed)
    }

    fn emissions(&self, mark: Mark) -> Vec<Emission> {
        emissions_since(&self.harness.store.data().observed, mark)
    }

    fn notifications(&self, mark: Mark) -> Vec<Notification> {
        notifications_since(&self.harness.store.data().observed, mark)
    }

    fn registrations(&self) -> Registered {
        registered(&self.harness.store.data().observed)
    }

    fn release(&mut self) -> Result<Vec<String>, String> {
        released_component(&mut self.harness)
    }

    fn live(&self) -> usize {
        self.harness.store.data().observed.live
    }
}

/// The built example artifact, built once per process.
fn example() -> Result<PathBuf, String> {
    static BUILT: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    BUILT
        .get_or_init(|| super::build::example_component("event_bus"))
        .clone()
}

/// The JSON document a session-start callback receives.
fn session_start() -> String {
    json!({"type":"session_start","reason":"new"}).to_string()
}

/// The two session-start callbacks a fresh factory registers, in registration order.
fn session_starts(observed: &Observed) -> Result<[u32; 2], String> {
    observed
        .registrations("event session_start")
        .try_into()
        .map_err(|keys: Vec<u32>| format!("registered {} session-start callbacks", keys.len()))
}

/// The emissions the host bus received since `mark`.
fn emissions_since(observed: &Observed, mark: Mark) -> Vec<Emission> {
    observed.bus.emissions[mark.emissions..]
        .iter()
        .map(|(channel, data)| Emission {
            channel: channel.clone(),
            data: data.clone(),
        })
        .collect()
}

/// The notifications reported since `mark`, each with the context that received it.
fn notifications_since(observed: &Observed, mark: Mark) -> Vec<Notification> {
    let owners = &observed.ui.acquired[mark.acquired..];
    let reached: Vec<(Option<String>, Option<String>)> =
        super::ui_host::receipts(&observed.transcript[mark.lines..])
            .into_iter()
            .filter(|receipt| receipt.method == "notify")
            .map(|receipt| {
                serde_json::from_value::<(Option<String>, Option<String>)>(receipt.operands)
                    .expect("a recorded notification")
            })
            .collect();
    assert_eq!(
        owners.len(),
        reached.len(),
        "one UI acquisition per notification"
    );
    owners
        .iter()
        .zip(reached)
        .map(|(owner, (message, kind))| Notification {
            owner: owner.clone(),
            message: message.expect("a reported message"),
            kind: kind.map(|kind| kind.to_lowercase()),
        })
        .collect()
}

/// The registrations the host received.
fn registered(observed: &Observed) -> Registered {
    Registered {
        starts: observed.registrations("event session_start").len(),
        listeners: observed.bus.listeners.clone(),
        commands: observed.registrations.commands.clone(),
    }
}

/// Asserts the factory emitted nothing while it loaded.
fn assert_no_load_emissions(host: &dyn Host) {
    assert!(
        host.emissions(Mark::START).is_empty(),
        "the factory emitted while it loaded"
    );
}

/// Assert the recorded registrations against what the host received.
fn assert_registrations(host: &dyn Host, metadata: &Metadata) {
    let seen = host.registrations();
    assert_eq!(
        seen.starts,
        metadata.event_handlers.len(),
        "session-start callbacks"
    );
    assert!(
        metadata
            .event_handlers
            .iter()
            .all(|name| name == "session_start"),
        "recorded handler names"
    );
    let channels: Vec<&String> = seen.listeners.iter().map(|(channel, _)| channel).collect();
    let recorded: Vec<&String> = metadata
        .registrations
        .iter()
        .filter_map(|registration| match registration {
            Registration::Listener { name } => Some(name),
            Registration::Event { .. } | Registration::Command { .. } => None,
        })
        .collect();
    assert_eq!(channels, recorded, "bus listeners");
    let commands: Vec<(&String, Option<&String>, bool)> = seen
        .commands
        .iter()
        .map(|(name, description, completions)| (name, description.as_ref(), *completions))
        .collect();
    let recorded: Vec<(&String, Option<&String>, bool)> = metadata
        .command
        .iter()
        .map(|command| {
            (
                &command.name,
                Some(&command.description),
                command.completions,
            )
        })
        .collect();
    assert_eq!(commands, recorded, "registered commands");
}

/// The registration names in the order the factory made them, with the listener identity.
fn registration_order(host: &dyn Host, metadata: &Metadata) -> Vec<String> {
    let listener = host
        .registrations()
        .listeners
        .first()
        .map(|(_, key)| *key)
        .expect("one bus listener");
    metadata
        .registrations
        .iter()
        .map(|registration| match registration {
            Registration::Event { name } => format!("event {name}"),
            Registration::Listener { .. } => format!("event bus {listener}"),
            Registration::Command { name } => format!("command {name}"),
        })
        .collect()
}

/// Runs one recorded case and asserts every observation it must produce.
fn run_row(host: &mut dyn Host, row: &Row) -> Result<(), String> {
    let mark = host.mark();
    match row.operation {
        Operation::Command => host.command(argument(row)?)?,
        Operation::BeforeStart => {
            host.command(argument(row)?)?;
            assert!(
                host.notifications(mark).is_empty(),
                "{}: reported before the session started",
                row.name
            );
            host.start_session(FIRST)?;
            host.command(argument(row)?)?;
        }
        Operation::External => {
            let channel = row.input["channel"]
                .as_str()
                .ok_or_else(|| format!("{}: a recorded channel", row.name))?;
            let data = &row.input["data"];
            if row.input["before"] == Value::Bool(true) {
                host.send(channel, data)?;
                assert!(
                    host.notifications(mark).is_empty(),
                    "{}: reported before the session started",
                    row.name
                );
                host.start_session(FIRST)?;
            }
            host.send(channel, data)?;
        }
        Operation::Lifecycle => {
            for cwd in row.input.as_array().ok_or("recorded sessions")? {
                host.start_session(cwd.as_str().ok_or("a recorded session")?)?;
            }
            host.send("my:notification", &json!({"message":"later","from":"peer"}))?;
        }
    }
    assert_eq!(
        host.emissions(mark),
        row.expected.emissions,
        "{}: emissions",
        row.name
    );
    assert_eq!(
        host.notifications(mark),
        row.expected.notifications,
        "{}: notifications",
        row.name
    );
    Ok(())
}

/// The command argument a recorded case supplies.
fn argument(row: &Row) -> Result<&str, String> {
    row.input
        .as_str()
        .ok_or_else(|| format!("{}: a recorded argument", row.name))
}

/// The recorded cases that perform `operation`.
fn cases(corpus: &Corpus, operation: Operation) -> impl Iterator<Item = &Row> {
    corpus
        .rows
        .iter()
        .filter(move |row| row.operation == operation)
}

/// Runs every supplied case on its own fresh host of `start`, reporting through `FIRST` where
/// the case expects a session, and releases what each one registered.
fn run_cases<'a>(
    corpus: &Corpus,
    rows: impl Iterator<Item = &'a Row>,
    mut open: impl FnMut() -> Result<Box<dyn Host>, String>,
) -> Result<(), String> {
    for row in rows {
        let mut host = open()?;
        assert_no_load_emissions(host.as_ref());
        assert_registrations(host.as_ref(), &corpus.metadata);
        if needs_session(row) {
            host.start_session(FIRST)?;
        }
        run_row(host.as_mut(), row)?;
        let order = registration_order(host.as_ref(), &corpus.metadata);
        assert_eq!(host.release()?, order, "{}: release order", row.name);
        assert_eq!(
            host.live(),
            0,
            "{}: the host lent nothing it kept",
            row.name
        );
    }
    Ok(())
}

/// Whether a recorded case expects a session to be running before it starts.
fn needs_session(row: &Row) -> bool {
    match row.operation {
        Operation::Command => true,
        Operation::External => row.input["before"] != Value::Bool(true),
        Operation::Lifecycle | Operation::BeforeStart => false,
    }
}

/// The recorded corpus.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("event_bus_expected.json")).expect("recorded corpus")
}

/// Exactly two session-start callbacks, one listener and one command, in registration order.
#[test]
fn event_bus_registers_channels_and_command() -> Result<(), String> {
    let mut host = ControlledHost::new()?;
    let corpus = corpus();
    assert_no_load_emissions(&host);
    assert_registrations(&host, &corpus.metadata);
    let order = registration_order(&host, &corpus.metadata);
    assert_eq!(host.release()?, order, "release order");
    assert_eq!(host.live(), 0, "the host lent nothing it kept");
    Ok(())
}

/// Every recorded command argument reaches the fixed channel and reports its fallback.
#[test]
fn event_bus_command_preserves_message_and_default() -> Result<(), String> {
    let corpus = corpus();
    run_cases(&corpus, cases(&corpus, Operation::Command), || {
        Ok(Box::new(ControlledHost::new()?))
    })
}

/// Neither a peer payload nor the command reports anything before a session started.
#[test]
fn event_bus_listener_waits_for_session_context() -> Result<(), String> {
    let corpus = corpus();
    run_cases(
        &corpus,
        corpus.rows.iter().filter(|row| {
            row.operation == Operation::External && row.input["before"] == Value::Bool(true)
                || row.operation == Operation::BeforeStart
        }),
        || Ok(Box::new(ControlledHost::new()?)),
    )
}

/// A second session replaces the context the retained listener reports through.
#[test]
fn event_bus_session_start_replaces_current_context() -> Result<(), String> {
    let corpus = corpus();
    run_cases(&corpus, cases(&corpus, Operation::Lifecycle), || {
        Ok(Box::new(ControlledHost::new()?))
    })
}

/// Three peer payloads keep their literal fields, and another channel reports nothing.
#[test]
fn event_bus_foreign_delivery_keeps_literal_fields() -> Result<(), String> {
    let corpus = corpus();
    run_cases(
        &corpus,
        corpus
            .rows
            .iter()
            .filter(|row| row.operation == Operation::External && !needs_session_before(row)),
        || Ok(Box::new(ControlledHost::new()?)),
    )
}

/// Whether a recorded external case runs before any session started.
fn needs_session_before(row: &Row) -> bool {
    row.input["before"] == Value::Bool(true)
}

/// The built single-copy example reports every recorded case through its own callbacks.
#[test]
fn event_bus_component_uses_registered_callbacks() -> Result<(), String> {
    let corpus = corpus();
    run_cases(&corpus, corpus.rows.iter(), || {
        Ok(Box::new(ComponentHost::new()?))
    })
}
