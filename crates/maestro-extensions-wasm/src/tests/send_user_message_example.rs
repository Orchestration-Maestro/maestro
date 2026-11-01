//! The shipped user-message factory exercised through both hosts.
use serde::Deserialize;
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::example_support::{
    Command as CommandRecord, Observation, invoke_command_on_component, observations, release_all,
    start,
};
use super::host::Harness;
use super::observed::Observed;
use crate::component_adapter::Exports;
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/send_user_message.rs"]
mod send_user_message;

/// The same factory exported by the shipped component.
struct SendUserMessage;

impl Extension for SendUserMessage {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        send_user_message::factory(api)
    }
}

/// The registrations the shipped factory makes, in the order the host received them.
const REGISTRATIONS: [&str; 4] = [
    "command ask",
    "command steer",
    "command followup",
    "command askwith",
];

/// The argument classes that carry no message.
const BLANK: [&str; 3] = [
    "",
    " \t\r\n ",
    "\u{a0}\u{1680}\u{2003}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}",
];

/// The argument classes native whitespace decides about: the byte-order mark is text and the
/// next-line character is blank.
const NATIVE_BLANK: [&str; 2] = ["\u{feff}", "\u{85}"];

/// The argument class that carries text with the whitespace around it.
const MESSAGE: &str = "  What is 雪 + e\u{301}?\r\n/steer literal  ";

/// The argument class that carries one zero-width character.
const ZERO_WIDTH: &str = "\u{200b}";

/// The recorded group name of the refused-operation cases.
const FAILURES: &str = "failures";

/// One invocation's observations and the answer it returned.
struct Answer {
    /// The observations the invocation made.
    lines: Vec<String>,
    /// The answer it returned, or the error it produced.
    outcome: Result<(), String>,
}

/// The host one recorded case runs against.
trait Host {
    /// Supplies the idle flag and the rejections one case names.
    fn prepare(&mut self, idle: bool, reject: Option<&str>);

    /// The command names the host admitted, in order.
    fn registrations(&self) -> Vec<String>;

    /// Runs the command `command` with `args`.
    fn run(&mut self, command: Command, args: &str) -> Answer;

    /// Releases every registration the host received.
    fn release(&mut self);
}

/// The controlled adapter with the keys the shipped factory registered.
struct ControlledHost {
    /// The host that recorded what the factory asked for.
    host: Controlled,
    /// The adapter serving the factory's callbacks.
    exports: Exports<Controlled>,
    /// The registered commands, at the position each was registered.
    keys: Vec<u32>,
}

impl Host for ControlledHost {
    fn prepare(&mut self, idle: bool, reject: Option<&str>) {
        let mut observed = self.host.observe();
        supply(&mut observed, idle, reject);
    }

    fn registrations(&self) -> Vec<String> {
        names(&self.host.observed().registrations.commands)
    }

    fn run(&mut self, command: Command, args: &str) -> Answer {
        let mark = self.host.observed().transcript.len();
        let context = self.host.command_context("/work");
        let outcome = super::block_on(self.exports.invoke_command(
            self.keys[command.position()],
            args.to_owned(),
            context,
        ));
        Answer {
            lines: self.host.observed().transcript[mark..].to_vec(),
            outcome,
        }
    }

    fn release(&mut self) {
        release_all(&self.host, &REGISTRATIONS);
    }
}

/// The component host with the keys the built factory registered.
struct ComponentHost {
    /// The instantiated component.
    harness: Harness,
    /// The registered commands, at the position each was registered.
    keys: Vec<u32>,
}

impl ComponentHost {
    /// Instantiates the built example and looks up its registrations.
    fn new() -> Result<Self, String> {
        let path = super::build::example_component("send_user_message")?;
        let harness = super::block_on(Harness::start(&path)).map_err(|error| error.to_string())?;
        let keys = keys(|name| harness.callback(name).map_err(|error| error.to_string()))?;
        Ok(Self { harness, keys })
    }
}

impl Host for ComponentHost {
    fn prepare(&mut self, idle: bool, reject: Option<&str>) {
        supply(self.harness.observed(), idle, reject);
    }

    fn registrations(&self) -> Vec<String> {
        names(&self.harness.store.data().observed.registrations.commands)
    }

    fn run(&mut self, command: Command, args: &str) -> Answer {
        let mark = self.harness.store.data().observed.transcript.len();
        let outcome = invoke_command_on_component(
            &mut self.harness,
            self.keys[command.position()],
            args,
            false,
        );
        Answer {
            lines: self.harness.store.data().observed.transcript[mark..].to_vec(),
            outcome,
        }
    }

    fn release(&mut self) {
        for name in REGISTRATIONS {
            let key = self.harness.callback(name).expect("a registration");
            super::block_on(self.harness.release(key)).expect("a released registration");
        }
        assert_eq!(self.harness.store.data().observed.live, 0);
    }
}

