//! Cancellation through the shared signal and the actual terminal component.

use maestro_tui::keybindings::{KeybindingKeys, KeybindingsManager, TUI_KEYBINDINGS};
use maestro_tui::keys::{is_kitty_protocol_active, set_kitty_protocol_active};
use maestro_tui::tui::InputHandler;
use maestro_tui::{
    CancellableLoader, Component, Loader, LoaderIndicatorOptions, TUI, TerminalImage,
};
use maestro_tui::{get_keybindings, set_keybindings};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};

#[allow(
    dead_code,
    reason = "Support items are shared by several test targets."
)]
mod support {
    pub mod checks;
    pub mod manual_runtime;
    pub mod recording_terminal;
}
use support::checks::{parse, succeeds};
use support::manual_runtime::{ManualRuntime, ms};
use support::recording_terminal::RecordingTerminal;

/// Serializes mutations of the toolkit's global input configuration.
static INPUT_STATE: Mutex<()> = Mutex::new(());
/// Restores the active manager and protocol even on a failed assertion.
struct InputState {
    /// Exclusive test configuration access.
    _guard: MutexGuard<'static, ()>,
    /// Original live manager.
    manager: KeybindingsManager,
    /// Original keyboard protocol.
    active: bool,
}
impl InputState {
    /// Installs isolated default bindings.
    fn new() -> Self {
        let guard = INPUT_STATE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = Self {
            _guard: guard,
            manager: get_keybindings(),
            active: is_kitty_protocol_active(),
        };
        set_keybindings(manager(None));
        set_kitty_protocol_active(false);
        state
    }
}
impl Drop for InputState {
    fn drop(&mut self) {
        set_keybindings(self.manager.clone());
        set_kitty_protocol_active(self.active);
    }
}
/// Creates defaults or an explicit cancel override.
fn manager(keys: Option<Vec<String>>) -> KeybindingsManager {
    let bindings = keys.map_or_else(Vec::new, |keys| {
        vec![(
            "tui.select.cancel".into(),
            Some(KeybindingKeys::Multiple(keys)),
        )]
    });
    KeybindingsManager::new(TUI_KEYBINDINGS.clone(), bindings)
}
/// Creates the real writer with controlled time.
fn writer() -> (TUI, RecordingTerminal, ManualRuntime) {
    let runtime = ManualRuntime::new();
    let terminal = RecordingTerminal::new(16, 24);
    let tui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::default(),
        None,
    );
    (tui, terminal, runtime)
}
/// Creates an identity-styled two-frame loader.
fn widget() -> (Rc<CancellableLoader>, ManualRuntime) {
    let (tui, _, runtime) = writer();
    succeeds(tui.stop());
    let loader = Loader::new(
        tui,
        Rc::new(str::to_owned),
        Rc::new(str::to_owned),
        Some("M".into()),
        Some(options(&["A", "B"], 80.0)),
    );
    (Rc::new(CancellableLoader::new(loader)), runtime)
}
/// Explicit owned indicator configuration.
fn options(frames: &[&str], interval: f64) -> LoaderIndicatorOptions {
    LoaderIndicatorOptions {
        frames: Some(frames.iter().map(|frame| (*frame).into()).collect()),
        interval_ms: Some(interval),
    }
}
/// Installs a callback witnessing both count and the already-aborted state.
fn counted(widget: &Rc<CancellableLoader>) -> Rc<Cell<usize>> {
    let count = Rc::new(Cell::new(0));
    let observed = Rc::clone(&count);
    let weak = Rc::downgrade(widget);
    widget.set_on_abort(Some(Rc::new(move || {
        assert!(weak.upgrade().is_some_and(|widget| widget.aborted()));
        observed.set(observed.get() + 1);
    })));
    count
}

