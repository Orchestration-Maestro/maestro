//! Strict recorded inputs, controlled hosts and lossless dialog observations.
use super::{manual_runtime::ms, recording_terminal::RecordingTerminal, runtime_spy::Spy};
use crate::bindings;
use bindings::{Binding, bindings};
use maestro_app::presentation_data::keybindings::KEYBINDINGS;
use maestro_chat::{
    ExtensionInputComponent, ExtensionInputOptions, ExtensionSelectorComponent,
    ExtensionSelectorOptions,
};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use maestro_tui::{
    Component, Container, KeybindingsManager, TUI, TerminalImage, Text, get_keybindings,
    set_keybindings,
};
use serde::Deserialize;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    rc::{Rc, Weak},
};

/// One admission owner for global manager construction, mutation, replay and cleanup.
use crate::registry::REGISTRY as ADMISSION;
/// Restores the original manager before releasing admission.
struct Restore(KeybindingsManager);
impl Drop for Restore {
    fn drop(&mut self) {
        set_keybindings(self.0.clone());
    }
}
/// One query owned by a public test.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Witness attribution.
    test: String,
    /// Constructor and operations.
    input: Query,
    /// Complete observed results.
    expected: Expected,
}
/// Dialog kind.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// String selector.
    Selector,
    /// Text editor.
    Input,
}
/// Supplied constructor inputs and optional specialized scenario.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    /// Component kind.
    kind: Kind,
    /// Supplied title, otherwise the recorder's kind-specific title.
    title: Option<String>,
    /// Supplied options, otherwise the recorder's three options.
    options: Option<Vec<String>>,
    /// Accepted inert placeholder.
    placeholder: Option<String>,
    /// Viewport cells.
    width: Option<usize>,
    /// Supplied timeout or explicit nonfinite tag.
    timeout: Option<Timeout>,
    /// Whether the writer is passed into the component.
    #[serde(default)]
    tui: bool,
    /// Constructor manager configuration.
    #[serde(default)]
    bindings: BTreeMap<String, Binding>,
    /// Subsequent operations.
    #[serde(default)]
    ops: Vec<Operation>,
    /// Dispose instant.
    at: Option<u64>,
    /// Reentrant cancellation phase; absence means selection/submission.
    phase: Option<Phase>,
}
impl Query {
    /// Apply the recorder's specialized reentry option/deadline setup.
    fn content(&self, reentrant: bool) -> (Vec<String>, Option<f64>) {
        let mut options = self
            .options
            .clone()
            .unwrap_or_else(|| vec!["Alpha".into(), "Beta".into(), "Gamma".into()]);
        if reentrant {
            options.truncate(if self.phase.is_some() { 1 } else { 2 });
        }
        let timeout = self
            .timeout
            .as_ref()
            .map(Timeout::value)
            .or_else(|| reentrant.then_some(1000.0));
        (options, timeout)
    }
}
/// Explicit phase for specialized cancellation witnesses.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Phase {
    /// Explicit cancellation.
    Cancel,
    /// Timer expiry.
    Expire,
}
/// Finite values and the three explicit nonfinite recorder tags.
#[derive(Deserialize)]
#[serde(untagged)]
enum Timeout {
    /// Ordinary numeric delay.
    Number(f64),
    /// Explicit nonfinite tag.
    Tagged(Nonfinite),
}
/// Nonfinite tags retain their distinct comparison behavior.
#[derive(Deserialize)]
enum Nonfinite {
    /// Inactive unordered comparison.
    NaN,
    /// Positive infinity is active and non-expiring.
    Infinity,
    /// Negative infinity is inactive.
    #[serde(rename = "-Infinity")]
    NegativeInfinity,
}
impl Timeout {
    /// Decodes only the recorder's declared numeric representation.
    fn value(&self) -> f64 {
        match self {
            Self::Number(value) => *value,
            Self::Tagged(Nonfinite::NaN) => f64::NAN,
            Self::Tagged(Nonfinite::Infinity) => f64::INFINITY,
            Self::Tagged(Nonfinite::NegativeInfinity) => f64::NEG_INFINITY,
        }
    }
}
/// One key or controlled host/configuration operation.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Operation {
    /// Raw input bytes.
    Key(String),
    /// Absolute clock instant.
    Advance { advance: u64 },
    /// Focus setter.
    Focus { focus: bool },
    /// Live theme replacement.
    Theme { theme: u8 },
    /// Active manager replacement.
    Replace { replace: BTreeMap<String, Binding> },
    /// Active manager mutation.
    Bindings { bindings: BTreeMap<String, Binding> },
    /// Stop the countdown.
    Dispose { dispose: bool },
    /// Invalidate chrome.
    Invalidate { invalidate: bool },
}
/// Full rendering and callback observation.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Frame {
    /// Complete styled lines.
    lines: Vec<String>,
    /// Ordered cumulative notifications, including repeated occurrences.
    events: Vec<Vec<String>>,
    /// Requests observed at the writer's scheduling boundary.
    requests: usize,
    /// Parent focus or absent capability.
    focused: Option<bool>,
}
/// Independent parent/child focus observations.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct FocusFrame {
    /// Parent flag.
    parent: bool,
    /// Child flag.
    child: bool,
    /// Child's complete rendered line, including its marker.
    child_lines: Vec<String>,
}
/// All recorded output shapes are strictly decoded.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// Single terminal observation.
    Frame(Frame),
    /// One terminal observation after construction and each operation.
    Frames { frames: Vec<Frame> },
    /// Independent focus observations.
    Focus { frames: Vec<FocusFrame> },
    /// Editable container transaction.
    Children(Box<ChildFrames>),
    /// Constructor failure.
    Failure { failed: bool },
}
/// Complete observations across child-array replacement and restoration.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct ChildFrames {
    /// Original chrome.
    initial: Frame,
    /// Chrome removed.
    cleared: Frame,
    /// Replacement chrome installed.
    replaced: Frame,
    /// Retained target edited while hidden.
    hidden: Frame,
    /// Original chrome restored.
    restored: Frame,
}
/// Selection/submission and cancellation callback pair.
pub type Callbacks = (Rc<dyn Fn(&str)>, Rc<dyn Fn()>);
/// Recorded five-role palette, not a production palette.
pub fn colors(shift: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Border, ColorValue::Index(31 + shift)),
                (ThemeColor::Accent, ColorValue::Index(32 + shift)),
                (ThemeColor::Text, ColorValue::Index(33 + shift)),
                (ThemeColor::Dim, ColorValue::Index(34 + shift)),
                (ThemeColor::Muted, ColorValue::Index(35 + shift)),
            ],
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Typed owners retain the public inherited container interface.
pub enum Dialog {
    /// Clamped selector owner.
    Selector(ExtensionSelectorComponent),
    /// Forwarding editor owner.
    Input(ExtensionInputComponent),
}
impl Dialog {
    /// Public render/input capability.
    pub fn component(&self) -> &dyn Component {
        match self {
            Self::Selector(value) => value,
            Self::Input(value) => value,
        }
    }
    /// Public child-array editing owner.
    pub fn container(&self) -> &Container {
        match self {
            Self::Selector(value) => value,
            Self::Input(value) => value,
        }
    }
    /// Stop owned countdown work.
    pub fn dispose(&self) {
        match self {
            Self::Selector(value) => value.dispose(),
            Self::Input(value) => value.dispose(),
        }
    }
    /// Deliver one chunk through the public capability.
    pub fn key(&self, key: &str) {
        self.component().input_handler().unwrap().handle_input(key);
    }
}
/// Construct the mapped root-exported component using the supplied hosts.
pub fn construct(
    kind: Kind,
    callbacks: Callbacks,
    opts: ExtensionSelectorOptions,
    content: (String, Vec<String>, Option<&str>),
) -> Result<Dialog, maestro_theme::ThemeError> {
    let (select, cancel) = callbacks;
    let (title, options, placeholder) = content;
    match kind {
        Kind::Selector => ExtensionSelectorComponent::new(title, options, select, cancel, opts)
            .map(Dialog::Selector),
        Kind::Input => ExtensionInputComponent::new(
            title,
            placeholder,
            select,
            cancel,
            ExtensionInputOptions {
                tui: opts.tui,
                timeout: opts.timeout,
                runtime: opts.runtime,
                theme: opts.theme,
            },
        )
        .map(Dialog::Input),
    }
}
/// Writer using the same scheduling observer as the existing countdown tests.
pub fn writer(spy: &Spy, terminal: &RecordingTerminal) -> TUI {
    TUI::new(
        terminal.handle(),
        Rc::new(spy.clone()),
        TerminalImage::new(|_| None, || 1),
        Some(false),
    )
}
/// Advance at each producer deadline so rescheduled ticks bind to their due instant.
pub fn advance(spy: &Spy, target: u64) {
    while let Some(due) = spy.clock.next_deadline().filter(|due| *due <= ms(target)) {
        for result in spy.clock.advance_to(due) {
            result.unwrap();
        }
    }
    for result in spy.clock.advance_to(ms(target)) {
        result.unwrap();
    }
}
/// One replay retains the actual writer and shared callback transcript.
struct Replay {
    /// Typed dialog owner.
    dialog: Rc<Dialog>,
    /// Ordered callbacks.
    events: Rc<RefCell<Vec<Vec<String>>>>,
    /// Scheduling observation.
    spy: Spy,
    /// Theme publication owner.
    theme: maestro_theme::ThemeState,
    /// Writer kept alive for weak render requests.
    _writer: TUI,
    /// Width used by the recorded query.
    width: usize,
}
impl Replay {
    /// Observe every emitted row and event without synthesizing labels.
    fn frame(&self) -> Frame {
        Frame {
            lines: self.dialog.component().render(self.width),
            events: self.events.borrow().clone(),
            requests: self
                .spy
                .events
                .borrow()
                .iter()
                .filter(|event| **event == serde_json::json!(["render"]))
                .count(),
            focused: self
                .dialog
                .component()
                .focusable()
                .map(|value| value.focus_flag().get()),
        }
    }
    /// Drive the public operation named by the fixture.
    fn apply(&self, operation: &Operation) {
        match operation {
            Operation::Key(key) => self.dialog.key(key),
            Operation::Advance { advance: target } => advance(&self.spy, *target),
            Operation::Focus { focus } => self
                .dialog
                .component()
                .focusable()
                .unwrap()
                .focus_flag()
                .set(*focus),
            Operation::Theme { theme } => self.theme.set_theme_instance(colors(*theme)).unwrap(),
            Operation::Replace { replace } => bindings::replace(replace),
            Operation::Bindings { bindings: values } => {
                bindings::mutate(values);
            }
            Operation::Dispose { dispose } => {
                assert!(*dispose);
                self.dialog.dispose();
            }
            Operation::Invalidate { invalidate } => {
                assert!(*invalidate);
                self.dialog.component().invalidate();
            }
        }
    }
}
/// A callback can reach its dialog without a strong ownership cycle.
type Target = Rc<RefCell<Option<Weak<Dialog>>>>;
/// Append actual notifications, then run one recorded synchronous reentry transaction.
fn callback(
    events: Rc<RefCell<Vec<Vec<String>>>>,
    target: Target,
    action: &'static str,
    reenter: Option<&'static str>,
) -> Rc<dyn Fn(&str)> {
    let first = std::cell::Cell::new(true);
    Rc::new(move |value| {
        let mut event = vec![action.to_owned()];
        if action != "cancel" {
            event.push(value.to_owned());
        }
        events.borrow_mut().push(event);
        if let Some(key) = reenter.filter(|_| first.replace(false)) {
            let dialog = target.borrow().as_ref().unwrap().upgrade().unwrap();
            dialog.dispose();
            if action != "cancel" {
                dialog.key(key);
            }
            assert!(!dialog.component().render(20).is_empty());
            dialog.key("\r");
            if action == "cancel" {
                dialog.key("\x1b");
            }
        }
    })
}
/// Construct recorded callbacks and hosts, retaining only weak callback backreferences.
fn replay(
    query: &Query,
    test: &str,
    events: Rc<RefCell<Vec<Vec<String>>>>,
) -> Result<Replay, maestro_theme::ThemeError> {
    set_keybindings(KeybindingsManager::new(
        KEYBINDINGS.clone(),
        bindings(&query.bindings),
    ));
    let (theme, live) = super::published::published(&colors(0));
    let live = if test == "dialog_constructor_propagates_hint_theme_failure" {
        super::published::unpublished().theme()
    } else {
        live
    };
    let spy = Spy::default();
    let tui = writer(&spy, &RecordingTerminal::new(80, 24));
    let target: Target = Rc::new(RefCell::new(None));
    let reentrant = test == "dialog_callbacks_allow_reentry_and_disposal";
    let (action, key, default_title) = match query.kind {
        Kind::Selector => ("select", "j", "Choose"),
        Kind::Input => ("submit", "Z", "Enter"),
    };
    let select = callback(
        events.clone(),
        target.clone(),
        action,
        (reentrant && query.phase.is_none()).then_some(key),
    );
    let cancel = callback(
        events.clone(),
        target.clone(),
        "cancel",
        (reentrant && query.phase.is_some()).then_some(""),
    );
    let (options, timeout) = query.content(reentrant);
    let dialog = Rc::new(construct(
        query.kind,
        (select, Rc::new(move || cancel(""))),
        ExtensionSelectorOptions {
            tui: (query.tui || reentrant).then(|| tui.clone()),
            timeout,
            runtime: Rc::new(spy.clone()),
            theme: live,
        },
        (
            query.title.clone().unwrap_or_else(|| default_title.into()),
            options,
            query.placeholder.as_deref(),
        ),
    )?);
    *target.borrow_mut() = Some(Rc::downgrade(&dialog));
    Ok(Replay {
        dialog,
        events,
        spy,
        theme,
        _writer: tui,
        width: query.width.unwrap_or(80),
    })
}
/// Full operation sequence including the initial observation.
fn frames(replay: &Replay, operations: &[Operation]) -> Expected {
    let mut frames = vec![replay.frame()];
    for operation in operations {
        replay.apply(operation);
        frames.push(replay.frame());
    }
    Expected::Frames { frames }
}
/// Observe retained child identity after a container removal and restoration.
fn children(replay: &Replay) -> Expected {
    let container = replay.dialog.container();
    let original = container.children();
    let initial = replay.frame();
    container.clear();
    let cleared = replay.frame();
    container.add_child(Rc::new(Text::new("replacement".into(), 0, 0, None)));
    let replaced = replay.frame();
    match &*replay.dialog {
        Dialog::Input(_) => {
            replay
                .dialog
                .component()
                .focusable()
                .unwrap()
                .focus_flag()
                .set(true);
            replay.dialog.key("retained");
        }
        Dialog::Selector(_) => replay.dialog.key("j"),
    }
    replay.dialog.key("\r");
    let hidden = replay.frame();
    container.set_children(original);
    Expected::Children(Box::new(ChildFrames {
        initial,
        cleared,
        replaced,
        hidden,
        restored: replay.frame(),
    }))
}
/// Setter effects are observed through the retained child's own flag immediately.
fn focus(replay: &Replay) -> Expected {
    let child = replay
        .dialog
        .container()
        .children()
        .borrow()
        .iter()
        .find(|child| child.focusable().is_some())
        .unwrap()
        .clone();
    let parent = replay.dialog.component().focusable().unwrap().focus_flag();
    let child_flag = child.focusable().unwrap().focus_flag();
    let capture = || FocusFrame {
        parent: parent.get(),
        child: child_flag.get(),
        child_lines: child.render(8),
    };
    let mut frames = vec![capture()];
    parent.set(true);
    frames.push(capture());
    child_flag.set(false);
    frames.push(capture());
    parent.set(false);
    frames.push(capture());
    replay.dialog.container().clear();
    parent.set(true);
    frames.push(capture());
    child_flag.set(false);
    frames.push(capture());
    parent.set(true);
    frames.push(capture());
    Expected::Focus { frames }
}
/// Choose only the specialized operation sequence recorded for this witness.
fn observe(replay: &Replay, case: &Case) -> Expected {
    match case.test.as_str() {
        "dialog_dispose_stops_ticks_without_settling_callbacks" => frames(
            replay,
            &[
                Operation::Advance {
                    advance: case.input.at.unwrap(),
                },
                Operation::Dispose { dispose: true },
                Operation::Dispose { dispose: true },
                Operation::Advance { advance: 3000 },
                Operation::Key("\r".into()),
                Operation::Key("\x1b".into()),
            ],
        ),
        "dialog_children_remain_editable_and_retained_targets_survive_removal" => children(replay),
        "input_focus_forwarding_keeps_parent_and_child_observations_separate" => focus(replay),
        "dialog_callbacks_allow_reentry_and_disposal" => {
            match case.input.phase {
                None => {
                    replay.dialog.key("\r");
                    advance(&replay.spy, 1000);
                }
                Some(Phase::Cancel) => {
                    replay.dialog.key("\x1b");
                    advance(&replay.spy, 2000);
                }
                Some(Phase::Expire) => {
                    advance(&replay.spy, 1000);
                    advance(&replay.spy, 2000);
                }
            }
            Expected::Frame(replay.frame())
        }
        _ if case.input.ops.is_empty() => Expected::Frame(replay.frame()),
        _ => frames(replay, &case.input.ops),
    }
}
/// Replays every attributed query, rejecting unread fields and duplicate scenarios.
pub fn group(name: &str) {
    let _admission = ADMISSION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = Restore(get_keybindings());
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/extension_dialogs.json")).unwrap();
    let raw: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../fixtures/extension_dialogs.json")).unwrap();
    let owners = [
        "selector_renders_bold_title_options_and_help",
        "input_renders_plain_title_editor_and_help",
        "selector_keeps_order_duplicates_and_empty_option_guard",
        "selector_clamps_navigation_and_keeps_literal_shortcuts",
        "dialogs_accept_lf_and_repeated_notifications",
        "dialogs_read_replaced_and_mutated_bindings",
        "dialog_key_collisions_preserve_source_precedence",
        "dialog_hints_capture_constructor_bindings",
        "input_accepts_but_does_not_render_placeholder",
        "input_delegates_editing_without_trimming_submission",
        "input_forwards_focus_to_cursor_marker",
        "input_focus_forwarding_keeps_parent_and_child_observations_separate",
        "dialog_colors_refresh_only_at_their_owning_operations",
        "dialog_countdown_requires_positive_timeout_and_writer",
        "dialog_countdown_reads_live_title_color_at_tick",
        "dialog_dispose_stops_ticks_without_settling_callbacks",
        "dialog_notifications_do_not_implicitly_stop_countdown",
        "dialog_children_remain_editable_and_retained_targets_survive_removal",
        "dialog_callbacks_allow_reentry_and_disposal",
        "dialog_constructor_propagates_hint_theme_failure",
    ];
    assert!(rows.iter().all(|row| owners.contains(&row.test.as_str())));
    assert!(
        owners
            .iter()
            .all(|name| rows.iter().any(|row| row.test == *name))
    );
    let mut identities = HashSet::new();
    for row in raw {
        assert!(identities.insert((row["test"].to_string(), row["input"].to_string())));
    }
    assert!(rows.iter().any(|row| row.test == name));
    for case in rows.into_iter().filter(|row| row.test == name) {
        let events = Rc::new(RefCell::new(vec![]));
        let result = replay(&case.input, name, events.clone());
        let observed = if let Ok(replay) = result {
            let observed = observe(&replay, &case);
            replay.dialog.dispose();
            assert_eq!(replay.spy.pending.get(), 0);
            observed
        } else {
            assert!(events.borrow().is_empty());
            Expected::Failure { failed: true }
        };
        assert_eq!(observed, case.expected);
    }
}

