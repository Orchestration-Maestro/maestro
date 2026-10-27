//! Presenter harness shared by the selector and configuration command tests.

use maestro_app::presentation_data::input_selectors::{ConfigSelectorOptions, PresentedSelector};
use maestro_app::{ResolvedPaths, Settings, SettingsManager};
use maestro_cli::input::selector::SelectorHost;
use maestro_test_terminal::manual_runtime::ManualRuntime;
use maestro_test_terminal::recording_terminal::RecordingTerminal;
use maestro_theme::{
    ThemeDirectories, ThemeReloadTimer, ThemeState, ThemeWatchOperations, ThemeWatcher,
};
use maestro_tui::tui::ComponentHandle;
use maestro_tui::{Component, Container, FocusFlag, Focusable, TUI, TerminalImage};
use std::cell::RefCell;
use std::io;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

/// A component that records the focus state of every frame it renders.
struct Probe {
    /// The flag the frame writer sets when focus moves here.
    flag: FocusFlag,
    /// Frames and their focus state, in render order.
    log: Rc<RefCell<Vec<String>>>,
}

impl Component for Probe {
    fn render(&self, _width: usize) -> Vec<String> {
        let count = {
            let mut log = self.log.borrow_mut();
            log.push(format!("render:{}", self.flag.get()));
            log.len()
        };
        vec![format!("frame {count}")]
    }

    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}

impl Focusable for Probe {
    fn focus_flag(&self) -> &FocusFlag {
        &self.flag
    }
}

/// One presented view: a containing component and a separately focused probe.
pub fn view(log: &Rc<RefCell<Vec<String>>>) -> PresentedSelector {
    let probe = Rc::new(Probe {
        flag: FocusFlag::default(),
        log: Rc::clone(log),
    });
    let container = Container::new();
    container.add_child(probe.clone());
    let component: ComponentHandle = Rc::new(container);
    let focus: ComponentHandle = probe;
    PresentedSelector { component, focus }
}

/// The frame writer effects and clock one presented selector runs on.
#[derive(Clone)]
pub struct Harness {
    /// The terminal every frame is written to.
    pub terminal: RecordingTerminal,
    /// The clock and deferred work of the frame writer.
    pub runtime: ManualRuntime,
}

impl Harness {
    /// A writer of 80 by 24 cells at time zero.
    pub fn new() -> Self {
        Self {
            terminal: RecordingTerminal::new(80, 24),
            runtime: ManualRuntime::new(),
        }
    }

    /// The supplied host over this harness and the given exit effect.
    pub fn host(&self, exit: Rc<dyn Fn(i32)>) -> SelectorHost {
        let create_ui = self.create_ui();
        SelectorHost {
            create_ui: Rc::new(move || create_ui(false)),
            exit,
        }
    }

    /// A frame writer factory over this harness, taking the hardware-cursor choice.
    pub fn create_ui(&self) -> Rc<dyn Fn(bool) -> io::Result<TUI>> {
        let (terminal, runtime) = (self.terminal.clone(), self.runtime.clone());
        Rc::new(move |cursor| {
            Ok(TUI::new(
                terminal.handle(),
                runtime.handle(),
                TerminalImage::default(),
                Some(cursor),
            ))
        })
    }

    /// Run every frame the writer scheduled.
    pub fn frames(&self) {
        let _ = self.runtime.advance_to(Duration::from_millis(50));
    }
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}

/// Watch effects that record their creation and close.
pub struct RecordingWatch {
    /// Effects in the order requested.
    pub log: Rc<RefCell<Vec<String>>>,
}

impl ThemeWatchOperations for RecordingWatch {
    fn watch(
        &self,
        _path: &str,
        _listener: Rc<dyn Fn(Option<String>)>,
        _on_error: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn ThemeWatcher>> {
        self.log.borrow_mut().push("watch".to_owned());
        Ok(Box::new(ClosingWatch {
            log: Rc::clone(&self.log),
        }))
    }

    fn schedule(
        &self,
        _delay: Duration,
        _callback: Box<dyn FnOnce()>,
    ) -> Box<dyn ThemeReloadTimer> {
        Box::new(IdleTimer)
    }
}

/// The watch handle recording its own close.
pub struct ClosingWatch {
    /// Effects in the order requested.
    pub log: Rc<RefCell<Vec<String>>>,
}

impl ThemeWatcher for ClosingWatch {
    fn close(&mut self) -> io::Result<()> {
        self.log.borrow_mut().push("watch-stop".to_owned());
        Ok(())
    }
}

/// A timer that never runs.
pub struct IdleTimer;

impl ThemeReloadTimer for IdleTimer {
    fn cancel(&mut self) {}
}

/// A theme state over the supplied directory with its shipped documents.
pub fn theme(directory: &Path) -> Rc<ThemeState> {
    let themes = directory.join("themes");
    std::fs::create_dir_all(&themes).unwrap();
    for name in ["dark", "light"] {
        let document = include_str!("../fixtures/custom_theme.json")
            .replace("\"name\": \"custom\"", &format!("\"name\": \"{name}\""));
        std::fs::write(themes.join(format!("{name}.json")), document).unwrap();
    }
    Rc::new(ThemeState::new(ThemeDirectories {
        themes_dir: directory.join("themes").to_string_lossy().into_owned(),
        custom_themes_dir: directory.join("custom").to_string_lossy().into_owned(),
    }))
}

/// The default settings manager of one presentation.
pub fn settings() -> Rc<RefCell<SettingsManager>> {
    Rc::new(RefCell::new(
        SettingsManager::in_memory(Settings::default()),
    ))
}

/// The options of one configuration presentation.
pub fn options(settings_manager: Rc<RefCell<SettingsManager>>) -> ConfigSelectorOptions {
    ConfigSelectorOptions {
        resolved_paths: ResolvedPaths::default(),
        settings_manager,
        cwd: "/project".to_owned(),
        agent_dir: "/agent".to_owned(),
    }
}

/// Watch effects that do nothing.
pub struct NoWatch;

impl ThemeWatchOperations for NoWatch {
    fn watch(
        &self,
        _path: &str,
        _listener: Rc<dyn Fn(Option<String>)>,
        _on_error: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn ThemeWatcher>> {
        Ok(Box::new(ClosingWatch { log: Rc::default() }))
    }

    fn schedule(
        &self,
        _delay: Duration,
        _callback: Box<dyn FnOnce()>,
    ) -> Box<dyn ThemeReloadTimer> {
        Box::new(IdleTimer)
    }
}