#[test]
fn cancellable_loader_notifies_each_match_after_abort() {
    let _state = InputState::new();
    let (widget, _) = widget();
    assert!(!widget.aborted());
    assert!(widget.on_abort().is_none());
    assert!(std::ptr::eq(widget.signal(), widget.signal()));
    widget.handle_input("x");
    assert!(!widget.aborted());
    widget.handle_input("\x1b");
    assert!(widget.aborted());
    let count = counted(&widget);
    let old = widget.on_abort().unwrap();
    for data in ["\x1b", "\x03", "x"] {
        widget.handle_input(data);
    }
    assert_eq!(count.get(), 2);
    let replacement = counted(&widget);
    assert!(!Rc::ptr_eq(&old, &widget.on_abort().unwrap()));
    widget.handle_input("\x1b");
    assert_eq!(replacement.get(), 1);
    assert_eq!(count.get(), 2);
    widget.set_on_abort(None);
    widget.handle_input("\x1b");
    assert!(widget.on_abort().is_none());
    assert_eq!(replacement.get(), 1);
    assert!(widget.aborted());
}

/// Scalar or list override as recorded by the reference consumer.
#[derive(serde::Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
enum CaseKeys {
    /// One binding.
    Single(String),
    /// Zero or more bindings.
    Multiple(Vec<String>),
}
/// A distinct oracle query and both observable cancellation outcomes.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyCase {
    /// Input bytes passed directly to the component.
    data: String,
    /// Keyboard protocol mode.
    active: bool,
    /// Optional cancel override; null keeps defaults.
    keys: Option<CaseKeys>,
    /// Expected latched signal state.
    aborted: bool,
    /// Expected callback invocations.
    callbacks: usize,
}
#[test]
fn cancellable_loader_binding_inputs_follow_active_manager() {
    let _state = InputState::new();
    let cases: Vec<KeyCase> = parse(include_str!("fixtures/cancellation_keys.json"));
    assert_eq!(cases.len(), 70);
    let mut queries = std::collections::HashSet::new();
    for case in cases {
        assert!(queries.insert((case.data.clone(), case.active, case.keys.clone())));
        set_keybindings(manager(case.keys.map(|keys| match keys {
            CaseKeys::Single(key) => vec![key],
            CaseKeys::Multiple(keys) => keys,
        })));
        set_kitty_protocol_active(case.active);
        let (widget, _) = widget();
        let count = counted(&widget);
        widget.handle_input(&case.data);
        assert_eq!(widget.aborted(), case.aborted, "{:?}", case.data);
        assert_eq!(count.get(), case.callbacks, "{:?}", case.data);
    }
    set_kitty_protocol_active(false);
    let active = manager(Some(Vec::new()));
    set_keybindings(active.clone());
    let (widget, _) = widget();
    let count = counted(&widget);
    widget.handle_input("\x1b");
    assert!(!widget.aborted());
    active.set_user_bindings(vec![(
        "tui.select.cancel".into(),
        Some(KeybindingKeys::Single("ctrl+g".into())),
    )]);
    widget.handle_input("\x07");
    assert!(widget.aborted());
    assert_eq!(count.get(), 1);
    set_keybindings(manager(Some(vec!["alt+x".into()])));
    widget.handle_input("\x07");
    assert_eq!(count.get(), 1);
    widget.handle_input("\x1bx");
    assert_eq!(count.get(), 2);
}

#[test]
fn cancellable_loader_callbacks_can_replace_and_reenter() {
    let _state = InputState::new();
    let (widget, runtime) = widget();
    let log = Rc::new(RefCell::new(Vec::new()));
    let weak = Rc::downgrade(&widget);
    let calls = Rc::clone(&log);
    widget.set_on_abort(Some(Rc::new(move || {
        calls.borrow_mut().push("outer");
        let widget = weak.upgrade().unwrap();
        assert!(widget.aborted());
        let inner = Rc::clone(&calls);
        widget.set_on_abort(Some(Rc::new(move || inner.borrow_mut().push("inner"))));
        widget.handle_input("\x1b");
        calls.borrow_mut().push("return");
    })));
    widget.handle_input("\x1b");
    assert_eq!(*log.borrow(), ["outer", "inner", "return"]);
    let weak = Rc::downgrade(&widget);
    let count = Rc::new(Cell::new(0));
    let calls = Rc::clone(&count);
    widget.set_on_abort(Some(Rc::new(move || {
        calls.set(calls.get() + 1);
        weak.upgrade().unwrap().dispose();
    })));
    widget.handle_input("\x1b");
    succeeds(runtime.settle());
    assert_eq!(runtime.pending(), 0);
    assert_eq!(widget.render(16), ["", " A M            "]);
    assert_eq!(count.get(), 1);
    callback_destruction_can_reenter(&widget);
}

