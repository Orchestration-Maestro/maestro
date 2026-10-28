//! The shipped session-name factory exercised through both hosts.
use serde::Deserialize;
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::example_support::{invoke_command_on_component, release_all, start};
use super::host::Harness;
use super::observed::Observed;
use super::ui_host::receipts;
use crate::component_adapter::Exports;
use crate::{Extension, ExtensionAPI, ExtensionFuture, NotificationType};

#[path = "../../../../examples/extensions/session_name.rs"]
mod session_name;

/// The same factory exported by the shipped component.
struct SessionName;

impl Extension for SessionName {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        session_name::factory(api)
    }
}

/// What one invocation observed and the answer it returned.
struct Answer {
    /// The observations the invocation made.
    lines: Vec<String>,
    /// The answer it returned, or the error it produced.
    outcome: Result<(), String>,
}

/// A host that registers the factory and runs the command it registered.
trait Host {
    /// Supplies the case's name and refusal, then runs its command on the retained callback.
    fn command(&mut self, case: &Case) -> Answer;

    /// The observations made since `mark`.
    fn since(&self, mark: usize) -> Vec<String>;

    /// The current observation mark.
    fn mark(&self) -> usize;
}

/// The controlled adapter with the key the shipped factory registered.
struct ControlledHost {
    /// The host that recorded what the factory asked for.
    host: Controlled,
    /// The adapter serving the factory's callback.
    exports: Exports<Controlled>,
    /// The registered command.
    command: u32,
}

impl Host for ControlledHost {
    fn command(&mut self, case: &Case) -> Answer {
        let mark = self.mark();
        supply(&mut self.host.observe(), case);
        let host = &self.host;
        let exports = &self.exports;
        let key = self.command;
        let args = case.input.args.clone();
        let outcome = super::block_on(async move {
            exports
                .invoke_command(key, args, host.command_context("/work"))
                .await
        });
        Answer {
            lines: self.since(mark),
            outcome,
        }
    }

    fn since(&self, mark: usize) -> Vec<String> {
        self.host.observed().transcript[mark..].to_vec()
    }

    fn mark(&self) -> usize {
        self.host.observed().transcript.len()
    }
}

/// The built component and the key of the command it registered.
struct ComponentHost {
    /// The instantiated component.
    harness: Harness,
    /// The registered command.
    command: u32,
}

impl ComponentHost {
    /// Instantiates the built example and looks up its command registration.
    fn new() -> Result<Self, String> {
        let path = super::build::example_component("session_name")?;
        let (engine, component) =
            super::build::compile(&path).map_err(|error| error.to_string())?;
        let harness = super::block_on(Harness::from_compiled(&engine, &component))
            .map_err(|error| error.to_string())?;
        let command = harness
            .callback("command session-name")
            .map_err(|error| error.to_string())?;
        Ok(Self { harness, command })
    }
}

impl Host for ComponentHost {
    fn command(&mut self, case: &Case) -> Answer {
        let mark = self.mark();
        supply(self.harness.observed(), case);
        let outcome =
            invoke_command_on_component(&mut self.harness, self.command, &case.input.args, false);
        Answer {
            lines: self.since(mark),
            outcome,
        }
    }

    fn since(&self, mark: usize) -> Vec<String> {
        self.harness.store.data().observed.transcript[mark..].to_vec()
    }

    fn mark(&self) -> usize {
        self.harness.store.data().observed.transcript.len()
    }
}

/// Supplies the case's current name and the operation it refuses to a host's observations.
fn supply(observed: &mut Observed, case: &Case) {
    observed.actions.value = case
        .input
        .current
        .clone()
        .map_or(Value::Null, Value::String);
    observed.actions.error = None;
    observed.ui.control(&json!({ "error": null }).to_string());
    let Some(refusal) = case.input.reject else {
        return;
    };
    let error = case.expected.error.clone();
    match refusal {
        Refusal::Set | Refusal::Get => observed.actions.error = error,
        Refusal::Notify => observed.ui.control(&json!({ "error": error }).to_string()),
        // A registration refusal is armed before the factory runs, not per invocation.
        Refusal::Register => {}
    }
}

/// Starts the shipped factory on the controlled adapter.
fn controlled() -> Result<ControlledHost, String> {
    let (host, exports) = super::block_on(start::<SessionName>())?;
    let command = host.observed().callback("command session-name")?;
    Ok(ControlledHost {
        host,
        exports,
        command,
    })
}

/// The registered command is the only registration, with the recorded metadata.
#[test]
fn session_name_registers_command_metadata() -> Result<(), String> {
    let host = controlled()?;
    assert_metadata(&host.host.observed(), &corpus().metadata);
    release_all(&host.host, &["command session-name"]);
    Ok(())
}

