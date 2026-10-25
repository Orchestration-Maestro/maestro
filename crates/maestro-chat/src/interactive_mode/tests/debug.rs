//! Debug capture witnesses: the log file, its failure, the session it reads and the
//! global debug key.
use super::info::{cases, expected, seed_messages, text_rows};
use super::{World, isolated, started};
use crate::components::session_selector::list::tests::session_picker::manual_runtime::FileEffect;
use maestro_app::configuration::config::{get_agent_dir, paths::get_debug_log_path};
use maestro_tui::tui::TuiRuntime;
use maestro_tui::{Component, EditorComponent};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, io, path::PathBuf, rc::Rc};

/// The toolkit's global debug key, as a terminal reports Shift+Ctrl+D.
const DEBUG_KEY: &str = "\x1b[100;6u";

/// The recorded debug operations.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum DebugInput {
    /// A capture of these rendered lines and session messages.
    Debug {
        /// The lines the presentation renders.
        rendered: Vec<String>,
        /// The session's messages.
        messages: Vec<Value>,
    },
    /// A capture whose log file cannot be written.
    DebugFailure {
        /// Why the file cannot be written.
        kind: String,
    },
}

/// The observations of a capture.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DebugExpected {
    /// The log text, with `<time>` for the capture time.
    log: String,
    /// The success notice, with `<debug-path>` for the log path.
    notice: String,
    /// The transcript's children.
    children: usize,
}

/// The observations of a failed capture.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DebugFailureExpected {
    /// The native error code.
    code: String,
    /// The transcript's children.
    children: usize,
}

/// A component rendering fixed lines, remembering the width of each render.
struct Lines {
    /// The lines.
    rows: Vec<String>,
    /// The widths asked for, in order.
    widths: RefCell<Vec<usize>>,
}

impl Component for Lines {
    fn render(&self, width: usize) -> Vec<String> {
        self.widths.borrow_mut().push(width);
        self.rows.clone()
    }

    fn invalidate(&self) {}
}

/// The configured debug log path, inside the disposable agent directory.
fn debug_path(directory: &std::path::Path) -> String {
    let path = get_debug_log_path(&get_agent_dir().unwrap());
    assert!(path.starts_with(directory.to_str().unwrap()));
    path
}

/// The recorded custom message as authored: its details put "2" before "1" and held
/// negative zero, which recorded JSON text cannot keep.
fn authored(mut message: Value) -> Value {
    if let Some(details) = message.get_mut("details") {
        let recorded = details.take();
        *details = json!({ "2": recorded["2"], "1": recorded["1"], "z": -0.0, "a": recorded["a"] });
    }
    message
}

/// Runs one recorded capture in a fresh mode.
async fn debug_case(
    directory: &std::path::Path,
    rendered: Vec<String>,
    messages: Vec<Value>,
    observed: Value,
) {
    let expected: DebugExpected = expected(observed);
    let world = started(directory).await;
    world.terminal.resize(40, 12);
    let lines = Rc::new(Lines {
        rows: rendered,
        widths: RefCell::default(),
    });
    world.mode.ui.clear();
    world.mode.ui.add_child(lines.clone());
    seed_messages(&world, messages.into_iter().map(authored).collect());
    world.mode.handle_debug_command().unwrap();
    assert_eq!(*lines.widths.borrow(), [40]);
    let path = debug_path(directory);
    let log = expected
        .log
        .replace("<time>", &world.runtime.log_context().iso_time);
    assert_eq!(
        world.runtime.files(),
        [FileEffect::Write(PathBuf::from(&path), log)]
    );
    let notice = expected.notice.replace("<debug-path>", &path);
    let transcript = &world.mode.chat_container;
    assert_eq!(transcript.children().borrow().len(), expected.children);
    assert_eq!(transcript.render(80), text_rows(&[notice], 1, 80));
    assert_eq!(world.runtime.pending(), 1);
}

#[test]
fn debug_log_keeps_dimensions_widths_escaping_and_message_order() {
    let test = "debug_log_keeps_dimensions_widths_escaping_and_message_order";
    isolated(test, async |directory| {
        for (input, observed) in cases(test) {
            let DebugInput::Debug { rendered, messages } = input else {
                panic!("a capture case");
            };
            debug_case(directory, rendered, messages, observed).await;
        }
    });
}

/// The error kind of a recorded native error code.
fn error_kind(code: &str) -> io::ErrorKind {
    match code {
        "EISDIR" => io::ErrorKind::IsADirectory,
        _ => panic!("a recorded error code"),
    }
}