#[test]
fn cancellable_loader_signal_wakes_retained_observers() {
    use std::future::Future;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::task::{Context, Poll, Wake, Waker};
    /// Counts wakes from the shared signal.
    struct Counter(AtomicUsize);
    impl Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let _state = InputState::new();
    let (widget, _) = widget();
    let retained = widget.signal().clone();
    let complete = Rc::new(Cell::new(0));
    let mut observers = Vec::new();
    for _ in 0..2 {
        let signal = widget.signal().clone();
        let complete = Rc::clone(&complete);
        observers.push(Box::pin(async move {
            signal.cancelled().await;
            assert!(signal.is_aborted());
            complete.set(complete.get() + 1);
        }));
    }
    let counter = Arc::new(Counter(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&counter));
    let mut context = Context::from_waker(&waker);
    for observer in &mut observers {
        assert_eq!(observer.as_mut().poll(&mut context), Poll::Pending);
    }
    assert_eq!(complete.get(), 0);
    widget.handle_input("\x1b");
    assert_eq!(counter.0.load(Ordering::SeqCst), 2);
    for observer in &mut observers {
        assert_eq!(observer.as_mut().poll(&mut context), Poll::Ready(()));
    }
    assert_eq!(complete.get(), 2);
    widget.dispose();
    drop(widget);
    assert!(retained.is_aborted());
}

/// Executes every controlled due callback and checks its result.
fn advance(runtime: &ManualRuntime, time: u64) {
    for result in runtime.advance_to(ms(time)) {
        succeeds(result);
    }
}
#[test]
fn cancellable_loader_dispose_stops_animation_without_aborting() {
    let _state = InputState::new();
    let (widget, runtime) = widget();
    let underlying = widget.loader().clone();
    let count = counted(&widget);
    advance(&runtime, 80);
    assert_eq!(widget.render(16), ["", " B M            "]);
    widget.dispose();
    widget.dispose();
    succeeds(runtime.settle());
    assert_eq!(runtime.pending(), 0);
    assert_eq!(widget.render(16), ["", " B M            "]);
    assert!(!widget.aborted());
    assert_eq!(count.get(), 0);
    underlying.set_message("N".into());
    assert_eq!(widget.render(16), ["", " B N            "]);
    assert_eq!(runtime.pending(), 0);
    widget.loader().start();
    let next = runtime.next_deadline().unwrap();
    for result in runtime.advance_to(next) {
        succeeds(result);
    }
    assert_eq!(widget.render(16), ["", " A N            "]);
    widget.handle_input("\x1b");
    assert!(runtime.next_deadline().is_some());
    let next = runtime.next_deadline().unwrap();
    for result in runtime.advance_to(next) {
        succeeds(result);
    }
    assert_eq!(widget.render(16), ["", " B N            "]);
    widget.dispose();
    succeeds(runtime.settle());
    assert_eq!(runtime.pending(), 0);
    dispose_static_and_hidden();
}

/// Disposal of nonanimated owners leaves their signal clear.
fn dispose_static_and_hidden() {
    for frames in [&[][..], &["*"][..]] {
        let (tui, _, runtime) = writer();
        succeeds(tui.stop());
        let widget = CancellableLoader::new(Loader::new(
            tui,
            Rc::new(str::to_owned),
            Rc::new(str::to_owned),
            None,
            Some(options(frames, 80.0)),
        ));
        widget.dispose();
        succeeds(runtime.settle());
        assert_eq!(runtime.pending(), 0);
        assert!(!widget.aborted());
    }
}

