//! The shipped bookmark factory exercised through both hosts.
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::example_support::{
    Command as CommandRecord, Observation, invoke_command_on_component, release_all, start,
};
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
const REGISTRATIONS: [&str; 2] = ["command bookmark", "command unbookmark"];

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

    /// Runs the command `command` with `args`.
    fn run(&mut self, command: Command, args: &str) -> Answer;

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
    bookmark: u32,
    /// The registered removal command.
    unbookmark: u32,
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

    fn run(&mut self, command: Command, args: &str) -> Answer {
        let mark = self.host.observed().transcript.len();
        let context = self.host.command_context("/work");
        let key = match command {
            Command::Bookmark => self.bookmark,
            Command::Unbookmark => self.unbookmark,
        };
        let outcome = super::block_on(self.exports.invoke_command(key, args.to_owned(), context));
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
    bookmark: u32,
    /// The registered removal command.
    unbookmark: u32,
}

impl ComponentHost {
    /// Instantiates the built example and looks up its registration.
    fn new() -> Result<Self, String> {
        let path = super::build::example_component("maestro-extensions-wasm", "bookmark")?;
        let (engine, component) =
            super::build::compile(&path).map_err(|error| error.to_string())?;
        let harness = super::block_on(Harness::from_compiled_with_clock(
            &engine,
            &component,
            super::host::Clock::default(),
        ))
        .map_err(|error| error.to_string())?;
        let bookmark = harness
            .callback("command bookmark")
            .map_err(|error| error.to_string())?;
        let unbookmark = harness
            .callback("command unbookmark")
            .map_err(|error| error.to_string())?;
        Ok(Self {
            harness,
            bookmark,
            unbookmark,
        })
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
        let clock = self.harness.clock().expect("a supplied clock");
        clock.set(millis);
        clock.reset_reads();
    }

    fn clock_reads(&self) -> Option<usize> {
        Some(self.harness.clock().expect("a supplied clock").reads())
    }

