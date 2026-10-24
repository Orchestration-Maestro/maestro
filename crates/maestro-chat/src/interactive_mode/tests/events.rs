//! The session-event subscriber, the terminal title and the thinking-level refresh.
use super::{Setup, World, cases, isolated, started};
use maestro_app::configuration::config::APP_TITLE;
use maestro_app::{AgentSessionEvent, ReadonlySessionManager, SessionManager};
use maestro_theme::ThemeColor;
use maestro_tui::Component;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, fs, path::Path, rc::Rc};

/// The session-event cases these witnesses drive.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_events.json");

/// The children `init` adds: seven containers and the footer.
const INITIALIZED_CHILDREN: usize = 8;

/// One recorded session event or the live state it reads.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum Input {
    /// An event before or after initialization.
    Init {
        /// Whether the mode was initialized when the event arrived.
        initialized: bool,
    },
    /// The subscription through a real producer and its removal operation.
    Subscribe,
    /// A stored name over a working directory.
    Title {
        /// The stored working directory.
        cwd: String,
        /// The stored name; absent clears it.
        name: Option<String>,
    },
    /// One stored name field of every admitted kind.
    TitleDomain {
        /// Which name record the session file holds.
        kind: String,
        /// The stored name record.
        name: Value,
    },
    /// A thinking-level change while the editor holds ordinary or shell input.
    Level {
        /// Whether the editor holds a shell command.
        bash: bool,
    },
}

/// The recorded empty presentation after a metadata change.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Unchanged {
    /// The transcript rows.
    chat: Vec<String>,
    /// The status rows.
    status: Vec<String>,
    /// The pending-message rows.
    #[serde(rename = "pendingRows")]
    pending_rows: Vec<String>,
    /// The unresolved tool calls.
    pending: Vec<String>,
    /// The editor history.
    history: Vec<String>,
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
    /// No transcript, status or history change.
    Unchanged(Unchanged),
}

/// The collected emulated terminal effects.
type Effects = Rc<RefCell<Vec<String>>>;

/// Collects the effects the terminal is asked to emulate from now on; a title is emulated
/// rather than recorded as a frame write.
fn tap(world: &World) -> Effects {
    let effects: Effects = Rc::default();
    let collect = Rc::clone(&effects);
    world
        .terminal
        .set_tap(move |effect| collect.borrow_mut().push(effect.to_owned()));
    effects
}

/// The titles `effects` holds, with their terminal framing removed.
fn titles(effects: &Effects) -> Vec<String> {
    effects
        .borrow()
        .iter()
        .filter_map(|effect| {
            effect
                .strip_prefix("\u{1b}]0;")
                .and_then(|title| title.strip_suffix('\u{7}'))
                .map(str::to_owned)
        })
        .collect()
}

/// A session-metadata event whose own payload names no session.
fn session_info() -> AgentSessionEvent {
    AgentSessionEvent::SessionInfoChanged { name: None }
}

/// Replaces the session's manager with one whose newest name record holds `name`, absent when
/// the session file has no name record, over `cwd`.
fn install_name(world: &World, directory: &Path, cwd: &str, name: Option<&Value>) {
    let record = match name {
        Some(name) => format!("{{\"type\":\"session_info\",\"id\":\"info\",\"name\":{name}}}\n"),
        None => String::new(),
    };
    let path = directory.join("stored-name.jsonl");
    fs::write(
        &path,
        format!("{{\"type\":\"session\",\"id\":\"session\"}}\n{record}"),
    )
    .unwrap();
    let manager = Rc::clone(world.host.session().session_manager());
    *manager.borrow_mut() = SessionManager::open(path.to_str().unwrap(), None, Some(cwd)).unwrap();
}

/// Asserts the recorded presentation: a metadata change adds no row, call or history entry.
fn unchanged(world: &World, expected: &Unchanged) {
    assert_eq!(world.mode.chat_container.render(100), expected.chat);
    assert_eq!(world.mode.status_container.render(100), expected.status);
    assert_eq!(
        world.mode.pending_messages_container.render(100),
        expected.pending_rows
    );
    let pending: Vec<String> = world.mode.pending_tools.borrow().keys().cloned().collect();
    assert_eq!(pending, expected.pending);
    assert_eq!(super::transcript::history(world), expected.history);
}

/// The editor's painted top border at width 20.
fn border(world: &World) -> String {
    world.mode.editor.render(20)[0].clone()
}

/// The top border painted by one of the published theme's roles.
fn painted(world: &World, role: &ThemeColor) -> String {
    world
        .themes
        .theme()
        .get()
        .unwrap()
        .fg(role, "─")
        .unwrap()
        .repeat(20)
}

/// Drives one event before or after initialization and a second one after it.
fn initialization(world: &World, initialized: bool) {
    if initialized {
        world.mode.init().unwrap();
        world.runtime.settle().unwrap();
    }
    let effects = tap(world);
    world.mode.handle_event(&session_info()).unwrap();
    world.runtime.settle().unwrap();
    assert!(world.mode.is_initialized.get());
    assert_eq!(
        world.mode.ui.children().borrow().len(),
        INITIALIZED_CHILDREN
    );
    // The uninitialized event initializes first, which writes the title before dispatch does.
    let after_first = if initialized { 1 } else { 2 };
    assert_eq!(titles(&effects).len(), after_first);
    world.mode.handle_event(&session_info()).unwrap();
    world.runtime.settle().unwrap();
    assert_eq!(titles(&effects).len(), after_first + 1);
    assert_eq!(
        world.mode.ui.children().borrow().len(),
        INITIALIZED_CHILDREN
    );
}