/// Assert the recorded command registration against what the host received.
fn assert_metadata(observed: &Observed, expected: &Metadata) {
    assert_eq!(
        observed.registrations.commands,
        [(
            expected.name.clone(),
            Some(expected.description.clone()),
            expected.completions
        )],
        "the registered command and its argument completions"
    );
    assert!(observed.tools.is_empty(), "the factory registers no tool");
    assert_eq!(
        observed.transcript,
        [format!("register command {}", expected.name)],
        "the factory registers no event and reaches nothing else"
    );
}

/// Every nonempty trimmed argument is written and then announced.
#[test]
fn session_name_sets_trimmed_names() -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    let cases = selected(&corpus.records, writes_a_name);
    assert_eq!(cases.len(), 8, "the recorded write cases");
    for case in cases {
        assert_case(&mut host, case);
    }
    release_all(&host.host, &["command session-name"]);
    Ok(())
}

/// A blank argument reads the name now in effect and shows it unchanged.
#[test]
fn session_name_shows_current_or_absent_name() -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    let cases = selected(&corpus.records, shows_the_name);
    assert_eq!(cases.len(), 10, "the recorded read cases");
    let changed: Vec<&str> = cases
        .iter()
        .filter(|case| case.name.starts_with("externally_changed"))
        .map(|case| case.name.as_str())
        .collect();
    assert_eq!(
        changed,
        ["externally_changed_first", "externally_changed_second"],
        "the read cases change the supplied name between calls on one callback"
    );
    for case in cases {
        assert_case(&mut host, case);
    }
    release_all(&host.host, &["command session-name"]);
    Ok(())
}

/// Each refused host operation is returned unchanged and stops the command.
#[test]
fn session_name_propagates_host_errors() -> Result<(), String> {
    let corpus = corpus();
    let refusals = selected(&corpus.records, |case| case.expected.error.is_some());
    assert_eq!(refusals.len(), 4, "the recorded refusal cases");
    for case in refusals
        .iter()
        .filter(|case| case.input.reject == Some(Refusal::Register))
    {
        refused_registration(case);
    }
    let mut host = controlled()?;
    for case in refusals
        .iter()
        .filter(|case| case.input.reject != Some(Refusal::Register))
    {
        assert_case(&mut host, case);
    }
    release_all(&host.host, &["command session-name"]);
    Ok(())
}

/// A refused registration returns the host's message, keeps nothing and reaches nothing.
fn refused_registration(case: &Case) {
    let host = Controlled::default();
    let error = case.expected.error.clone().unwrap_or_default();
    host.observe().registrations.error = Some(error);
    let exports = Exports::new(host.clone());
    let outcome = super::block_on(exports.start::<SessionName>());
    assert_refused(case, outcome, &host.observed());
}

/// The built component's refused registration, observed through the host it ran on.
fn refused_in_component(case: &Case) -> Result<(), String> {
    let path = super::build::example_component("session_name")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    let error = case.expected.error.clone().unwrap_or_default();
    let (mut harness, outcome) = super::block_on(Harness::from_compiled_refusing(
        &engine, &component, &error,
    ))
    .map_err(|error| error.to_string())?;
    assert_refused(case, outcome, harness.observed());
    Ok(())
}

/// A refused start returned the host's message and left nothing registered or recorded.
fn assert_refused(case: &Case, outcome: Result<(), String>, observed: &Observed) {
    assert_eq!(
        outcome.err(),
        case.expected.error,
        "{}: the registration refusal",
        case.name
    );
    assert!(
        observed.registrations.commands.is_empty(),
        "{}: nothing stays registered",
        case.name
    );
    assert!(
        observed.transcript.is_empty(),
        "{}: no session or UI operation runs",
        case.name
    );
}

/// The built component registers the same command and runs every invocable case.
#[test]
fn session_name_component_runs_registered_command() -> Result<(), String> {
    let mut started = ComponentHost::new()?;
    let corpus = corpus();
    assert_metadata(started.harness.observed(), &corpus.metadata);
    for case in corpus
        .records
        .iter()
        .filter(|case| case.input.reject != Some(Refusal::Register))
    {
        assert_case(&mut started, case);
    }
    let refusals = selected(&corpus.records, |case| {
        case.input.reject == Some(Refusal::Register)
    });
    for case in refusals {
        refused_in_component(case)?;
    }
    let key = started.command;
    super::block_on(started.harness.release(key)).map_err(|error| error.to_string())?;
    assert_eq!(started.harness.observed().live, 0);
    assert_eq!(started.harness.observed().dropped.len(), 1);
    Ok(())
}

/// Runs one case and asserts its observations and answer.
fn assert_case<H: Host>(host: &mut H, case: &Case) {
    let answer = host.command(case);
    assert_observations(&answer.lines, case);
    match &case.expected.error {
        Some(error) => assert_eq!(
            answer.outcome.as_ref().err(),
            Some(error),
            "{}: the returned error",
            case.name
        ),
        None => assert!(
            answer.outcome.is_ok(),
            "{}: {:?}",
            case.name,
            answer.outcome.as_ref().err()
        ),
    }
}

