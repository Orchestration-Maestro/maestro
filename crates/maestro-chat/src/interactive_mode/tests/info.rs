//! Session name, session information, keyboard help and debug capture witnesses.
use super::{World, isolated, isolated_case, started};
use crate::interactive_mode::info::session_info;
use maestro_app::{
    AgentSessionEvent, NewSessionOptions, ReadonlySessionManager, SessionStats,
    agent_session::SessionStatsTokens,
};
use maestro_tui::{Component, Container, EditorComponent, Spacer, Text};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{collections::BTreeMap, io, panic::AssertUnwindSafe, path::Path, rc::Rc};

/// Every recorded command case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_info.json");

/// The recorded operations of `test`, with observations typed later by operation.
pub(super) fn cases<I: DeserializeOwned>(test: &str) -> Vec<(I, Value)> {
    super::cases::<I, Value>(FIXTURES, test)
}

/// Decodes one recorded observation strictly.
pub(super) fn expected<E: DeserializeOwned>(value: Value) -> E {
    serde_json::from_value(value).unwrap()
}

/// The transcript rows of one spacer and one padded text per recorded notice text.
fn notice_rows(texts: &[String], width: usize) -> Vec<String> {
    text_rows(texts, 0, width)
}

/// The transcript rows of one spacer and one text, padded by one column and `padding_y`
/// rows, per recorded text.
pub(super) fn text_rows(texts: &[String], padding_y: usize, width: usize) -> Vec<String> {
    let rows = Container::new();
    for text in texts {
        rows.add_child(Rc::new(Spacer::new(1)));
        rows.add_child(Rc::new(Text::new(text.clone(), 1, padding_y, None)));
    }
    rows.render(width)
}

/// Checks the transcript's recorded children, its recorded texts and each recorded frame.
fn assert_transcript(
    world: &World,
    texts: &[String],
    frames: &BTreeMap<usize, Vec<String>>,
    children: Option<usize>,
) {
    let transcript = &world.mode.chat_container;
    if let Some(children) = children {
        assert_eq!(transcript.children().borrow().len(), children);
    }
    assert_eq!(transcript.render(80), notice_rows(texts, 80));
    for (width, rows) in frames {
        assert_eq!(&transcript.render(*width), rows);
    }
}

/// The recorded name operations.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum NameInput {
    /// The command text over an optional stored name.
    Name {
        /// The stored name before the command.
        name: Option<String>,
        /// The submitted command text.
        text: String,
    },
    /// `/name` over one stored name record of the given kind.
    NameRecord {
        /// The record's name kind.
        kind: String,
        /// The record's name value.
        value: Value,
    },
    /// `/name selected` with a name listener making the given edit.
    NameListener {
        /// The listener's edit.
        edit: String,
    },
}

/// The observations of a name command.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NameExpected {
    /// The names recorded by the command.
    writes: Vec<String>,
    /// The warning shown, if any.
    warning: Option<String>,
    /// The texts appended to the transcript.
    text: Vec<String>,
    /// The transcript rows at each width.
    frames: BTreeMap<usize, Vec<String>>,
    /// The transcript's children.
    children: usize,
}

/// The observations of `/name` over one stored record.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NameRecordExpected {
    /// The failure kind, when reading the name fails.
    error: Option<String>,
    /// The texts appended to the transcript.
    text: Vec<String>,
}

/// The observations of a name listener's edit.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct NameListenerExpected {
    /// The texts appended to the transcript.
    text: Vec<String>,
    /// Whether the listener stopped the command.
    aborted: bool,
    /// The stored name afterwards.
    stored_name: Option<String>,
    /// The editor text afterwards.
    editor_text: String,
}

/// The session's recorded name values, oldest first.
fn recorded_names(world: &World) -> Vec<String> {
    let session = world.host.session();
    let manager = session.session_manager().borrow();
    manager
        .get_entries()
        .into_iter()
        .filter(|entry| entry["type"] == "session_info")
        .map(|entry| entry["name"].as_str().unwrap().to_owned())
        .collect()
}

/// Replaces the session's history by one header and one name record holding `name`.
fn install_name_record(world: &World, directory: &Path, name: Option<&Value>) {
    let mut record = serde_json::json!({
        "type": "session_info", "id": "a1b2c3d4", "parentId": null,
        "timestamp": "2026-01-02T03:04:05.006Z",
    });
    if let Some(name) = name {
        record["name"] = name.clone();
    }
    let header = serde_json::json!({
        "type": "session", "version": 3, "id": "named",
        "timestamp": "2026-01-02T03:04:05.006Z", "cwd": directory.to_str().unwrap(),
    });
    let file = directory.join("named.jsonl");
    std::fs::write(&file, format!("{header}\n{record}\n")).unwrap();
    let session = world.host.session();
    let mut manager = session.session_manager().borrow_mut();
    manager.set_session_file(file.to_str().unwrap()).unwrap();
}