/// Drives the subscription, a second initialization, the removal operation and the release.
fn subscription(world: World, unchanged_rows: &Unchanged) {
    let session = world.host.session();
    let basename = maestro_path::basename(session.session_manager().borrow().get_cwd(), None);
    world.mode.init().unwrap();
    world.mode.init().unwrap();
    world.runtime.settle().unwrap();
    let effects = tap(&world);
    // A second initialization adds no listener: one event writes one title.
    session.set_session_name("named").unwrap();
    assert_eq!(
        titles(&effects),
        vec![format!("{APP_TITLE} - named - {basename}")]
    );
    unchanged(&world, unchanged_rows);
    let removal = world.mode.unsubscribe.borrow_mut().take().unwrap();
    // The listener keeps no strong reference: the mode is released while it is still
    // subscribed, and the retained removal operation still detaches it afterwards.
    let released = Rc::downgrade(&world.mode);
    drop(world.mode);
    assert!(released.upgrade().is_none());
    removal();
    session.set_session_name("other").unwrap();
    assert_eq!(titles(&effects).len(), 1);
}

#[test]
fn events_subscription_initializes_once_and_releases() {
    let test = "events_subscription_initializes_once_and_releases";
    isolated(test, |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let Expected::Unchanged(unchanged_rows) = expected else {
                panic!("a metadata case");
            };
            let world = Setup::default().build(directory);
            match input {
                Input::Init { initialized } => {
                    initialization(&world, initialized);
                    unchanged(&world, &unchanged_rows);
                }
                Input::Subscribe => subscription(world, &unchanged_rows),
                _ => panic!("a subscription case"),
            }
        }
    });
}

#[test]
fn events_title_uses_native_lexical_basename_and_optional_name() {
    let test = "events_title_uses_native_lexical_basename_and_optional_name";
    isolated(test, |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let Input::Title { cwd, name } = input else {
                panic!("a title case");
            };
            let Expected::Title { title } = expected else {
                panic!("a written title");
            };
            let world = started(directory);
            let stored = name.map(Value::String);
            install_name(&world, directory, &cwd, stored.as_ref());
            let effects = tap(&world);
            world.mode.handle_event(&session_info()).unwrap();
            world.runtime.settle().unwrap();
            assert_eq!(titles(&effects), vec![title]);
        }
    });
}

#[test]
fn events_title_keeps_the_session_name_producers_field_domain() {
    let test = "events_title_keeps_the_session_name_producers_field_domain";
    isolated(test, |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let Input::TitleDomain { kind, name } = input else {
                panic!("a stored-name case");
            };
            let world = started(directory);
            let stored = (kind != "missing").then_some(&name);
            install_name(&world, directory, "/work/project", stored);
            let effects = tap(&world);
            let outcome = world.mode.handle_event(&session_info());
            world.runtime.settle().unwrap();
            match expected {
                Expected::Title { title } => {
                    outcome.unwrap();
                    assert_eq!(titles(&effects), vec![title]);
                }
                Expected::Failed { error_kind } => {
                    let error = outcome.unwrap_err();
                    assert_eq!(error_kind, "invalid-session-name");
                    // The invalid stored name keeps its invalid-data category.
                    let invalid = error.downcast_ref::<std::io::Error>().unwrap();
                    assert_eq!(invalid.kind(), std::io::ErrorKind::InvalidData);
                    assert!(titles(&effects).is_empty());
                }
                Expected::Unchanged(_) => panic!("a stored-name observation"),
            }
        }
    });
}

#[test]
fn events_thinking_level_refreshes_footer_and_border() {
    let test = "events_thinking_level_refreshes_footer_and_border";
    isolated(test, |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let Input::Level { bash } = input else {
                panic!("a thinking-level case");
            };
            let Expected::Unchanged(unchanged_rows) = expected else {
                panic!("a metadata case");
            };
            let world = started(directory);
            let session = world.host.session();
            session.model().write().unwrap().reasoning = true;
            world.mode.is_bash_mode.set(bash);
            session
                .set_thinking_level(serde_json::from_value(json!("high")).unwrap())
                .unwrap();
            world.runtime.settle().unwrap();
            let (selected, sibling) = if bash {
                (ThemeColor::BashMode, ThemeColor::ThinkingHigh)
            } else {
                (ThemeColor::ThinkingHigh, ThemeColor::BashMode)
            };
            assert_eq!(border(&world), painted(&world, &selected));
            assert_ne!(border(&world), painted(&world, &sibling));
            // A stale payload does not repaint the border: the session's live level does.
            world
                .mode
                .handle_event(&AgentSessionEvent::ThinkingLevelChanged {
                    level: serde_json::from_value(json!("off")).unwrap(),
                })
                .unwrap();
            world.runtime.settle().unwrap();
            assert_eq!(border(&world), painted(&world, &selected));
            unchanged(&world, &unchanged_rows);
        }
    });
}
