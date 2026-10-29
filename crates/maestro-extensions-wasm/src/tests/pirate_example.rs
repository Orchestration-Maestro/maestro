//! The shipped prompt-mode toggle example exercised through both hosts.
//!
//! Notification severities are the host's own spellings, so the source's `info` is `Info`.
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::controlled::Controlled;
use super::documents::Answer;
use super::example_support::{
    Command, assert_registered_command, invoke_command_on_component, release_all, start,
};
use super::host::Harness;
use super::observed::Observed;
use super::ui_host::receipts;
use crate::component_adapter::{Capabilities, Exports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/pirate.rs"]
mod pirate;

/// The same factory exported by the shipped component.
struct Pirate;

impl Extension for Pirate {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        pirate::pirate_extension(api)
    }
}

/// A host the recorded operations run against.
trait Host {
    /// Runs the registered command with `input` and returns what the host recorded.
    fn command(&mut self, input: &CommandInput) -> (Vec<Notification>, Result<(), String>);

    /// Delivers `event` to the registered handler and returns its answer.
    fn event(&mut self, event: &Value) -> Result<Answer, String>;
}

/// A controlled host with the two callbacks one factory instance registered.
struct ControlledHost {
    /// The host that recorded what the factory asked for.
    host: Controlled,
    /// The adapter serving the factory's callbacks.
    exports: Exports<Controlled>,
    /// The registered command.
    command: u32,
    /// The registered prompt handler.
    event: u32,
}

impl ControlledHost {
    /// Starts one factory instance on a fresh controlled host.
    fn new() -> Result<Self, String> {
        let (host, exports) = super::block_on(start::<Pirate>())?;
        let command = host.observed().callback("command pirate")?;
        let event = host.observed().callback("event before_agent_start")?;
        Ok(Self {
            host,
            exports,
            command,
            event,
        })
    }

    /// The observations made since `mark`.
    fn since(&self, mark: usize) -> Vec<String> {
        self.host.observed().transcript[mark..].to_vec()
    }
}

impl Host for ControlledHost {
    fn command(&mut self, input: &CommandInput) -> (Vec<Notification>, Result<(), String>) {
        let mark = self.host.observed().transcript.len();
        set_rejection(&mut self.host.observe(), input.rejection.as_deref());
        let context = self.host.command_context("/controlled");
        let run = self
            .exports
            .invoke_command(self.command, input.args.clone(), context);
        let outcome = super::block_on(run);
        (notifications(&self.since(mark)), outcome)
    }

    fn event(&mut self, event: &Value) -> Result<Answer, String> {
        let capabilities = Capabilities {
            ctx: self.host.context("/controlled"),
            signal: None,
        };
        let run = self
            .exports
            .invoke_event(self.event, event.to_string(), capabilities);
        let outcome = super::block_on(run)?;
        Answer::of(super::guest_family::delivery(outcome))
    }
}

/// A built component instance with the two callbacks one factory instance registered.
struct ComponentHost {
    /// The instantiated component.
    harness: Harness,
    /// The registered command.
    command: u32,
    /// The registered prompt handler.
    event: u32,
}

impl ComponentHost {
    /// Instantiates the built example and looks up both registrations.
    fn new(path: &Path) -> Result<Self, String> {
        let harness = super::block_on(Harness::start(path)).map_err(|error| error.to_string())?;
        let command = harness
            .callback("command pirate")
            .map_err(|error| error.to_string())?;
        let event = harness
            .callback("event before_agent_start")
            .map_err(|error| error.to_string())?;
        Ok(Self {
            harness,
            command,
            event,
        })
    }

    /// The observations made since `mark`.
    fn since(&self, mark: usize) -> Vec<String> {
        self.harness.store.data().observed.transcript[mark..].to_vec()
    }
}

impl Host for ComponentHost {
    fn command(&mut self, input: &CommandInput) -> (Vec<Notification>, Result<(), String>) {
        let mark = self.harness.store.data().observed.transcript.len();
        set_rejection(self.harness.observed(), input.rejection.as_deref());
        let run = invoke_command_on_component(&mut self.harness, self.command, &input.args, false);
        (notifications(&self.since(mark)), run)
    }