/// Starts the shipped factory on the controlled adapter.
fn controlled() -> Result<ControlledHost, String> {
    let (host, exports) = super::block_on(start::<SendUserMessage>())?;
    let keys = {
        let observed = host.observed();
        keys(|name| observed.callback(name))
    }?;
    Ok(ControlledHost {
        host,
        exports,
        keys,
    })
}

/// The keys the host holds the four registrations under, in registration order.
fn keys(lookup: impl Fn(&str) -> Result<u32, String>) -> Result<Vec<u32>, String> {
    REGISTRATIONS.into_iter().map(lookup).collect()
}

/// The command names of the recorded registrations, in order.
fn names(commands: &[(String, Option<String>, bool)]) -> Vec<String> {
    commands.iter().map(|(name, _, _)| name.clone()).collect()
}

/// Supplies the idle flag and the rejections one recorded case names.
fn supply(observed: &mut Observed, idle: bool, reject: Option<&str>) {
    let refusal = |operation: &str| reject.filter(|kind| *kind == operation).map(refused);
    observed
        .catalog
        .control(&json!({"state": {"idle": idle, "error": refusal("idle")}}).to_string());
    observed
        .ui
        .control(&json!({ "error": refusal("notify") }).to_string());
    observed.actions.error = refusal("send");
}

/// The message the host refuses `operation` with.
fn refused(operation: &str) -> String {
    match operation {
        "idle" => "idle refused",
        "notify" => "notification refused",
        "send" => "delivery refused",
        _ => "registration refused",
    }
    .to_owned()
}

/// The built single-copy example replays the recorded calls through its own registrations.
#[test]
fn user_delivery_component_replays_recorded_calls() -> Result<(), String> {
    let mut started = ComponentHost::new()?;
    let corpus = corpus();
    let cases: Vec<&Case> = corpus
        .rows
        .iter()
        .filter(|case| !refuses_registration(case))
        .collect();
    assert_eq!(cases.len(), 77, "the rows the component replays");
    for case in cases {
        run_case(&mut started, case);
    }
    started.release();
    Ok(())
}

/// Every command is registered with its own description and no completions, in order.
#[test]
fn user_delivery_registers_commands_in_order() -> Result<(), String> {
    let mut started = controlled()?;
    let recorded: Vec<(String, Option<String>, bool)> = corpus()
        .metadata
        .iter()
        .map(|command| {
            (
                command.name.clone(),
                Some(command.description.clone()),
                command.completions,
            )
        })
        .collect();
    assert_eq!(started.host.observed().registrations.commands, recorded);
    assert!(
        started.host.observed().tools.is_empty(),
        "no tool registered"
    );
    started.release();
    Ok(())
}

/// A blank argument warns before any delivery, whichever command receives it.
#[test]
fn user_delivery_blank_arguments_warn_before_delivery() -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    let cases: Vec<&Case> = corpus
        .rows
        .iter()
        .filter(|case| {
            case.group != FAILURES
                && case.input.reject.is_none()
                && BLANK.contains(&case.input.args.as_str())
        })
        .collect();
    assert_eq!(cases.len(), 24, "the blank rows");
    for case in cases {
        run_case(&mut host, case);
    }
    host.release();
    Ok(())
}

/// The ordinary command forwards the original text while idle and warns while busy.
#[test]
fn user_delivery_ask_checks_idle_and_keeps_text() -> Result<(), String> {
    replay_messages(Command::Ask, 4)
}

/// Steering selects its own delivery only while the agent is busy.
#[test]
fn user_delivery_steer_selects_only_while_busy() -> Result<(), String> {
    replay_messages(Command::Steer, 4)
}

/// A follow-up names its delivery and reports the queue only after the host admitted it.
#[test]
fn user_delivery_followup_reports_only_the_queued_branch() -> Result<(), String> {
    replay_messages(Command::Followup, 4)
}

/// The structured command sends two text blocks in order while idle and warns while busy.
#[test]
fn user_delivery_askwith_keeps_two_text_blocks() -> Result<(), String> {
    replay_messages(Command::Askwith, 4)
}

/// Boundary whitespace follows the native trim, which keeps the byte-order mark and drops the
/// next-line character.
#[test]
fn user_delivery_blank_checks_use_native_whitespace() -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    let cases: Vec<&Case> = corpus
        .rows
        .iter()
        .filter(|case| {
            case.group != FAILURES
                && case.input.reject.is_none()
                && NATIVE_BLANK.contains(&case.input.args.as_str())
        })
        .collect();
    assert_eq!(cases.len(), 16, "the boundary-whitespace rows");
    for case in cases {
        run_case(&mut host, case);
    }
    host.release();
    Ok(())
}

/// A refused host operation stops the invocation and keeps the effects before it.
#[test]
fn user_delivery_rejections_stop_at_the_failed_operation() -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    let cases: Vec<&Case> = corpus
        .rows
        .iter()
        .filter(|case| case.group == FAILURES)
        .collect();
    assert_eq!(cases.len(), 25, "the failure rows");
    let mut refused_at_startup = 0;
    for case in cases {
        if refuses_registration(case) {
            refused_registration(case)?;
            refused_at_startup += 1;
        } else {
            run_case(&mut host, case);
        }
    }
    assert_eq!(refused_at_startup, 4, "the refused registrations");
    host.release();
    Ok(())
}