impl Component for Dialog {
    fn render(&self, width: usize) -> Vec<String> {
        self.component().render(width)
    }
    fn invalidate(&self) {
        self.component().invalidate();
    }
    fn input_handler(&self) -> Option<&dyn maestro_tui::tui::InputHandler> {
        self.component().input_handler()
    }
    fn focusable(&self) -> Option<&dyn maestro_tui::Focusable> {
        self.component().focusable()
    }
}
/// Run a native witness under the same global registry transaction owner.
pub fn native(run: impl FnOnce()) {
    let _admission = ADMISSION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = Restore(get_keybindings());
    set_keybindings(KeybindingsManager::new(KEYBINDINGS.clone(), vec![]));
    run();
}
/// Native construction uses the root exports and the same host configuration routes.
pub fn native_dialog(
    kind: Kind,
    theme: maestro_theme::LiveTheme,
    spy: &Spy,
    tui: Option<TUI>,
) -> Dialog {
    construct(
        kind,
        (Rc::new(|_| {}), Rc::new(|| {})),
        ExtensionSelectorOptions {
            tui,
            timeout: Some(1000.0),
            runtime: Rc::new(spy.clone()),
            theme,
        },
        ("Title".into(), vec!["Alpha".into(), "Beta".into()], None),
    )
    .unwrap()
}