    fn event(&mut self, event: &Value) -> Result<Answer, String> {
        let key = self.event;
        let capabilities = super::host_family::guest::Capabilities {
            ctx: self
                .harness
                .ordinary("/controlled")
                .map_err(|error| error.to_string())?,
            signal: None,
        };
        let exports = self.harness.exports.clone();
        let document = event.to_string();
        let run = super::block_on(self.harness.store.run_concurrent(async |accessor| {
            exports
                .call_invoke_event(accessor, super::host::borrow(key), document, capabilities)
                .await
        }))
        .map_err(|error| error.to_string())?;
        let called = run.map_err(|error| error.to_string())?;
        Answer::of(super::host_family::delivery(called?))
    }
}

/// Supplies the rejection the next UI request receives.
fn set_rejection(observed: &mut Observed, rejection: Option<&str>) {
    observed
        .ui
        .control(&serde_json::json!({ "error": rejection }).to_string());
}

/// The notifications the host recorded in `lines`, in time order.
fn notifications(lines: &[String]) -> Vec<Notification> {
    receipts(lines)
        .into_iter()
        .filter(|receipt| receipt.method == "notify")
        .map(|receipt| Notification {
            message: receipt.operands[0].as_str().unwrap_or_default().to_owned(),
            kind: receipt.operands[1].as_str().unwrap_or_default().to_owned(),
        })
        .collect()
}

/// Runs one recorded row against the host instance it selects.
fn run<H: Host>(hosts: &mut [H], row: &Row) -> Result<(), String> {
    let index = row.instance();
    match row {
        Row::Command {
            name,
            input,
            expected,
            ..
        } => {
            let (seen, outcome) = hosts[index].command(input);
            assert_eq!(seen, expected.notifications, "{name}: notifications");
            assert_eq!(outcome.err(), expected.error, "{name}: error");
        }
        Row::Event {
            name,
            input,
            expected,
            ..
        } => {
            let answer = hosts[index].event(input)?;
            assert_eq!(
                answer,
                Answer::returned(input, expected.result.clone()),
                "{name}"
            );
        }
    }
    Ok(())
}

/// The rows one controlled test replays, in recorded order.
fn group(corpus: &Corpus, want: Group) -> Vec<&Row> {
    corpus
        .rows
        .iter()
        .filter(|row| row.group() == want)
        .collect()
}

/// Builds `count` independent instances of the shipped component.
fn components(count: usize) -> Result<Vec<ComponentHost>, String> {
    let path = super::build::example_component("maestro-extensions-wasm", "pirate")?;
    (0..count).map(|_| ComponentHost::new(&path)).collect()
}

/// Replays the rows of one group over `count` built components.
fn replay_built(want: Group, count: usize) -> Result<(), String> {
    let corpus = corpus();
    let rows = group(&corpus, want);
    assert!(!rows.is_empty(), "recorded rows of {want:?}");
    let mut hosts = components(count)?;
    for row in rows {
        run(&mut hosts, row)?;
    }
    release_built(&mut hosts)
}

/// Releases every registration of each built instance and asserts none is left lent.
fn release_built(hosts: &mut [ComponentHost]) -> Result<(), String> {
    for host in hosts {
        for key in [host.command, host.event] {
            super::block_on(host.harness.release(key)).map_err(|error| error.to_string())?;
        }
        assert_eq!(host.harness.observed().live, 0, "every callback released");
    }
    Ok(())
}

/// The shipped factory registers exactly one command before one prompt handler.
#[test]
fn pirate_registers_command_and_prompt_handler() -> Result<(), String> {
    let metadata = corpus().metadata;
    let started = ControlledHost::new()?;
    assert_registered_command(&started.host.observed(), &metadata.command);
    release_all(&started.host, &recorded_order(&metadata));
    Ok(())
}

/// Every recorded toggling command and prompt observation of one instance.
#[test]
fn pirate_toggle_and_prompts_match_recorded_inputs() -> Result<(), String> {
    let corpus = corpus();
    let mut host = ControlledHost::new()?;
    for row in group(&corpus, Group::Toggle) {
        run(std::slice::from_mut(&mut host), row)?;
    }
    release_all(&host.host, &recorded_order(&corpus.metadata));
    Ok(())
}