/// Assert the writes a case admits and the notifications it shows, each in order.
///
/// The host's name getter records nothing, so the read branch is identified by the notification
/// built from the independently supplied current name and by the injected getter refusal. A write
/// records its line only once the host admits it, which is why a refused write leaves none.
fn assert_observations(lines: &[String], case: &Case) {
    assert_eq!(
        writes(lines),
        admitted_writes(case),
        "{}: session writes",
        case.name
    );
    assert_eq!(
        notifications(lines),
        recorded_notifications(case),
        "{}: notifications",
        case.name
    );
    let write = lines.iter().position(|line| line.starts_with("action "));
    let notification = lines.iter().position(|line| line.starts_with("ui "));
    if let (Some(write), Some(notification)) = (write, notification) {
        assert!(
            write < notification,
            "{}: the admitted write precedes the notification",
            case.name
        );
    }
}

/// The session-name writes the host admitted, in order.
fn writes(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| line.strip_prefix("action setSessionName "))
        .map(|rest| {
            let [name]: [String; 1] = serde_json::from_str(rest).expect("a written session name");
            name
        })
        .collect()
}

/// The notifications the host recorded, in order.
fn notifications(lines: &[String]) -> Vec<Value> {
    receipts(lines)
        .into_iter()
        .filter(|receipt| receipt.method == "notify")
        .map(|receipt| receipt.operands)
        .collect()
}

/// The writes the host admits for the case, in order.
fn admitted_writes(case: &Case) -> Vec<String> {
    if case.input.reject == Some(Refusal::Set) {
        return Vec::new();
    }
    case.expected
        .calls
        .iter()
        .filter(|call| call.operation == Operation::Set)
        .filter_map(|call| call.value.clone())
        .collect()
}

/// The notifications the host records for the case, in order.
///
/// The recorded corpus names the severity the way the extension writes it; the host records the
/// name of the delivered variant that severity selects.
fn recorded_notifications(case: &Case) -> Vec<Value> {
    let severity = super::ui_host::variant(NotificationType::Info);
    case.expected
        .calls
        .iter()
        .filter(|call| call.operation == Operation::Notify)
        .map(|call| {
            assert_eq!(
                call.kind.as_deref(),
                Some("info"),
                "{}: the recorded severity",
                case.name
            );
            json!([call.message, &severity])
        })
        .collect()
}

/// The cases these predicates select, which a selection must find at least one of.
fn selected(cases: &[Case], predicate: impl Fn(&Case) -> bool) -> Vec<&Case> {
    let selected: Vec<&Case> = cases.iter().filter(|case| predicate(case)).collect();
    assert!(!selected.is_empty(), "recorded cases");
    selected
}

/// Whether the case writes the trimmed argument and succeeds.
fn writes_a_name(case: &Case) -> bool {
    case.expected.error.is_none()
        && case
            .expected
            .calls
            .iter()
            .any(|call| call.operation == Operation::Set)
}

/// Whether the case shows the name now in effect and succeeds.
fn shows_the_name(case: &Case) -> bool {
    case.expected.error.is_none()
        && !case
            .expected
            .calls
            .iter()
            .any(|call| call.operation == Operation::Set)
}

/// The recorded observations these tests consume.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("session_name_expected.json")).expect("recorded corpus")
}

/// The recorded registration and case observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// What the factory registers.
    metadata: Metadata,
    /// The recorded cases.
    records: Vec<Case>,
}

/// The command the factory registers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    /// The command name.
    name: String,
    /// The command description.
    description: String,
    /// Whether the factory registers argument completions.
    completions: bool,
}

/// One recorded case: its inputs and the observations it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The case name in the recorded corpus.
    name: String,
    /// The host inputs it supplies.
    input: Input,
    /// The observations it must produce.
    expected: Expected,
}

/// The host inputs of one case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// The command arguments.
    args: String,
    /// The current session name the host supplies, or none.
    current: Option<String>,
    /// The host operation the case refuses, or none.
    reject: Option<Refusal>,
}

/// The observations one case must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// The calls the case makes, in order.
    calls: Vec<Call>,
    /// The error the invocation produces.
    error: Option<String>,
}

/// One call a case makes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    /// The requested operation.
    operation: Operation,
    /// The name a write supplies.
    value: Option<String>,
    /// The text a notification shows.
    message: Option<String>,
    /// The notification severity.
    kind: Option<String>,
}

/// An operation a case requests.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Operation {
    /// Write the session name.
    Set,
    /// Read the session name.
    Get,
    /// Show a notification.
    Notify,
}

/// The host operation a case refuses.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Refusal {
    /// Command registration.
    Register,
    /// The session name write.
    Set,
    /// The session name read.
    Get,
    /// The notification request.
    Notify,
}
