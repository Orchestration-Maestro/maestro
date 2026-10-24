//! Session name, session information, keyboard help and debug capture witnesses.
use super::{World, isolated, started};
use maestro_app::{AgentSessionEvent, ReadonlySessionManager};
use maestro_tui::{Component, Container, EditorComponent, Spacer, Text};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{collections::BTreeMap, io, panic::AssertUnwindSafe, path::Path, rc::Rc};

/// Every recorded command case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_info.json");

/// One recorded case, decoded into its witness's own records.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The witness the case belongs to.
    test: String,
    /// The recorded operation.
    input: Value,
    /// The recorded observation.
    expected: Value,
}

/// The recorded operations and observations of `test`.
fn cases<I: DeserializeOwned>(test: &str) -> Vec<(I, Value)> {
    let all: Vec<Case> = serde_json::from_str(FIXTURES).unwrap();
    let cases: Vec<_> = all
        .into_iter()
        .filter(|case| case.test == test)
        .map(|case| (serde_json::from_value(case.input).unwrap(), case.expected))
        .collect();
    assert!(!cases.is_empty());
    cases
}

/// Decodes one recorded observation strictly.
fn expected<E: DeserializeOwned>(value: Value) -> E {
    serde_json::from_value(value).unwrap()
}

/// The transcript rows of one spacer and one padded text per recorded notice text.
fn notice_rows(texts: &[String], width: usize) -> Vec<String> {
    let rows = Container::new();
    for text in texts {
        rows.add_child(Rc::new(Spacer::new(1)));
        rows.add_child(Rc::new(Text::new(text.clone(), 1, 0, None)));
    }
    rows.render(width)
}

/// Checks the transcript's children, its recorded texts and each recorded frame.
fn assert_transcript(
    world: &World,
    texts: &[String],
    frames: &BTreeMap<usize, Vec<String>>,
    children: usize,
) {
    let transcript = &world.mode.chat_container;
    assert_eq!(transcript.children().borrow().len(), children);
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
fn name_case(directory: &Path, input: NameInput, observed: Value) {
    let world = started(directory);
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
            assert_transcript(&world, &expected.text, &expected.frames, expected.children);
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
        |directory| {
            for (input, observed) in cases("name_command_reads_sets_and_trims_selected_text") {
                name_case(directory, input, observed);
            }
        },
    );
}