/// Two factory instances keep the toggle state they each own.
#[test]
fn pirate_factories_keep_separate_state() -> Result<(), String> {
    let corpus = corpus();
    let mut hosts = components(2)?;
    let enabling = CommandInput {
        args: String::new(),
        rejection: None,
    };
    hosts[0].command(&enabling).1?;
    for row in group(&corpus, Group::Instances) {
        run(&mut hosts, row)?;
    }
    release_built(&mut hosts)
}

/// A refused notification reports the host error and leaves the toggle on.
#[test]
fn pirate_notify_failure_keeps_toggle() -> Result<(), String> {
    replay_built(Group::Rejection, 3)
}

/// The built single-copy example replays every recorded operation.
#[test]
fn pirate_component_runs_recorded_transitions() -> Result<(), String> {
    let corpus = corpus();
    let mut hosts = components(3)?;
    assert_registered_command(hosts[0].harness.observed(), &corpus.metadata.command);
    for row in &corpus.rows {
        run(&mut hosts, row)?;
    }
    release_built(&mut hosts)
}

/// The registered names in the order the host received them.
fn recorded_order(metadata: &Metadata) -> Vec<String> {
    metadata
        .order
        .iter()
        .map(|registration| format!("{} {}", registration.operation.name(), registration.name))
        .collect()
}

/// The recorded corpus these tests consume.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("pirate_expected.json")).expect("recorded corpus")
}

/// The recorded registration and observation rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// What the factory registers.
    metadata: Metadata,
    /// The recorded operations.
    rows: Vec<Row>,
}

/// The command and prompt handler the factory registers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    /// The registrations in the order the host received them.
    order: Vec<RegistrationRow>,
    /// The registered command.
    command: Command,
}

/// A registration operation of the recorded corpus.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Registration {
    /// A command registration.
    Command,
    /// An event handler registration.
    Event,
}

impl Registration {
    /// The host's own name for the registration kind.
    fn name(&self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Event => "event",
        }
    }
}

/// One registration of the recorded corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationRow {
    /// The registration operation the factory called.
    operation: Registration,
    /// The name it registered.
    name: String,
}

/// One recorded operation: its inputs and the observations it must produce.
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "lowercase", deny_unknown_fields)]
enum Row {
    /// A command invocation.
    Command {
        /// The case name in the recorded corpus.
        name: String,
        /// The factory instance it ran on.
        instance: usize,
        /// The controlled test that replays it.
        group: Group,
        /// The host inputs.
        input: CommandInput,
        /// The observations it must produce.
        expected: CommandExpected,
    },
    /// A prompt event delivery.
    Event {
        /// The case name in the recorded corpus.
        name: String,
        /// The factory instance it ran on.
        instance: usize,
        /// The controlled test that replays it.
        group: Group,
        /// The event document the host supplies.
        input: Value,
        /// The answer it must produce.
        expected: EventExpected,
    },
}

impl Row {
    /// The factory instance the row selects.
    fn instance(&self) -> usize {
        match self {
            Self::Command { instance, .. } | Self::Event { instance, .. } => *instance,
        }
    }

    /// The controlled test that replays the row.
    fn group(&self) -> Group {
        match self {
            Self::Command { group, .. } | Self::Event { group, .. } => *group,
        }
    }
}

/// The controlled test a row belongs to.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Group {
    /// One instance toggling and reading its prompt.
    Toggle,
    /// Two instances keeping their own state.
    Instances,
    /// A refused notification.
    Rejection,
}

/// The host inputs of one command case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandInput {
    /// The command arguments.
    args: String,
    /// The rejection the next notification receives.
    rejection: Option<String>,
}

/// The observations one command case must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandExpected {
    /// The notifications the host received.
    notifications: Vec<Notification>,
    /// The error the invocation produced.
    error: Option<String>,
}

/// One notification the host recorded.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Notification {
    /// The message shown.
    message: String,
    /// The severity, in the host's own spelling.
    kind: String,
}

/// The answer one event case must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventExpected {
    /// The replacement prompt, or none while the mode is off.
    result: Option<Value>,
}
