//! The frame writer's diagnostic files follow the one agent directory.
#![cfg(all(test, any(unix, windows)))]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use maestro_test_terminal::recording_terminal::RecordingTerminal;
use maestro_tui::{Component, TUI, TerminalImage};

/// The variable that carries the scratch base into the child.
const BASE: &str = "MAESTRO_TEST_BASE";
/// The variable that names the agent directory.
const AGENT_DIR: &str = "MAESTRO_CODING_AGENT_DIR";

/// A component that renders the lines it is given.
struct Lines(RefCell<Vec<String>>);

impl Component for Lines {
    fn render(&self, _width: usize) -> Vec<String> {
        self.0.borrow().clone()
    }
}

/// The child environment for one case: a disposable home, the redraw log switched on and
/// the agent directory set to `configured`, or left unset.
fn environment(base: &str, configured: Option<&str>) -> BTreeMap<&'static str, Option<String>> {
    let home = Path::new(base).join("home").display().to_string();
    let mut environment = BTreeMap::from([
        ("HOME", Some(home.clone())),
        (BASE, Some(base.to_owned())),
        ("MAESTRO_DEBUG_REDRAW", Some("1".to_owned())),
        (AGENT_DIR, configured.map(str::to_owned)),
    ]);
    if cfg!(windows) {
        environment.insert("USERPROFILE", Some(home));
    }
    environment
}

/// Draws a fitting frame, then an oversized one, on a 10 column terminal whose host comes
/// from the native selector pair. The directory is left absent, as a first run finds it.
fn draw_then_overflow(agent_dir: &Path) {
    assert!(
        !agent_dir.exists(),
        "the host starts without a log directory"
    );
    let local = Rc::new(tokio::task::LocalSet::new());
    let (_terminal, runtime) =
        maestro_cli::input::selection::native_terminal(Rc::clone(&local)).unwrap();
    let terminal = RecordingTerminal::new(10, 3);
    let tui = TUI::new(
        terminal.handle(),
        runtime,
        TerminalImage::new(|_| None, || 1),
        None,
    );
    let lines = Rc::new(Lines(RefCell::new(vec!["fits".to_owned()])));
    tui.add_child(lines.clone());
    tui.request_render(false);
    tui.render_pending_frame().unwrap();
    lines.0.replace(vec!["01234567890".to_owned()]);
    tui.request_render(false);
    tui.render_pending_frame().unwrap_err();
}

#[test]
fn crash_report_honours_the_agent_directory() {
    let parent = (!maestro_test_process::selection().is_child())
        .then(|| maestro_test_directory::Scratch::new().unwrap());
    let base = parent.as_ref().map_or_else(
        || std::env::var(BASE).expect("the child carries the scratch base"),
        |scratch| scratch.path().display().to_string(),
    );
    let cases = [true, false];
    let ran = maestro_test_process::run_per_environment(
        "crash_report_honours_the_agent_directory",
        cases,
        |configured| {
            let agent = Path::new(&base).join("agent").display().to_string();
            environment(&base, configured.then_some(agent.as_str()))
        },
    )
    .unwrap();
    for configured in ran.unwrap_or_default() {
        let agent = if configured {
            Path::new(&base).join("agent")
        } else {
            Path::new(&base).join("home").join(".maestro").join("agent")
        };
        draw_then_overflow(&agent);
        let redraw = std::fs::read_to_string(agent.join("maestro-debug.log")).unwrap();
        assert!(redraw.contains("fullRender: first render"), "{redraw}");
        let crash = std::fs::read_to_string(agent.join("maestro-crash.log")).unwrap();
        assert!(crash.contains("Line 0 visible width: 11"), "{crash}");
    }
}