/// Subscribes a name listener that edits the editor, then the stored name as `edit` says.
fn listen_for_name(world: &World, edit: &str) {
    let session = world.host.session();
    let weak = Rc::downgrade(&session);
    let editor = Rc::clone(&world.mode.editor);
    let edit = edit.to_owned();
    drop(session.subscribe(Rc::new(move |event| {
        let AgentSessionEvent::SessionInfoChanged { .. } = event else {
            return;
        };
        editor.set_text("edited later");
        let replacement = match edit.as_str() {
            "mutate" => "listener",
            "replace" => "replacement",
            "add" => "added",
            "delete" => "",
            _ => panic!("the listener stops the command"),
        };
        let session = weak.upgrade().unwrap();
        let mut manager = session.session_manager().borrow_mut();
        manager.append_session_info(replacement).unwrap();
    })));
}

/// Runs one recorded name case in a fresh mode.
async fn name_case(directory: &Path, input: NameInput, observed: Value) {
    let world = started(directory).await;
    match input {
        NameInput::Name { name, text } => {
            let expected: NameExpected = expected(observed);
            if let Some(name) = &name {
                let session = world.host.session();
                let mut manager = session.session_manager().borrow_mut();
                manager.append_session_info(name).unwrap();
            }
            let before = recorded_names(&world).len();
            world.mode.handle_name_command(&text).unwrap();
            assert_eq!(recorded_names(&world)[before..], expected.writes);
            if let Some(warning) = &expected.warning {
                assert!(expected.text[0].contains(&format!("Warning: {warning}")));
            }
            assert_transcript(
                &world,
                &expected.text,
                &expected.frames,
                Some(expected.children),
            );
        }
        NameInput::NameRecord { kind, value } => {
            let expected: NameRecordExpected = expected(observed);
            install_name_record(&world, directory, (kind != "missing").then_some(&value));
            let result = world.mode.handle_name_command("/name");
            let error = result
                .err()
                .map(|error| error.downcast_ref::<io::Error>().unwrap().kind());
            assert_eq!(error.is_some(), expected.error.is_some());
            if error.is_some() {
                assert_eq!(error, Some(io::ErrorKind::InvalidData));
            }
            assert_eq!(
                world.mode.chat_container.render(80),
                notice_rows(&expected.text, 80)
            );
        }
        NameInput::NameListener { edit } => {
            let expected: NameListenerExpected = expected(observed);
            listen_for_name(&world, &edit);
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                world.mode.handle_name_command("/name selected")
            }));
            assert_eq!(outcome.is_err(), expected.aborted);
            assert_eq!(
                world.host.session().session_name().unwrap(),
                expected.stored_name
            );
            assert_eq!(world.mode.editor.get_text(), expected.editor_text);
            assert_eq!(
                world.mode.chat_container.render(80),
                notice_rows(&expected.text, 80)
            );
        }
    }
}

#[test]
fn name_command_reads_sets_and_trims_selected_text() {
    isolated(
        "name_command_reads_sets_and_trims_selected_text",
        async |directory| {
            for (input, observed) in cases("name_command_reads_sets_and_trims_selected_text") {
                name_case(directory, input, observed).await;
            }
        },
    );
}

/// Recorded session statistics.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StatsRecord {
    /// The session identity.
    session_id: String,
    /// User messages.
    user_messages: usize,
    /// Assistant messages.
    assistant_messages: usize,
    /// Requested tool calls.
    tool_calls: usize,
    /// Tool results.
    tool_results: usize,
    /// Every message.
    total_messages: usize,
    /// Token totals.
    tokens: TokensRecord,
    /// Total cost.
    cost: f64,
    /// The session file; absent and null both mean in memory.
    #[serde(default)]
    session_file: Option<String>,
}

/// Recorded token totals.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TokensRecord {
    /// Input tokens.
    input: f64,
    /// Output tokens.
    output: f64,
    /// Cache-read tokens.
    cache_read: f64,
    /// Cache-write tokens.
    cache_write: f64,
    /// Every token.
    total: f64,
}