/// Replays the recorded cases of `command` that carry a message.
fn replay_messages(command: Command, count: usize) -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    let cases: Vec<&Case> = corpus
        .rows
        .iter()
        .filter(|case| {
            case.group != FAILURES
                && case.input.reject.is_none()
                && case.input.command == command
                && [MESSAGE, ZERO_WIDTH].contains(&case.input.args.as_str())
        })
        .collect();
    assert_eq!(cases.len(), count, "the rows of the recorded group");
    for case in cases {
        run_case(&mut host, case);
    }
    host.release();
    Ok(())
}

/// Runs one recorded case and asserts every observation it records.
fn run_case<H: Host>(host: &mut H, case: &Case) {
    host.prepare(case.input.idle, case.input.reject.as_deref());
    let answer = host.run(case.input.command, &case.input.args);
    assert_eq!(
        host.registrations(),
        case.expected.registration,
        "{}: registrations",
        case.name
    );
    assert_eq!(
        calls(&answer.lines),
        case.expected.calls,
        "{}: calls",
        case.name
    );
    assert_eq!(
        answer.outcome.as_ref().err(),
        case.expected.error.as_ref(),
        "{}: answer",
        case.name
    );
}

/// Refuses one registration before the factory starts and asserts the admitted prefix.
fn refused_registration(case: &Case) -> Result<(), String> {
    let name = case
        .input
        .reject
        .as_deref()
        .and_then(|reject| reject.strip_prefix("register:"))
        .ok_or("a registration rejection")?;
    let position = REGISTRATIONS
        .iter()
        .position(|registration| *registration == format!("command {name}"))
        .ok_or("a registered command")?;
    let host = Controlled::default();
    let exports = Exports::new(host.clone());
    host.observe().registrations.refuse_at = Some((position, refused("register")));
    let started = super::block_on(exports.start::<SendUserMessage>());
    assert_eq!(started, Err(refused("register")), "{}: answer", case.name);
    assert_eq!(
        calls(&host.observed().transcript),
        case.expected.calls,
        "{}: effects",
        case.name
    );
    let admitted: Vec<String> = case
        .expected
        .registration
        .iter()
        .map(|name| format!("command {name}"))
        .collect();
    release_all(&host, &admitted);
    Ok(())
}

/// Whether the case refuses a registration, which the controlled adapter arms before startup.
fn refuses_registration(case: &Case) -> bool {
    case.input
        .reject
        .as_deref()
        .is_some_and(|reject| reject.starts_with("register:"))
}

/// The effects the host produced, in the order it produced them.
fn calls(lines: &[String]) -> Vec<Call> {
    observations(lines)
        .into_iter()
        .map(|observation| match observation {
            Observation::Notify { message, kind } => Call::Notify {
                message,
                kind: kind.expect("a notification severity"),
            },
            Observation::Delivery { content, options } => Call::SendUserMessage {
                content: content.expect("delivered content"),
                options,
            },
        })
        .collect()
}

/// The recorded observations these tests consume.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("send_user_message_expected.json"))
        .expect("the recorded corpus")
}

/// The recorded registrations and cases.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// What the factory registers.
    metadata: Vec<CommandRecord>,
    /// The recorded cases.
    rows: Vec<Case>,
}

/// One recorded case: its inputs and the observations it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The case name in the recorded corpus.
    name: String,
    /// The recorded group the case belongs to.
    group: String,
    /// What the invocation receives.
    input: Input,
    /// The observations the case must produce.
    expected: Expected,
}

/// The inputs one recorded case supplies.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// The command the case invokes.
    command: Command,
    /// The command arguments.
    args: String,
    /// Whether the agent is idle when the command runs.
    idle: bool,
    /// The host operation the case refuses, when it refuses one.
    reject: Option<String>,
}

/// A command the shipped factory registers.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Command {
    /// The ordinary message command.
    Ask,
    /// The steering command.
    Steer,
    /// The follow-up command.
    Followup,
    /// The structured-content command.
    Askwith,
}

impl Command {
    /// The position the factory registers the command at, counted from zero.
    fn position(self) -> usize {
        match self {
            Self::Ask => 0,
            Self::Steer => 1,
            Self::Followup => 2,
            Self::Askwith => 3,
        }
    }
}

/// The observations one case must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// The commands the host admits, in registration order.
    registration: Vec<String>,
    /// The effects the invocation produces, in order.
    calls: Vec<Call>,
    /// The error the invocation produces.
    error: Option<String>,
}

/// One effect a recorded case expects.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "operation", rename_all = "camelCase")]
enum Call {
    /// A notification request.
    Notify {
        /// The message text.
        message: String,
        /// The severity the request carries.
        kind: String,
    },
    /// An admitted user-message delivery.
    SendUserMessage {
        /// The delivered content.
        content: Value,
        /// The delivery options, absent when the request carries none.
        options: Option<Value>,
    },
}
