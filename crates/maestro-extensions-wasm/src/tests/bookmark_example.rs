//! The shipped bookmark factory exercised through both hosts.
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::example_support::{invoke_command_on_component, release_all, start};
use super::host::Harness;
use super::reader_host::Reader;
use crate::component_adapter::Exports;
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/bookmark.rs"]
mod bookmark;

/// The same factory exported by the shipped component.
struct Bookmark;

impl Extension for Bookmark {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        bookmark::factory(api)
    }
}

/// The registrations the shipped factory makes, in the order the host received them.
const REGISTRATIONS: [&str; 1] = ["command bookmark"];

/// One invocation's observations and the answer it returned.
struct Answer {
    /// The observations the invocation made.
    lines: Vec<String>,
    /// The answer it returned, or the error it produced.
    outcome: Result<(), String>,
}

/// The host one recorded case runs against.
trait Host {
    /// Binds the supplied reader replies and returns the reader it bound.
    fn supply(&mut self, supplied: Value) -> Result<Reader, String>;

    /// Supplies the label-write and notification rejections `failure` names.
    fn reject(&mut self, failure: Option<&str>);

    /// Supplies the instant the standard library clock reports, counting from zero.
    fn set_clock(&mut self, millis: u64);

    /// The clock reads since the last reset, when this host supplies a clock.
    fn clock_reads(&self) -> Option<usize>;

    /// Runs the bookmark command with `args`.
    fn run(&mut self, args: &str) -> Answer;

    /// Releases every registration the host received.
    fn release(&mut self);
}

/// The controlled adapter with the key the shipped factory registered.
struct ControlledHost {
    /// The host that recorded what the factory asked for.
    host: Controlled,
    /// The adapter serving the factory's callbacks.
    exports: Exports<Controlled>,
    /// The registered bookmark command.
    command: u32,
}

impl Host for ControlledHost {
    fn supply(&mut self, supplied: Value) -> Result<Reader, String> {
        let observed = &mut self.host.observe();
        observed
            .readers
            .control(&json!({ "bind": supplied }).to_string());
        observed.readers.acquire()
    }

    fn reject(&mut self, failure: Option<&str>) {
        let observed = &mut self.host.observe();
        observed.actions.error = supplied_failure(failure, "setLabel");
        observed
            .ui
            .control(&json!({ "error": supplied_failure(failure, "notify") }).to_string());
    }

    fn set_clock(&mut self, _millis: u64) {}

    fn clock_reads(&self) -> Option<usize> {
        None
    }