    fn run(&mut self, command: Command, args: &str) -> Answer {
        let mark = self.harness.store.data().observed.transcript.len();
        let key = match command {
            Command::Bookmark => self.bookmark,
            Command::Unbookmark => self.unbookmark,
        };
        let outcome = invoke_command_on_component(&mut self.harness, key, args, false);
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
    let bookmark = host.observed().callback("command bookmark")?;
    let unbookmark = host.observed().callback("command unbookmark")?;
    Ok(ControlledHost {
        host,
        exports,
        bookmark,
        unbookmark,
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

/// Every recorded removal case clears the newest label of any entry kind.
#[test]
fn unbookmark_selects_latest_nonempty_label() -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    for case in selected(&corpus, |case| {
        case.command == Command::Unbookmark
            && case.failure.is_none()
            && !case.expected.queries.is_empty()
            && case.expected.notifications[0].kind.as_deref() == Some("info")
    }) {
        run_case(&mut host, case);
    }
    host.release();
    Ok(())
}

/// A session without a label anywhere is reported and left alone.
#[test]
fn unbookmark_without_labels_warns() -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    for case in selected(&corpus, |case| {
        case.command == Command::Unbookmark
            && case.failure.is_none()
            && case.expected.notifications[0].kind.as_deref() == Some("warning")
    }) {
        run_case(&mut host, case);
    }
    host.release();
    Ok(())
}

/// A rejected host operation keeps the writes and notifications that came before it.
#[test]
fn bookmark_host_failures_preserve_effect_order() -> Result<(), String> {
    let mut host = controlled()?;
    let corpus = corpus();
    for case in selected(&corpus, |case| case.failure.is_some()) {
        run_case(&mut host, case);
    }
    host.release();
    Ok(())
}

/// One traversal case of the walk test.
struct Walk {
    /// The command it invokes.
    command: Command,
    /// The labels the host holds before the invocation.
    labels: BTreeMap<String, String>,
    /// The argument text it supplies.
    args: String,
    /// The identities it reads a label for, in query order.
    queries: Vec<String>,
    /// The label writes it produces.
    sets: Vec<Set>,
    /// The notification it produces.
    notification: Notification,
}

/// One entries request per invocation, and at most one label query per visited entry.
///
/// The recorded repeated-scan control reads a quadratic number of labels, which the
/// per-entry bound rejects.
#[test]
fn bookmark_walk_is_linear() -> Result<(), String> {
    let mut host = controlled()?;
    for count in [32_usize, 64, 128] {
        for case in walk_cases(count) {
            let reader = run_case(&mut host, &case);
            assert_eq!(
                entry_requests(&reader),
                1,
                "{}: entries requests",
                case.name
            );
        }
    }
    host.release();
    Ok(())
}

/// The three recorded traversals over `count` assistant entries.
fn walk_cases(count: usize) -> Vec<Case> {
    let newest = format!("entry{}", count - 1);
    let visited: Vec<String> = (0..count)
        .rev()
        .map(|index| format!("entry{index}"))
        .collect();
    let info = |message: &str| Notification {
        message: message.to_owned(),
        kind: Some("info".to_owned()),
    };
    vec![
        walk(
            count,
            Walk {
                command: Command::Bookmark,
                labels: BTreeMap::default(),
                args: "topic".to_owned(),
                queries: Vec::new(),
                sets: vec![Set {
                    id: newest.clone(),
                    label: Some("topic".to_owned()),
                }],
                notification: info("Bookmarked as: topic"),
            },
        ),
        walk(
            count,
            Walk {
                command: Command::Unbookmark,
                labels: BTreeMap::default(),
                args: String::new(),
                queries: visited,
                sets: Vec::new(),
                notification: Notification {
                    message: "No bookmarked entry found".to_owned(),
                    kind: Some("warning".to_owned()),
                },
            },
        ),
        walk(
            count,
            Walk {
                command: Command::Unbookmark,
                labels: BTreeMap::from([(newest.clone(), "newest".to_owned())]),
                args: String::new(),
                queries: vec![newest.clone()],
                sets: vec![Set {
                    id: newest,
                    label: None,
                }],
                notification: info("Removed bookmark: newest"),
            },
        ),
    ]
}

/// One traversal case over `count` assistant entries with the supplied walk's state.
fn walk(count: usize, walk: Walk) -> Case {
    Case {
        name: format!("{count} entries"),
        command: walk.command,
        args: walk.args,
        entries: Value::Array(
            (0..count)
                .map(|index| assistant(&format!("entry{index}")))
                .collect(),
        ),
        labels: walk.labels,
        now: 0,
        failure: None,
        expected: Expected {
            sets: walk.sets,
            notifications: vec![walk.notification],
            queries: walk.queries,
            clock_reads: 0,
            error: None,
        },
    }
}

/// The number of times the guest asked for the session entries.
fn entry_requests(reader: &Reader) -> usize {
    reader
        .queries()
        .iter()
        .filter(|query| query.method == "getEntries")
        .count()
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
    for case in &corpus.rows {
        run_case(&mut started, case);
    }
    started.release();
    Ok(())
}

/// A component the test never gives a clock reads the host wall clock.
#[test]
fn bookmark_component_reads_host_clock() -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "bookmark")?;
    let mut harness =
        super::block_on(super::host::Harness::start(&path)).map_err(|error| error.to_string())?;
    harness.observed().readers.control(
        &json!({ "bind": { "replies": [{
            "method": "getEntries",
            "args": [],
            "value": [assistant("assistant")],
        }] } })
        .to_string(),
    );
    harness.store.data().observed.readers.acquire()?;
    let key = harness
        .callback("command bookmark")
        .map_err(|error| error.to_string())?;
    super::example_support::invoke_command_on_component(&mut harness, key, "", false)?;
    let sets = label_writes(&harness.store.data().observed.transcript);
    let label = sets
        .first()
        .and_then(|set| set.label.as_deref())
        .ok_or("a default timestamp label")?;
    let millis: u128 = label
        .strip_prefix("bookmark-")
        .ok_or("a generated label")?
        .parse()
        .map_err(|error| format!("{error}"))?;
    assert!(millis > 0, "the host clock reports a real instant: {label}");
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
fn run_case<H: Host>(host: &mut H, case: &Case) -> Reader {
    let reader = host.supply(supplied(case)).expect("the supplied reader");
    host.reject(case.failure.as_deref());
    host.set_clock(case.now);
    let answer = host.run(case.command, &case.args);
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
    reader
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
    super::example_support::observations(lines)
        .into_iter()
        .filter_map(|observation| match observation {
            Observation::Notify { message, kind } => Some(Notification { message, kind }),
            Observation::Delivery { .. } => None,
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