#[test]
fn debug_write_failure_has_no_success_notice() {
    let test = "debug_write_failure_has_no_success_notice";
    isolated(test, async |directory| {
        let [(DebugInput::DebugFailure { kind }, observed)] =
            <[_; 1]>::try_from(cases(test)).ok().unwrap()
        else {
            panic!("one failure case");
        };
        assert_eq!(kind, "file-path-is-directory");
        let expected: DebugFailureExpected = expected(observed);
        let world = started(directory).await;
        world
            .runtime
            .set_write_error(Some(error_kind(&expected.code)));
        let error = world.mode.handle_debug_command().unwrap_err();
        let error = error.downcast_ref::<io::Error>().unwrap();
        assert_eq!(error.kind(), error_kind(&expected.code));
        let transcript = &world.mode.chat_container;
        assert_eq!(transcript.children().borrow().len(), expected.children);
        assert!(world.runtime.files().is_empty());
        assert_eq!(world.runtime.pending(), 0);
    });
}

/// The text of the one log write the host received.
pub(super) fn only_log(world: &World) -> String {
    let files = world.runtime.files();
    let [FileEffect::Write(_, log)] = files.as_slice() else {
        panic!("one log write");
    };
    log.clone()
}

/// The write effects of the debug log so far.
fn writes(world: &World) -> usize {
    world.runtime.files().len()
}

#[test]
fn global_debug_runs_outside_editor_focus_without_retaining_mode() {
    isolated(
        "global_debug_runs_outside_editor_focus_without_retaining_mode",
        async |directory| {
            let world = started(directory).await;
            world.mode.ui.set_focus(None);
            world.terminal.send_input(DEBUG_KEY);
            assert_eq!(writes(&world), 1);
            assert_eq!(world.mode.chat_container.children().borrow().len(), 2);
            assert_eq!(world.mode.editor.get_text(), "");
            world
                .runtime
                .set_write_error(Some(io::ErrorKind::PermissionDenied));
            world.terminal.send_input(DEBUG_KEY);
            assert_eq!(world.mode.chat_container.children().borrow().len(), 2);
            let errors = world.runtime.drain_futures();
            let [error] = errors.as_slice() else {
                panic!("one reported failure");
            };
            let error = error.downcast_ref::<io::Error>().unwrap();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            world.runtime.set_write_error(None);
            let World {
                mode,
                terminal,
                runtime,
                ..
            } = world;
            let ui = mode.ui.clone();
            let editor = Rc::clone(&mode.editor);
            let weak = Rc::downgrade(&mode);
            drop(mode);
            assert!(weak.upgrade().is_none());
            terminal.send_input(DEBUG_KEY);
            assert_eq!(runtime.files().len(), 1);
            assert!(runtime.drain_futures().is_empty());
            drop((ui, editor));
        },
    );
}

/// The default switch options, whose record the application root does not name.
fn defaults<T: Default>() -> T {
    T::default()
}

/// Replaces the runtime's session by a stored history named `Replacement`.
async fn replace_session(world: &World, directory: &std::path::Path) -> String {
    let header = json!({
        "type": "session", "version": 3, "id": "replacement-session",
        "timestamp": "2026-01-02T03:04:05.006Z", "cwd": directory.to_str().unwrap(),
    });
    let name = json!({
        "type": "session_info", "id": "a1b2c3d4", "parentId": null,
        "timestamp": "2026-01-02T03:04:05.006Z", "name": "Replacement",
    });
    let file = directory.join("replacement.jsonl");
    std::fs::write(&file, format!("{header}\n{name}\n")).unwrap();
    let path = file.to_str().unwrap().to_owned();
    let switch = world.host.switch_session(&path, None, defaults());
    let result = switch.await;
    assert!(!result.unwrap().cancelled);
    path
}

#[test]
fn debug_uses_current_session_and_pre_notice_frame() {
    isolated(
        "debug_uses_current_session_and_pre_notice_frame",
        async |directory| {
            let world = started(directory).await;
            let first = world.host.session();
            let path = replace_session(&world, directory).await;
            assert!(!Rc::ptr_eq(&first, &world.host.session()));
            let message =
                json!({"role": "user", "content": "replacement question", "timestamp": 5});
            seed_messages(&world, vec![message]);
            world.mode.handle_name_command("/name").unwrap();
            world.mode.handle_session_command().unwrap();
            let transcript = world.mode.chat_container.render(200).join("\n");
            assert!(transcript.contains("Session name: Replacement"));
            assert!(transcript.contains(&format!("File:\x1b[39m {path}")));
            assert!(transcript.contains("ID:\x1b[39m replacement-session"));
            assert!(transcript.contains("User:\x1b[39m 1"));
            world.runtime.settle().unwrap();
            world.terminal.clear_writes();
            world.mode.handle_debug_command().unwrap();
            let log = only_log(&world);
            assert!(log.contains("Session name: Replacement"));
            assert!(log.contains("replacement question"));
            assert!(!log.contains("Debug log written"));
            assert!(world.terminal.writes().is_empty());
            world.runtime.settle().unwrap();
            assert!(
                world
                    .terminal
                    .writes()
                    .concat()
                    .contains("Debug log written")
            );
        },
    );
}