    fn run(&mut self, args: &str) -> Answer {
        let mark = self.host.observed().transcript.len();
        let context = self.host.command_context("/work");
        let outcome = super::block_on(self.exports.invoke_command(
            self.command,
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

/// The component host and the key the built factory registered.
struct ComponentHost {
    /// The instantiated component.
    harness: Harness,
    /// The registered bookmark command.
    command: u32,
}

impl ComponentHost {
    /// Instantiates the built example and looks up its registration.
    fn new() -> Result<Self, String> {
        let path = super::build::example_component("bookmark")?;
        let (engine, component) =
            super::build::compile(&path).map_err(|error| error.to_string())?;
        let harness = super::block_on(Harness::from_compiled(&engine, &component))
            .map_err(|error| error.to_string())?;
        let command = harness
            .callback("command bookmark")
            .map_err(|error| error.to_string())?;
        Ok(Self { harness, command })
    }
}

impl Host for ComponentHost {
    fn supply(&mut self, supplied: Value) -> Result<Reader, String> {
        self.harness
            .observed()
            .readers
            .control(&json!({ "bind": supplied }).to_string());
        self.harness.store.data().observed.readers.acquire()
    }

    fn reject(&mut self, failure: Option<&str>) {
        let observed = self.harness.observed();
        observed.actions.error = supplied_failure(failure, "setLabel");
        observed
            .ui
            .control(&json!({ "error": supplied_failure(failure, "notify") }).to_string());
    }

    fn set_clock(&mut self, millis: u64) {
        let clock = self.harness.clock();
        clock.set(millis);
        clock.reset_reads();
    }

    fn clock_reads(&self) -> Option<usize> {
        Some(self.harness.clock().reads())
    }

    fn run(&mut self, args: &str) -> Answer {
        let mark = self.harness.store.data().observed.transcript.len();
        let outcome = invoke_command_on_component(&mut self.harness, self.command, args, false);
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
    let (host, exports) = super::block_on(start::<Bookmark>())?;
    let command = host.observed().callback("command bookmark")?;
    Ok(ControlledHost {
        host,
        exports,
        command,
    })
}

/// Exactly the recorded command registrations, in order, and nothing else.
#[test]
fn bookmark_registers_command_metadata() -> Result<(), String> {
    let mut started = controlled()?;
    let recorded = recorded_metadata(&corpus(), &REGISTRATIONS);
    assert_eq!(started.host.observed().registrations.commands, recorded);
    assert!(
        started.host.observed().tools.is_empty(),
        "no tool registered"
    );
    started.release();
    Ok(())
}

/// Every recorded explicit-label case labels the newest assistant message.
#[test]
fn bookmark_selects_latest_assistant() -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    let cases = selected(&corpus, |case| {
        case.command == Command::Bookmark
            && case.failure.is_none()
            && case.expected.clock_reads == 0
            && !case.expected.sets.is_empty()
    });
    for case in &cases {
        run_case(&mut host, case);
    }
    host.release();
    Ok(())
}

/// A session without an eligible assistant message is reported and left unlabelled.
#[test]
fn bookmark_missing_assistant_warns() -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    for case in selected(&corpus, |case| {
        case.command == Command::Bookmark
            && case.failure.is_none()
            && case.expected.sets.is_empty()
            && case.expected.notifications[0].kind.as_deref() == Some("warning")
    }) {
        run_case(&mut host, case);
    }
    host.release();
    Ok(())
}

/// Boundary whitespace follows the native trim, which keeps U+FEFF and drops U+0085.
#[test]
fn bookmark_labels_use_native_whitespace() -> Result<(), String> {
    let mut host = controlled()?;
    for (name, args, label) in [
        (
            "bom-boundary",
            "\u{feff}topic\u{feff}",
            "\u{feff}topic\u{feff}",
        ),
        ("nel-boundary", "\u{85}topic\u{85}", "topic"),
    ] {
        let case = Case {
            name: name.to_owned(),
            command: Command::Bookmark,
            args: args.to_owned(),
            entries: json!([assistant("assistant")]),
            labels: BTreeMap::default(),
            now: 1_700_000_000_123,
            failure: None,
            expected: Expected {
                sets: vec![Set {
                    id: "assistant".into(),
                    label: Some(label.to_owned()),
                }],
                notifications: vec![Notification {
                    message: format!("Bookmarked as: {label}"),
                    kind: Some("info".into()),
                }],
                queries: Vec::new(),
                clock_reads: 0,
                error: None,
            },
        };
        run_case(&mut host, &case);
    }
    host.release();
    Ok(())
}

/// The built single-copy example replays the recorded cases through its callbacks.
#[test]
fn bookmark_component_replays_recorded_cases() -> Result<(), String> {
    let mut started = ComponentHost::new()?;
    let corpus = corpus();
    let cases = selected(&corpus, |case| case.command == Command::Bookmark);
    for case in &cases {
        run_case(&mut started, case);
    }
    started.release();
    Ok(())
}

/// The recorded metadata of the commands `names` register, in registration order.
fn recorded_metadata(corpus: &Corpus, names: &[&str]) -> Vec<(String, Option<String>, bool)> {
    corpus
        .metadata
        .iter()
        .filter(|command| names.contains(&format!("command {}", command.name).as_str()))
        .map(|command| {
            (
                command.name.clone(),
                Some(command.description.clone()),
                command.completions,
            )
        })
        .collect()
}

/// The recorded cases a selection finds at least one of.
fn selected(corpus: &Corpus, keep: impl Fn(&Case) -> bool) -> Vec<&Case> {
    let selected: Vec<&Case> = corpus.rows.iter().filter(|case| keep(case)).collect();
    assert!(!selected.is_empty(), "recorded cases");
    selected
}

/// Runs one recorded case and asserts every observation it records.
fn run_case<H: Host>(host: &mut H, case: &Case) {
    let reader = host.supply(supplied(case)).expect("the supplied reader");
    host.reject(case.failure.as_deref());
    host.set_clock(case.now);
    let answer = host.run(&case.args);
    assert_eq!(
        label_writes(&answer.lines),
        case.expected.sets,
        "{}: label writes",
        case.name
    );
    assert_eq!(
        notifications(&answer.lines),
        case.expected.notifications,
        "{}: notifications",
        case.name
    );
    assert_eq!(
        label_queries(&reader),
        case.expected.queries,
        "{}: label queries",
        case.name
    );
    assert_eq!(
        answer.outcome.as_ref().err(),
        case.expected.error.as_ref(),
        "{}: answer",
        case.name
    );
    if let Some(reads) = host.clock_reads() {
        assert_eq!(reads, case.expected.clock_reads, "{}: clock", case.name);
    }
}

/// The reader replies one recorded case supplies.
fn supplied(case: &Case) -> Value {
    let mut replies = vec![json!({
        "method": "getEntries",
        "args": [],
        "value": case.entries.clone(),
        "error": supplied_failure(case.failure.as_deref(), "getEntries"),
    })];
    for id in &case.expected.queries {
        replies.push(json!({
            "method": "getLabel",
            "args": [id],
            "value": case.labels.get(id).map_or(Value::Null, |label| json!(label)),
            "error": supplied_failure(case.failure.as_deref(), "getLabel"),
        }));
    }
    json!({ "replies": replies })
}

/// The rejection one recorded case injects into `operation`.
fn supplied_failure(failure: Option<&str>, operation: &str) -> Option<String> {
    failure
        .filter(|failure| *failure == operation)
        .map(|_| format!("host refused {operation}"))
}

/// The label writes the host admitted, with the label each carried.
fn label_writes(lines: &[String]) -> Vec<Set> {
    lines
        .iter()
        .filter_map(|line| line.strip_prefix("action setLabel "))
        .map(|rest| {
            let (id, label): (String, Presence) =
                serde_json::from_str(rest).expect("a label write");
            Set {
                id,
                label: label.present.then_some(label.value).flatten(),
            }
        })
        .collect()
}

/// The notifications the host was asked for, with their severity.
fn notifications(lines: &[String]) -> Vec<Notification> {
    super::ui_host::receipts(lines)
        .into_iter()
        .filter(|receipt| receipt.method == "notify")
        .map(|receipt| Notification {
            message: receipt.operands[0]
                .as_str()
                .expect("a notification message")
                .to_owned(),
            kind: receipt.operands[1].as_str().map(str::to_lowercase),
        })
        .collect()
}

/// The identities the guest queried a label for, in query order.
fn label_queries(reader: &Reader) -> Vec<String> {
    reader
        .queries()
        .into_iter()
        .filter(|query| query.method == "getLabel")
        .map(|query| {
            query.args[0]
                .as_str()
                .expect("a queried identity")
                .to_owned()
        })
        .collect()
}

/// One session entry holding an assistant message under `id`.
fn assistant(id: &str) -> Value {
    json!({
        "type": "message",
        "id": id,
        "parentId": null,
        "timestamp": "2026-01-01T00:00:00.000Z",
        "message": {
            "role": "assistant",
            "content": [],
            "api": "test-api",
            "provider": "test-provider",
            "model": "test-model",
            "usage": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "totalTokens": 0,
                "cost": {
                    "input": 0,
                    "output": 0,
                    "cacheRead": 0,
                    "cacheWrite": 0,
                    "total": 0
                }
            },
            "stopReason": "stop",
            "timestamp": 0
        }
    })
}

/// The recorded observations these tests consume.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("bookmark_expected.json")).expect("the recorded corpus")
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

/// One recorded command registration.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRecord {
    /// The registered name.
    name: String,
    /// The registered description.
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
    /// The command the case invokes.
    command: Command,
    /// The command arguments.
    args: String,
    /// The session entries the host supplies, in session order.
    entries: Value,
    /// The labels the host holds before the invocation.
    labels: BTreeMap<String, String>,
    /// The instant the clock reports, in epoch milliseconds.
    now: u64,
    /// The host operation the case rejects, when it rejects one.
    failure: Option<String>,
    /// The observations the case must produce.
    expected: Expected,
}

/// A command the shipped factory registers.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Command {
    /// The bookmark command.
    Bookmark,
    /// The removal command.
    Unbookmark,
}

/// The observations one case must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// The label writes the host admits, in order.
    sets: Vec<Set>,
    /// The notifications the host is asked for, in order.
    notifications: Vec<Notification>,
    /// The identities read for a label, in query order.
    queries: Vec<String>,
    /// How many times the invocation reads the clock.
    #[serde(rename = "clockReads")]
    clock_reads: usize,
    /// The error the invocation produces.
    error: Option<String>,
}

/// One label write the host admits.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Set {
    /// The entry that receives the label.
    id: String,
    /// The label it receives, or its absence to clear the label.
    #[serde(default)]
    label: Option<String>,
}

/// One notification the host is asked for.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Notification {
    /// The message text.
    message: String,
    /// The severity the request carries.
    #[serde(default)]
    kind: Option<String>,
}

/// A presence wrapper the host's wire records use for optional operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Presence {
    /// Whether the member is present at all.
    present: bool,
    /// The member's value, when it carries one.
    #[serde(default)]
    value: Option<String>,
}