impl From<StatsRecord> for SessionStats {
    fn from(record: StatsRecord) -> Self {
        let tokens = record.tokens;
        Self {
            session_file: record.session_file,
            session_id: record.session_id,
            user_messages: record.user_messages,
            assistant_messages: record.assistant_messages,
            tool_calls: record.tool_calls,
            tool_results: record.tool_results,
            total_messages: record.total_messages,
            tokens: SessionStatsTokens {
                input: tokens.input,
                output: tokens.output,
                cache_read: tokens.cache_read,
                cache_write: tokens.cache_write,
                total: tokens.total,
            },
            cost: record.cost,
            context_usage: None,
        }
    }
}

/// The recorded session information operations.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum SessionInput {
    /// The information for supplied statistics and name.
    Session {
        /// The session name.
        name: Option<String>,
        /// The statistics.
        stats: StatsRecord,
    },
    /// The command over a session holding these messages.
    SessionProducer {
        /// The session's messages.
        messages: Vec<Value>,
    },
    /// The information for supplied statistics in one locale.
    Locale {
        /// The locale environment.
        locale: String,
        /// The statistics.
        stats: StatsRecord,
    },
}

/// The information text and its transcript rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionExpected {
    /// The statistics the session produced, for the command over messages.
    stats: Option<StatsRecord>,
    /// The information text.
    text: String,
    /// The transcript rows at each width.
    frames: BTreeMap<usize, Vec<String>>,
    /// The transcript's children, for the information of supplied statistics.
    children: Option<usize>,
}

/// The information text alone.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocaleExpected {
    /// The information text.
    text: String,
}

/// Adds the recorded messages to the session's history.
pub(super) fn seed_messages(world: &World, messages: Vec<Value>) {
    let session = world.host.session();
    let history = session.messages();
    let mut history = history.write().unwrap();
    for message in messages {
        history.push(serde_json::from_value(message).unwrap());
    }
}

/// Runs the command over the recorded messages and checks the produced statistics.
fn producer_case(world: &World, messages: Vec<Value>, stats: StatsRecord) {
    let session = world.host.session();
    session
        .session_manager()
        .borrow_mut()
        .new_session(NewSessionOptions {
            id: Some(stats.session_id.clone()),
            ..NewSessionOptions::default()
        });
    seed_messages(world, messages);
    world.mode.handle_session_command().unwrap();
    let produced = SessionStats {
        context_usage: None,
        ..session.get_session_stats().unwrap()
    };
    assert_eq!(produced, SessionStats::from(stats));
}

#[test]
fn session_info_keeps_counts_cache_cost_and_file_branches() {
    let test = "session_info_keeps_counts_cache_cost_and_file_branches";
    isolated_case(
        test,
        "child",
        &[("LC_ALL", "en_US.UTF-8")],
        async |directory| {
            let world = started(directory).await;
            let theme = world.themes.theme().get().unwrap();
            for (input, observed) in cases(test) {
                let SessionExpected {
                    stats,
                    text,
                    frames,
                    children,
                } = expected(observed);
                world.mode.chat_container.clear();
                match input {
                    SessionInput::Session { name, stats } => {
                        let info = session_info(&theme, &stats.into(), name.as_deref()).unwrap();
                        assert_eq!(info, text);
                        world.mode.add_text(info, 0);
                    }
                    SessionInput::SessionProducer { messages } => {
                        producer_case(&world, messages, stats.unwrap());
                    }
                    SessionInput::Locale { .. } => panic!("a statistics case"),
                }
                assert_transcript(&world, &[text], &frames, children);
            }
        },
    );
}

/// Checks the information texts recorded for the `selected` locale.
async fn locale_texts(directory: &Path, selected: &str) {
    let world = started(directory).await;
    let theme = world.themes.theme().get().unwrap();
    let recorded: Vec<_> = cases("session_info_uses_selected_locale_and_fixed_cost")
        .into_iter()
        .filter_map(|(input, observed)| {
            let SessionInput::Locale { locale, stats } = input else {
                panic!("a locale case");
            };
            let observed: LocaleExpected = expected(observed);
            (locale == selected).then_some((stats, observed.text))
        })
        .collect();
    assert_eq!(recorded.len(), 6);
    for (stats, text) in recorded {
        assert_eq!(session_info(&theme, &stats.into(), None).unwrap(), text);
    }
}

#[test]
fn session_info_uses_selected_locale_and_fixed_cost() {
    for selected in ["en_US.UTF-8", "de_DE.UTF-8"] {
        isolated_case(
            "session_info_uses_selected_locale_and_fixed_cost",
            selected,
            &[("LC_ALL", selected)],
            async |directory| (locale_texts(directory, selected)).await,
        );
    }
}
