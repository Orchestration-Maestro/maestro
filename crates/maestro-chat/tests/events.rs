#![cfg(unix)]
//! Session events the terminal shows, through the stored session and the terminal's effects.
use crate::interactive_mode_support::{Effects, Setup, World, corpus::cases, isolation::isolated};
use maestro_session::SessionManager;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{fs, path::Path, rc::Rc};

/// The session-event cases these tests drive.
const FIXTURES: &str = include_str!("fixtures/interactive_events.json");

/// One recorded stored session.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum Input {
    /// A stored name over a working directory.
    Title {
        /// The stored working directory.
        cwd: String,
        /// The stored name; absent without a name record.
        name: Option<String>,
    },
    /// One stored name field of every admitted kind.
    TitleDomain {
        /// Which name record the session file holds.
        kind: String,
        /// The stored name record.
        name: Value,
    },
}

/// The recorded observation of one case.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// The title written to the terminal.
    Title {
        /// The title text.
        title: String,
    },
    /// The failure a stored name outside the field domain causes.
    Failed {
        /// The failure category.
        #[serde(rename = "errorKind")]
        error_kind: String,
    },
}

/// The titles `effects` holds, without their terminal framing.
fn titles(effects: &Effects) -> Vec<String> {
    let effects = effects.borrow();
    let framed = effects
        .iter()
        .filter_map(|effect| effect.strip_prefix("\u{1b}]0;")?.strip_suffix('\u{7}'));
    framed.map(str::to_owned).collect()
}

/// Points the session of `world` at a stored session over `cwd` whose newest name record
/// holds `name`, or which has no name record.
fn store_name(world: &World, directory: &Path, cwd: &str, name: Option<&Value>) {
    let mut stored = json!({"type": "session", "id": "session"}).to_string() + "\n";
    if let Some(name) = name {
        let record = json!({"type": "session_info", "id": "info", "name": name});
        stored += &(record.to_string() + "\n");
    }
    let path = directory.join("stored-name.jsonl");
    fs::write(&path, stored).unwrap();
    let manager = Rc::clone(world.host.session().session_manager());
    *manager.borrow_mut() = SessionManager::open(path.to_str().unwrap(), None, Some(cwd)).unwrap();
}

#[test]
fn events_title_uses_native_lexical_basename_and_optional_name() {
    const TEST: &str = "events_title_uses_native_lexical_basename_and_optional_name";
    isolated(TEST, async |directory| {
        for (input, expected) in cases(FIXTURES, TEST) {
            let (Input::Title { cwd, name }, Expected::Title { title }) = (input, expected) else {
                panic!("a title case");
            };
            let world = Setup::default().build(directory);
            let stored = name.clone().map(Value::String);
            store_name(&world, directory, &cwd, stored.as_ref());
            let effects = world.effects();
            // Starting binds the stored session, which writes its title.
            world.start().await;
            assert_eq!(
                titles(&effects),
                std::slice::from_ref(&title),
                "{cwd:?} {name:?}"
            );
            if name.is_none() {
                // Naming the session reaches the mode as a session event.
                world.host.session().set_session_name("renamed").unwrap();
                let renamed = title.replacen(" - ", " - renamed - ", 1);
                assert_eq!(titles(&effects), [title, renamed], "{cwd:?}");
            }
        }
    });
}

#[test]
fn events_title_keeps_the_session_name_producers_field_domain() {
    const TEST: &str = "events_title_keeps_the_session_name_producers_field_domain";
    isolated(TEST, async |directory| {
        for (input, expected) in cases(FIXTURES, TEST) {
            let Input::TitleDomain { kind, name } = input else {
                panic!("a stored-name case");
            };
            let world = Setup::default().build(directory);
            let stored = (kind != "missing").then_some(&name);
            store_name(&world, directory, "/work/project", stored);
            let effects = world.effects();
            let outcome = world.mode.init().await;
            match expected {
                Expected::Title { title } => {
                    outcome.unwrap();
                    assert_eq!(titles(&effects), [title], "{kind}");
                }
                Expected::Failed { error_kind } => {
                    let error = outcome.unwrap_err();
                    assert_eq!(error_kind, "invalid-session-name");
                    // The invalid stored name keeps its invalid-data category.
                    let invalid = error.downcast_ref::<std::io::Error>().unwrap();
                    assert_eq!(invalid.kind(), std::io::ErrorKind::InvalidData);
                    assert!(titles(&effects).is_empty(), "{kind}");
                }
            }
        }
    });
}