#[test]
fn cancellable_loader_embedded_loader_preserves_rendering() {
    let _state = InputState::new();
    let cases = [
        (
            None,
            None,
            vec!["", " <⠋>            ", " [Loading...]   "],
            Some(80),
        ),
        (
            Some("Working..."),
            Some(options(&[], 80.0)),
            vec!["", " [Working...]   "],
            None,
        ),
        (
            Some(""),
            Some(options(&[], 80.0)),
            vec!["", " []             "],
            None,
        ),
        (
            Some("S"),
            Some(options(&["*"], 80.0)),
            vec!["", " * [S]          "],
            None,
        ),
        (
            Some("M"),
            Some(options(&["X", "Y"], 7.0)),
            vec!["", " X [M]          "],
            Some(7),
        ),
    ];
    for (message, indicator, rows, deadline) in cases {
        let (tui, _, runtime) = writer();
        succeeds(tui.stop());
        let widget = CancellableLoader::new(Loader::new(
            tui,
            Rc::new(|frame| format!("<{frame}>")),
            Rc::new(|message| format!("[{message}]")),
            message.map(str::to_owned),
            indicator,
        ));
        assert_eq!(widget.render(16), rows);
        assert_eq!(runtime.next_deadline(), deadline.map(ms));
        widget.invalidate();
        assert_eq!(widget.render(16), rows);
        assert!(!widget.aborted());
        widget.dispose();
    }
    let (widget, _) = widget();
    widget.loader().set_indicator(Some(options(&["Z"], 7.0)));
    assert_eq!(widget.render(16), ["", " Z M            "]);
}

#[test]
fn cancellable_loader_routes_focused_input() {
    let _state = InputState::new();
    let (tui, terminal, _) = writer();
    let widget = Rc::new(CancellableLoader::new(Loader::new(
        tui.clone(),
        Rc::new(str::to_owned),
        Rc::new(str::to_owned),
        None,
        Some(options(&[], 80.0)),
    )));
    let count = counted(&widget);
    tui.add_child(widget.clone());
    tui.set_focus(Some(widget.clone()));
    succeeds(tui.start());
    set_kitty_protocol_active(true);
    terminal.send_input("\x1b[27;1:3u");
    assert!(!widget.aborted());
    assert_eq!(count.get(), 0);
    for data in ["\x1b", "\x03", "\x1b[27;1:2u"] {
        terminal.send_input(data);
    }
    assert!(widget.aborted());
    assert_eq!(count.get(), 3);
    widget.handle_input("\x1b[27;1:3u");
    assert_eq!(count.get(), 4);
    set_keybindings(manager(Some(vec!["ctrl+g".into()])));
    terminal.send_input("\x1b");
    assert_eq!(count.get(), 4);
    terminal.send_input("\x07");
    assert_eq!(count.get(), 5);
    succeeds(tui.stop());
    widget.dispose();
}

/// A callback capture whose destructor replaces the callback again.
struct CallbackDrop {
    /// Widget identity without a cycle.
    widget: std::rc::Weak<CancellableLoader>,
    /// Witness after the reentrant setter completed.
    complete: Rc<Cell<bool>>,
}
impl Drop for CallbackDrop {
    fn drop(&mut self) {
        if let Some(widget) = self.widget.upgrade() {
            widget.set_on_abort(None);
            self.complete.set(true);
        }
    }
}
/// Replacement releases its mutable borrow before destroying captured caller state.
fn callback_destruction_can_reenter(widget: &Rc<CancellableLoader>) {
    let complete = Rc::new(Cell::new(false));
    let capture = CallbackDrop {
        widget: Rc::downgrade(widget),
        complete: Rc::clone(&complete),
    };
    widget.set_on_abort(Some(Rc::new(move || {
        let _keep = &capture;
    })));
    widget.set_on_abort(Some(Rc::new(|| {})));
    assert!(complete.get());
    assert!(widget.on_abort().is_none());
}