/// The real startup selector in its containing writer, with the controlled runtime that
/// paints the writer's scheduled frames.
pub struct StartupHost {
    /// The containing writer.
    pub ui: TUI,
    /// The terminal the writer paints to.
    pub terminal: RecordingTerminal,
    /// The startup selector factory.
    pub factory: maestro_app::presentation_data::input_selectors::ExtensionSelectorFactory,
    /// The runtime that runs the writer's scheduled frames.
    pub runtime: Rc<super::manual_runtime::ManualRuntime>,
}

/// Installs the selector bindings the startup host is driven with.
fn bind_startup_keys() {
    let keys = KeybindingsManager::new(
        maestro_app::presentation_data::keybindings::KEYBINDINGS.clone(),
        vec![],
    );
    keys.set_user_bindings(vec![
        (
            "tui.select.down".into(),
            Some(maestro_tui::KeybindingKeys::Single("ctrl+n".into())),
        ),
        (
            "tui.select.confirm".into(),
            Some(maestro_tui::KeybindingKeys::Single("ctrl+y".into())),
        ),
        (
            "tui.select.cancel".into(),
            Some(maestro_tui::KeybindingKeys::Single("ctrl+x".into())),
        ),
    ]);
    set_keybindings(keys);
}

/// Construct the real startup component in its supplied containing writer.
pub fn startup_host() -> StartupHost {
    use super::{manual_runtime, published, recording_terminal};
    use maestro_app::presentation_data::input_selectors::{
        ExtensionSelectorFactory, PresentedSelector,
    };
    use maestro_theme::LiveTheme;
    bind_startup_keys();
    let runtime = Rc::new(manual_runtime::ManualRuntime::new());
    let terminal = recording_terminal::RecordingTerminal::new(80, 24);
    let ui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::default(),
        Some(false),
    );
    let live = LiveTheme::from_theme(published::theme(0));
    let actual_runtime = runtime.clone();
    let factory: ExtensionSelectorFactory = Rc::new(move |request| {
        let component = Rc::new(
            ExtensionSelectorComponent::new(
                request.title,
                request.options,
                request.on_select,
                request.on_cancel,
                ExtensionSelectorOptions {
                    tui: Some(request.tui),
                    timeout: None,
                    runtime: actual_runtime.clone(),
                    theme: live.clone(),
                },
            )
            .unwrap(),
        );
        Ok(PresentedSelector {
            component: component.clone(),
            focus: component,
        })
    });
    StartupHost {
        ui,
        terminal,
        factory,
        runtime,
    }
}
