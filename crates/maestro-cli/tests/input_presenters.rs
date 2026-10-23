//! The configuration and session selector lifecycles through supplied presenters.

use maestro_app::presentation_data::input_selectors::{
    ConfigSelectorCallbacks, ConfigSelectorFactory, ConfigSelectorOptions, PresentedSelector,
    SessionSelectorCallbacks, SessionSelectorFactory, SessionSelectorRequest, SessionsFuture,
    SessionsLoader,
};
use maestro_app::{KeybindingsManager, ResolvedPaths, SessionInfo, Settings, SettingsManager};
use maestro_cli::input::config_selector::{ConfigPresentation, select_config};
use maestro_cli::input::selector::SelectorHost;
use maestro_cli::input::session_picker::{SessionPresentation, select_session};
use maestro_test_directory::Scratch;
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
use std::task::{Context, Poll, Waker};
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
fn view(log: &Rc<RefCell<Vec<String>>>) -> PresentedSelector {
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
struct Harness {
    /// The terminal every frame is written to.
    terminal: RecordingTerminal,
    /// The clock and deferred work of the frame writer.
    runtime: ManualRuntime,
}

impl Harness {
    /// A writer of 80 by 24 cells at time zero.
    fn new() -> Self {
        Self {
            terminal: RecordingTerminal::new(80, 24),
            runtime: ManualRuntime::new(),
        }
    }

    /// The supplied host over this harness and the given exit effect.
    fn host(&self, exit: Rc<dyn Fn(i32)>) -> SelectorHost {
        let (terminal, runtime) = (self.terminal.clone(), self.runtime.clone());
        SelectorHost {
            create_ui: Rc::new(move || {
                Ok(TUI::new(
                    terminal.handle(),
                    runtime.handle(),
                    TerminalImage::default(),
                    Some(false),
                ))
            }),
            exit,
        }
    }

    /// Run every frame the writer scheduled.
    fn frames(&self) {
        let _ = self.runtime.advance_to(Duration::from_millis(50));
    }
}

/// Watch effects that record their creation and close.
struct RecordingWatch {
    /// Effects in the order requested.
    log: Rc<RefCell<Vec<String>>>,
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
struct ClosingWatch {
    /// Effects in the order requested.
    log: Rc<RefCell<Vec<String>>>,
}

impl ThemeWatcher for ClosingWatch {
    fn close(&mut self) -> io::Result<()> {
        self.log.borrow_mut().push("watch-stop".to_owned());
        Ok(())
    }
}

/// A timer that never runs.
struct IdleTimer;

impl ThemeReloadTimer for IdleTimer {
    fn cancel(&mut self) {}
}

/// A theme state over the supplied directory with its shipped documents.
fn theme(directory: &Path) -> Rc<ThemeState> {
    let themes = directory.join("themes");
    std::fs::create_dir_all(&themes).unwrap();
    for name in ["dark", "light"] {
        let document = include_str!("fixtures/custom_theme.json")
            .replace("\"name\": \"custom\"", &format!("\"name\": \"{name}\""));
        std::fs::write(themes.join(format!("{name}.json")), document).unwrap();
    }
    Rc::new(ThemeState::new(ThemeDirectories {
        themes_dir: directory.join("themes").to_string_lossy().into_owned(),
        custom_themes_dir: directory.join("custom").to_string_lossy().into_owned(),
    }))
}

/// The default settings manager of one presentation.
fn settings() -> Rc<RefCell<SettingsManager>> {
    Rc::new(RefCell::new(
        SettingsManager::in_memory(Settings::default()),
    ))
}

/// The options of one configuration presentation.
fn options(settings_manager: Rc<RefCell<SettingsManager>>) -> ConfigSelectorOptions {
    ConfigSelectorOptions {
        resolved_paths: ResolvedPaths::default(),
        settings_manager,
        cwd: "/project".to_owned(),
        agent_dir: "/agent".to_owned(),
    }
}

/// Watch effects that do nothing.
struct NoWatch;

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

/// An empty discovery scope.
fn no_sessions() -> SessionsFuture {
    Box::pin(async { Ok(Vec::<SessionInfo>::new()) })
}

/// The discovery factories of one session presentation.
fn loaders() -> SessionsLoader {
    Rc::new(|_progress| no_sessions())
}

#[test]
fn configuration_focus_precedes_start_and_close_settles_once() {
    let scratch = Scratch::new().unwrap();
    let harness = Harness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let captured: Rc<RefCell<Option<ConfigSelectorCallbacks>>> = Rc::default();
    let (record, view_log) = (Rc::clone(&captured), Rc::clone(&log));
    let presenter: ConfigSelectorFactory = Rc::new(move |_options, callbacks| {
        *record.borrow_mut() = Some(callbacks);
        Ok(view(&view_log))
    });
    let presentation = ConfigPresentation {
        host: harness.host(Rc::new(|_code: i32| ())),
        theme: theme(scratch.path()),
        watcher: Rc::new(NoWatch),
        presenter,
    };
    let mut select = Box::pin(select_config(options(settings()), presentation));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let first = select.as_mut().poll(&mut context);
    assert!(
        first.is_pending(),
        "the presentation awaits its close: {first:?}"
    );
    harness.frames();
    assert_eq!(
        log.borrow().first().map(String::as_str),
        Some("render:true"),
        "the retained focus handle is focused before the first frame"
    );
    let close = Rc::clone(&captured.borrow().as_ref().unwrap().on_close);
    close();
    close();
    assert!(matches!(
        select.as_mut().poll(&mut context),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(
        harness.terminal.events(),
        vec!["start", "stop"],
        "one close stops the writer once"
    );
}

/// A scratch directory holding the recorded custom theme.
fn scratch_custom_theme() -> Scratch {
    let scratch = Scratch::new().unwrap();
    std::fs::create_dir_all(scratch.path().join("custom")).unwrap();
    std::fs::write(
        scratch.path().join("custom/custom.json"),
        include_str!("fixtures/custom_theme.json"),
    )
    .unwrap();
    scratch
}

#[test]
fn configuration_exit_stops_terminal_and_watcher_before_exit() {
    let scratch = scratch_custom_theme();
    let harness = Harness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let order = Rc::clone(&log);
    let watched = Rc::clone(&log);
    let frames = Rc::new(RefCell::new(Vec::new()));
    let stopped = harness.terminal.clone();
    let exit = Rc::new(move |_code: i32| {
        assert!(
            stopped.events().contains(&"stop"),
            "the terminal stops before the process exits"
        );
        assert!(
            order.borrow().contains(&"watch-stop".to_owned()),
            "the theme watch stops before the process exits"
        );
        order.borrow_mut().push("exit".to_owned());
    });
    let captured: Rc<RefCell<Option<ConfigSelectorCallbacks>>> = Rc::default();
    let (record, view_log) = (Rc::clone(&captured), Rc::clone(&frames));
    let presenter: ConfigSelectorFactory = Rc::new(move |_options, callbacks| {
        *record.borrow_mut() = Some(callbacks);
        Ok(view(&view_log))
    });
    let mut manager = SettingsManager::in_memory(Settings::default());
    manager.set_theme("custom".to_owned());
    let presentation = ConfigPresentation {
        host: harness.host(exit),
        theme: theme(scratch.path()),
        watcher: Rc::new(RecordingWatch {
            log: Rc::clone(&watched),
        }),
        presenter,
    };
    let mut select = Box::pin(select_config(
        options(Rc::new(RefCell::new(manager))),
        presentation,
    ));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let first = select.as_mut().poll(&mut context);
    assert!(
        first.is_pending(),
        "the presentation awaits its close: {first:?}"
    );
    harness.frames();
    let exit_now = Rc::clone(&captured.borrow().as_ref().unwrap().on_exit);
    exit_now();
    assert_eq!(
        log.borrow().as_slice(),
        ["watch", "watch-stop", "exit"],
        "the theme watch stops before the exit notification"
    );
    assert_eq!(harness.terminal.events(), vec!["start", "stop"]);
    drop(select);
}

#[test]
fn session_selection_and_cancel_only_settle_once() {
    let harness = Harness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let captured: Rc<RefCell<Option<SessionSelectorCallbacks>>> = Rc::default();
    let (record, view_log) = (Rc::clone(&captured), Rc::clone(&log));
    let presenter: SessionSelectorFactory = Rc::new(move |_request, callbacks| {
        *record.borrow_mut() = Some(callbacks);
        Ok(view(&view_log))
    });
    let presentation = SessionPresentation {
        host: harness.host(Rc::new(|_code: i32| ())),
        keybindings: None,
        presenter,
    };
    let mut select = Box::pin(select_session(loaders(), loaders(), presentation));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(select.as_mut().poll(&mut context).is_pending());
    harness.frames();
    {
        let callbacks = captured.borrow();
        let callbacks = callbacks.as_ref().expect("presented callbacks");
        (callbacks.on_select)("first");
        (callbacks.on_cancel)();
    }
    assert!(matches!(
        select.as_mut().poll(&mut context),
        Poll::Ready(Ok(Some(path))) if path == "first"
    ));

    let second = Harness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let captured: Rc<RefCell<Option<SessionSelectorCallbacks>>> = Rc::default();
    let (record, view_log) = (Rc::clone(&captured), Rc::clone(&log));
    let presenter: SessionSelectorFactory = Rc::new(move |_request, callbacks| {
        *record.borrow_mut() = Some(callbacks);
        Ok(view(&view_log))
    });
    let presentation = SessionPresentation {
        host: second.host(Rc::new(|_code: i32| ())),
        keybindings: None,
        presenter,
    };
    let mut select = Box::pin(select_session(loaders(), loaders(), presentation));
    assert!(select.as_mut().poll(&mut context).is_pending());
    second.frames();
    {
        let callbacks = captured.borrow();
        let callbacks = callbacks.as_ref().expect("presented callbacks");
        (callbacks.on_cancel)();
        (callbacks.on_select)("later");
    }
    assert!(matches!(
        select.as_mut().poll(&mut context),
        Poll::Ready(Ok(None))
    ));
}

#[test]
fn selector_callbacks_request_the_existing_frame_writer() {
    let scratch = Scratch::new().unwrap();
    let harness = Harness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let captured: Rc<RefCell<Option<ConfigSelectorCallbacks>>> = Rc::default();
    let (record, view_log) = (Rc::clone(&captured), Rc::clone(&log));
    let presenter: ConfigSelectorFactory = Rc::new(move |_options, callbacks| {
        *record.borrow_mut() = Some(callbacks);
        Ok(view(&view_log))
    });
    let presentation = ConfigPresentation {
        host: harness.host(Rc::new(|_code: i32| ())),
        theme: theme(scratch.path()),
        watcher: Rc::new(NoWatch),
        presenter,
    };
    let mut select = Box::pin(select_config(options(settings()), presentation));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let first = select.as_mut().poll(&mut context);
    assert!(
        first.is_pending(),
        "the presentation awaits its close: {first:?}"
    );
    harness.frames();
    harness.terminal.clear_writes();
    let waiting = harness.runtime.pending();
    let request = Rc::clone(&captured.borrow().as_ref().unwrap().request_render);
    request();
    assert!(
        harness.runtime.pending() > waiting,
        "the redraw notification reaches the existing writer"
    );
    let close = Rc::clone(&captured.borrow().as_ref().unwrap().on_close);
    close();
    let _ = select.as_mut().poll(&mut context);
}

#[test]
fn selector_state_releases_owned_values_after_completion_or_drop() {
    let scratch = Scratch::new().unwrap();
    let harness = Harness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let released: Rc<RefCell<Option<std::rc::Weak<dyn Component>>>> = Rc::default();
    let captured: Rc<RefCell<Option<ConfigSelectorCallbacks>>> = Rc::default();
    let (record, view_log, weak) = (Rc::clone(&captured), Rc::clone(&log), Rc::clone(&released));
    let presenter: ConfigSelectorFactory = Rc::new(move |_options, callbacks| {
        *record.borrow_mut() = Some(callbacks);
        let presented = view(&view_log);
        *weak.borrow_mut() = Some(Rc::downgrade(&presented.component));
        Ok(presented)
    });
    let presentation = ConfigPresentation {
        host: harness.host(Rc::new(|_code: i32| ())),
        theme: theme(scratch.path()),
        watcher: Rc::new(NoWatch),
        presenter,
    };
    let mut select = Box::pin(select_config(options(settings()), presentation));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let first = select.as_mut().poll(&mut context);
    assert!(
        first.is_pending(),
        "the presentation awaits its close: {first:?}"
    );
    harness.frames();
    let close = Rc::clone(&captured.borrow().as_ref().unwrap().on_close);
    close();
    assert!(matches!(
        select.as_mut().poll(&mut context),
        Poll::Ready(Ok(()))
    ));
    assert!(
        released
            .borrow()
            .as_ref()
            .expect("presented component")
            .upgrade()
            .is_none(),
        "a settled presentation keeps no presented component"
    );
}

#[test]
fn supplied_presenters_keep_owned_options_and_real_focus_handles() {
    let scratch = Scratch::new().unwrap();
    let harness = Harness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let manager = settings();
    let seen: Rc<RefCell<Option<ConfigSelectorOptions>>> = Rc::default();
    let captured: Rc<RefCell<Option<ConfigSelectorCallbacks>>> = Rc::default();
    let (record, view_log, observed) = (Rc::clone(&captured), Rc::clone(&log), Rc::clone(&seen));
    let presenter: ConfigSelectorFactory = Rc::new(move |options, callbacks| {
        assert_eq!(options.cwd, "/project");
        assert_eq!(options.agent_dir, "/agent");
        let presented = view(&view_log);
        assert!(
            !Rc::ptr_eq(&presented.component, &presented.focus),
            "the focused handle is not the containing component"
        );
        *observed.borrow_mut() = Some(options);
        *record.borrow_mut() = Some(callbacks);
        Ok(presented)
    });
    let presentation = ConfigPresentation {
        host: harness.host(Rc::new(|_code: i32| ())),
        theme: theme(scratch.path()),
        watcher: Rc::new(NoWatch),
        presenter,
    };
    let shared = Rc::clone(&manager);
    let mut select = Box::pin(select_config(options(shared), presentation));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let first = select.as_mut().poll(&mut context);
    assert!(
        first.is_pending(),
        "the presentation awaits its close: {first:?}"
    );
    harness.frames();
    {
        let seen = seen.borrow();
        let seen = seen.as_ref().expect("presented options");
        assert!(
            Rc::ptr_eq(&seen.settings_manager, &manager),
            "the supplied settings manager is the one presented"
        );
    }
    let close = Rc::clone(&captured.borrow().as_ref().unwrap().on_close);
    close();
    let _ = select.as_mut().poll(&mut context);
}

/// A session presentation that retains the request it was built with.
fn recording_session_presentation(
    harness: &Harness,
    log: &Rc<RefCell<Vec<String>>>,
    keybindings: KeybindingsManager,
) -> (
    Rc<RefCell<Option<SessionSelectorRequest>>>,
    SessionPresentation,
) {
    let captured: Rc<RefCell<Option<SessionSelectorRequest>>> = Rc::default();
    let (record, view_log) = (Rc::clone(&captured), Rc::clone(log));
    let presenter: SessionSelectorFactory = Rc::new(move |request, _callbacks| {
        *record.borrow_mut() = Some(request);
        Ok(view(&view_log))
    });
    let presentation = SessionPresentation {
        host: harness.host(Rc::new(|_code: i32| ())),
        keybindings: Some(keybindings),
        presenter,
    };
    (captured, presentation)
}

#[test]
fn session_loaders_keep_scope_progress_and_keybinding_identity() {
    let harness = Harness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let progress = Rc::new(RefCell::new(Vec::new()));
    let called = Rc::new(RefCell::new(0_usize));
    let seen = Rc::clone(&called);
    let current: SessionsLoader = Rc::new(move |notify| {
        if let Some(notify) = notify {
            (notify)(1, 2);
        }
        *seen.borrow_mut() += 1;
        no_sessions()
    });
    let all: SessionsLoader = Rc::new(|_notify| no_sessions());
    let manager = KeybindingsManager::default();
    let bindings = manager.get_resolved_bindings();
    let (captured, presentation) = recording_session_presentation(&harness, &log, manager);
    let mut select = Box::pin(select_session(
        Rc::clone(&current),
        Rc::clone(&all),
        presentation,
    ));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(select.as_mut().poll(&mut context).is_pending());
    harness.frames();
    let request = captured.borrow_mut().take().expect("presented request");
    assert!(
        Rc::ptr_eq(&request.current_sessions_loader, &current),
        "the current-scope loader is the supplied one"
    );
    assert!(
        Rc::ptr_eq(&request.all_sessions_loader, &all),
        "the all-scope loader is the supplied one"
    );
    assert!(
        !Rc::ptr_eq(
            &request.current_sessions_loader,
            &request.all_sessions_loader
        ),
        "the two scopes stay distinct"
    );
    assert!(!request.show_rename_hint, "the rename hint is hidden");
    assert_eq!(
        request.keybindings.get_resolved_bindings(),
        bindings,
        "the supplied bindings reach the presentation"
    );
    let progress_seen = Rc::clone(&progress);
    let notify: Rc<dyn Fn(usize, usize)> = {
        let progress_seen = Rc::clone(&progress_seen);
        Rc::new(move |done, total| progress_seen.borrow_mut().push((done, total)))
    };
    drop((request.current_sessions_loader)(Some(notify)));
    assert_eq!(
        *progress.borrow(),
        vec![(1, 2)],
        "progress reaches the caller"
    );
    assert_eq!(*called.borrow(), 1, "the current-scope loader ran once");
}
