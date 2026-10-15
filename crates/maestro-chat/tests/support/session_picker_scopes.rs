//! Controlled discovery effects and complete independent selector observations.
use super::*;
use maestro_chat::{
    SessionListProgress, SessionSelectorCallbacks, SessionSelectorComponent,
    SessionSelectorOptions, SessionsLoader,
};
use maestro_tui::tui::{Focusable, InputHandler};
use std::cell::RefCell;
/// Complete rendered frame and actual callback transcript.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Frame {
    lines: Vec<String>,
    selected: Option<String>,
    events: Vec<Vec<Value>>,
}
/// Closed discovery producer counts.
#[derive(Default, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Calls {
    current: usize,
    all: usize,
}
/// One controlled producer result.
type DiscoveryResult = Result<Vec<maestro_app::SessionInfo>, Box<dyn std::error::Error>>;
/// A caller-controlled completion, retaining the supplied progress handle.
struct Load {
    sender: Option<tokio::sync::oneshot::Sender<DiscoveryResult>>,
    progress: SessionListProgress,
}
/// Closed set of loader factory effects.
#[derive(Default)]
struct Loader {
    loads: RefCell<Vec<Load>>,
}
impl Loader {
    /// Registers immediately when the factory is called, before polling.
    fn factory(self: &Rc<Self>) -> SessionsLoader {
        let owner = self.clone();
        Rc::new(move |progress| {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            owner.loads.borrow_mut().push(Load {
                sender: Some(sender),
                progress: progress.unwrap(),
            });
            Box::pin(async move { receiver.await.unwrap() })
        })
    }
    /// Settles exactly the most recently admitted call.
    fn finish(&self, result: Result<Vec<maestro_app::SessionInfo>, Box<dyn std::error::Error>>) {
        let sender = self
            .loads
            .borrow_mut()
            .last_mut()
            .unwrap()
            .sender
            .take()
            .unwrap();
        assert!(sender.send(result).is_ok());
    }
}
/// One selector and its independently controlled discovery ports.
struct Harness {
    selector: Rc<SessionSelectorComponent>,
    current: Rc<Loader>,
    all: Rc<Loader>,
    events: Rc<RefCell<Vec<Vec<Value>>>>,
    world: World,
}
impl Harness {
    /// Constructs with no loader already completed.
    fn new() -> Self {
        Self::configured(|_| {})
    }
    /// Configures constructor operands before any consuming operation.
    fn configured(configure: impl FnOnce(&mut SessionSelectorOptions)) -> Self {
        Self::build(configure, false)
    }
    /// Optionally inspects terminal notifications inside the callback boundary.
    fn build(configure: impl FnOnce(&mut SessionSelectorOptions), inspect: bool) -> Self {
        let world = World::new();
        let current = Rc::new(Loader::default());
        let all = Rc::new(Loader::default());
        let events = Rc::new(RefCell::new(Vec::new()));
        let holder = Rc::new(RefCell::new(
            None::<std::rc::Weak<SessionSelectorComponent>>,
        ));
        let callbacks = notifications(
            holder.clone(),
            events.clone(),
            world.runtime.clone(),
            inspect,
        );
        let mut options = SessionSelectorOptions {
            keybindings: manager(Vec::new()),
            show_rename_hint: false,
            current_session_file_path: None,
            theme: world.theme.clone(),
            runtime: world.runtime.handle(),
            request_render: Rc::new(|| {}),
            now_ms: Rc::new(|| 1_767_225_600_000.0),
            home: Rc::new(|| "/oracle-home".into()),
            paths: Rc::new(maestro_app::NativeResourceOperations),
            styles_enabled: Rc::new(|| true),
        };
        configure(&mut options);
        let selector =
            SessionSelectorComponent::new(current.factory(), all.factory(), callbacks, options);
        holder.replace(Some(Rc::downgrade(&selector)));
        Self {
            selector,
            current,
            all,
            events,
            world,
        }
    }
    /// Completes the standard Current result.
    fn current_default(&self) {
        self.finish("current", vec![named("current", "Current")]);
    }
    /// Completes the standard All result.
    fn all_default(&self) {
        let mut record = named("all", "All");
        record.cwd = "/other/project".into();
        self.finish("all", vec![record]);
    }
    /// Executes all currently admitted work without time-based waiting.
    fn drain(&self) {
        assert!(self.world.runtime.drain_futures().is_empty());
    }
    /// Settles one discovery port and completes its admitted future.
    fn finish(&self, scope: &str, records: Vec<maestro_app::SessionInfo>) {
        self.loader(scope).finish(Ok(records));
        self.drain();
    }
    /// Finds the controlled producer by its fixture scope.
    fn loader(&self, scope: &str) -> &Loader {
        match scope {
            "current" => &self.current,
            "all" => &self.all,
            _ => panic!("invalid scope"),
        }
    }
    /// Samples the entire rendered output and transcript.
    fn frame(&self, width: usize) -> Frame {
        Frame {
            lines: self.selector.render(width),
            selected: self.selector.get_session_list().get_selected_session_path(),
            events: self.events.borrow().clone(),
        }
    }
    /// Checks exactly the closed factory set, including the synchronous start.
    fn calls(&self, expected: &Calls) {
        assert_eq!(
            Calls {
                current: self.current.loads.borrow().len(),
                all: self.all.loads.borrow().len()
            },
            *expected
        );
    }
}
/// Strict fixture decoding retains every member of each group.
fn rows<I: DeserializeOwned, E: DeserializeOwned>(name: &str) -> Vec<(I, E)> {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/session-picker-scopes.json")).unwrap();
    let result: Vec<_> = cases
        .into_iter()
        .filter(|c| c.test == name)
        .map(|c| {
            (
                serde_json::from_value(c.input).unwrap(),
                serde_json::from_value(c.expected).unwrap(),
            )
        })
        .collect();
    assert!(!result.is_empty());
    result
}
/// One constructor population.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Size {
    size: usize,
}
/// Constructor, current completion and empty All completion.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Startup {
    initial: Frame,
    loaded: Frame,
    all: Frame,
    calls: Calls,
}
/// Witnesses synchronous factory admission and settled-empty cancellation.
pub fn initial() {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(get_keybindings());
    for (i, e) in rows::<Size, Startup>("session_scope_starts_current_and_cancels_only_empty_all") {
        set_keybindings(manager(Vec::new()));
        let h = Harness::new();
        assert_eq!(h.frame(120), e.initial);
        h.finish(
            "current",
            (0..i.size).map(|n| session(&format!("c{n}"))).collect(),
        );
        assert_eq!(h.frame(120), e.loaded);
        h.selector.handle_input("\t");
        h.finish("all", Vec::new());
        assert_eq!(h.frame(120), e.all);
        h.calls(&e.calls);
        assert_eq!(h.world.runtime.futures(), 0);
    }
}

/// Standard named record, independent of the production selector.
fn named(id: &str, name: &str) -> maestro_app::SessionInfo {
    let mut record = session(id);
    record.name = Some(name.into());
    record
}
/// No fixture operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
/// One complete observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Single {
    frame: Frame,
}
/// Runs one scenario with registry admission covering construction, polls and cleanup.
fn replay<I: DeserializeOwned, E: DeserializeOwned>(name: &str, run: impl Fn(I, E)) {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(get_keybindings());
    for (i, e) in rows::<I, E>(name) {
        set_keybindings(manager(Vec::new()));
        run(i, e);
    }
}

/// Chosen scope and source viewport.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    state: String,
    width: usize,
}
/// Composes the existing children without a second renderer.
pub fn layout() {
    replay(
        "session_selector_composes_existing_borders_and_three_row_header",
        |i: Layout, e: Single| {
            let h = Harness::new();
            h.current_default();
            if i.state == "all" {
                h.selector.handle_input("\t");
                h.all_default();
            } else {
                assert_eq!(i.state, "current");
            }
            assert_eq!(h.frame(i.width), e.frame);
        },
    );
}

/// Explicit focus sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FocusInput {
    focus: Vec<bool>,
}
/// Parent and child focus plus the emitted cursor.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FocusFrame {
    parent: bool,
    list: bool,
    frame: Frame,
}
/// Ordered focus observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FocusExpected {
    frames: Vec<FocusFrame>,
}
/// Observes independent flags and forwarding through the rendered cursor marker.
pub fn focus() {
    replay(
        "session_selector_focus_reaches_retained_search",
        |i: FocusInput, e: FocusExpected| {
            let h = Harness::new();
            h.current_default();
            assert_eq!(i.focus.len(), e.frames.len());
            for (value, f) in i.focus.into_iter().zip(e.frames) {
                h.selector.focus_flag().set(value);
                assert_eq!(h.selector.focus_flag().get(), f.parent);
                assert_eq!(h.selector.get_session_list().focus_flag().get(), f.list);
                assert_eq!(h.frame(80), f.frame);
            }
            h.selector.get_session_list().focus_flag().set(true);
            assert!(!h.selector.focus_flag().get());
        },
    );
}

/// Explicit help policy, including absence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Hint {
    show: Option<bool>,
}
/// Observes constructor policy through the complete rendered hints.
pub fn rename_hint() {
    replay(
        "session_selector_rename_hint_accepts_explicit_policy",
        |i: Hint, e: Single| {
            let h = Harness::configured(|o| o.show_rename_hint = i.show.unwrap_or(false));
            h.current_default();
            assert_eq!(h.frame(120), e.frame);
        },
    );
}

/// Whether All is reentered before settlement.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reenter {
    reenter: bool,
}
/// All pending and cached publication boundaries.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ScopeFrames {
    all_loading: Frame,
    returned: Frame,
    pending_again: Frame,
    settled: Frame,
    cached: Frame,
    calls: Calls,
}
/// Replays one scope transition across an in-flight completion.
fn pending(i: &Reenter, e: &ScopeFrames) {
    let h = Harness::new();
    h.current_default();
    h.selector.handle_input("\t");
    assert_eq!(h.frame(120), e.all_loading);
    h.selector.handle_input("\t");
    assert_eq!(h.frame(120), e.returned);
    if i.reenter {
        h.selector.handle_input("\t");
    }
    assert_eq!(h.frame(120), e.pending_again);
    h.all_default();
    assert_eq!(h.frame(120), e.settled);
    if !i.reenter {
        h.selector.handle_input("\t");
    }
    assert_eq!(h.frame(120), e.cached);
    h.calls(&e.calls);
}
/// Inactive completion preserves the current view and fills a reusable cache.
pub fn late() {
    replay(
        "session_scope_late_all_completion_keeps_current_choice",
        |i, e| pending(&i, &e),
    );
}

/// Pending reentry keeps loading visible and does not admit a second factory.
pub fn pending_once() {
    replay(
        "session_scope_pending_all_load_is_not_started_twice",
        |i, e| pending(&i, &e),
    );
}

/// Progress producer and visibility at delivery.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgressInput {
    scope: String,
    selected: bool,
}
/// Before, progress and settled publication.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgressExpected {
    before: Frame,
    after: Frame,
    settled: Frame,
    calls: Calls,
}
/// Only the visible scope may publish progress.
pub fn progress() {
    replay(
        "session_scope_progress_belongs_to_visible_load",
        |i: ProgressInput, e: ProgressExpected| {
            let h = Harness::new();
            if i.scope == "all" {
                h.current_default();
                h.selector.handle_input("\t");
            }
            if !i.selected {
                h.selector.handle_input("\t");
                if i.scope == "current" {
                    let mut record = named("all", "All");
                    record.cwd = "/other/project".into();
                    h.all.finish(Ok(vec![record]));
                }
            }
            assert_eq!(h.frame(120), e.before);
            let callback = h.loader(&i.scope).loads.borrow()[0].progress.clone();
            callback(2, 7);
            assert_eq!(h.frame(120), e.after);
            if i.scope == "current" {
                h.current_default();
            } else {
                h.all_default();
            }
            assert_eq!(h.frame(120), e.settled);
            h.calls(&e.calls);
        },
    );
}

/// All's independently selected population.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AllEmpty {
    all_empty: bool,
}
/// Both completion boundaries and return to Current.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct InactiveExpected {
    other_loaded: Frame,
    current_settled: Frame,
    current: Frame,
    calls: Calls,
}
/// Late Current results remain cached without changing the visible scope.
pub fn inactive_current() {
    replay(
        "session_scope_inactive_current_completion_only_updates_cache",
        |i: AllEmpty, e: InactiveExpected| {
            let h = Harness::new();
            h.selector.handle_input("\t");
            h.finish(
                "all",
                if i.all_empty {
                    vec![]
                } else {
                    vec![named("all", "All")]
                },
            );
            assert_eq!(h.frame(120), e.other_loaded);
            h.current_default();
            assert_eq!(h.frame(120), e.current_settled);
            h.selector.handle_input("\t");
            assert_eq!(h.frame(120), e.current);
            h.calls(&e.calls);
        },
    );
}

/// Failure producer and current visibility.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureInput {
    scope: String,
    active: bool,
}
/// Status before and at its scheduled expiry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FailureExpected {
    failed: Frame,
    before_expiry: Frame,
    expired: Frame,
    delays: Vec<u64>,
    calls: Calls,
}
/// Rejects a real admitted future without reconstructing the producer error.
fn fail(h: &Harness, scope: &str, message: &str) {
    h.loader(scope)
        .finish(Err(std::io::Error::other(message.to_owned()).into()));
    h.drain();
}
/// Active errors display for exactly the existing status delay; inactive errors stay silent.
pub fn failure() {
    replay(
        "session_scope_load_failure_obeys_scope_and_status_expiry",
        |i: FailureInput, e: FailureExpected| {
            let h = Harness::new();
            if i.scope == "all" {
                h.current_default();
                h.selector.handle_input("\t");
            }
            if !i.active {
                h.selector.handle_input("\t");
                if i.scope == "current" {
                    h.all_default();
                }
            }
            fail(&h, &i.scope, "fixture load failed");
            assert_eq!(h.frame(120), e.failed);
            assert_eq!(h.world.runtime.pending(), e.delays.len());
            for delay in e.delays {
                assert_eq!(
                    h.world.runtime.next_deadline(),
                    Some(manual_runtime::ms(delay))
                );
            }
            advance(&h.world.runtime, 3999);
            assert_eq!(h.frame(120), e.before_expiry);
            advance(&h.world.runtime, 4000);
            assert_eq!(h.frame(120), e.expired);
            h.calls(&e.calls);
        },
    );
}

/// Error and retried result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Retry {
    failed: Frame,
    retried: Frame,
    calls: Calls,
}
/// An error never becomes a successful empty cache.
pub fn retry() {
    replay(
        "session_scope_failed_all_load_can_retry",
        |_: Empty, e: Retry| {
            let h = Harness::new();
            h.current_default();
            h.selector.handle_input("\t");
            fail(&h, "all", "first failure");
            assert_eq!(h.frame(120), e.failed);
            h.selector.handle_input("\t");
            h.selector.handle_input("\t");
            h.all_default();
            assert_eq!(h.frame(120), e.retried);
            h.calls(&e.calls);
        },
    );
}
/// Original records and a sequence of user controls.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlsInput {
    command: String,
    query: String,
    sessions: Vec<Record>,
}
/// Ordered complete observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frames {
    frames: Vec<Frame>,
}
/// Reuses query, sorting and membership through actual list controls.
pub fn controls() {
    replay(
        "session_selector_controls_reuse_query_and_live_list_setters",
        |i: ControlsInput, e: Frames| {
            let h = Harness::new();
            h.finish(
                "current",
                i.sessions.into_iter().map(Record::owned).collect(),
            );
            if !i.query.is_empty() {
                h.selector.handle_input(&i.query);
            }
            let (key, count) = match i.command.as_str() {
                "sort" => ("\x13", 3),
                "named" => ("\x0e", 2),
                "path" => ("\x10", 2),
                _ => panic!("invalid command"),
            };
            assert_eq!(e.frames.len(), count + 1);
            assert_eq!(h.frame(120), e.frames[0]);
            for frame in &e.frames[1..] {
                h.selector.handle_input(key);
                assert_eq!(h.frame(120), *frame);
            }
        },
    );
}

/// Distinguishable records and retained query across scopes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryScopes {
    query: String,
    current: Vec<Record>,
    all: Vec<Record>,
}
/// Visible scope-specific filtered results.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryExpected {
    other: Frame,
    current: Frame,
}
/// Scope snapshots pass through the original query owner.
pub fn query_scope() {
    replay(
        "session_scope_switch_reapplies_retained_search",
        |i: QueryScopes, e: QueryExpected| {
            let h = Harness::new();
            h.finish(
                "current",
                i.current.into_iter().map(Record::owned).collect(),
            );
            h.selector.handle_input(&i.query);
            h.selector.handle_input("\t");
            h.finish("all", i.all.into_iter().map(Record::owned).collect());
            assert_eq!(h.frame(120), e.other);
            h.selector.handle_input("\t");
            assert_eq!(h.frame(120), e.current);
        },
    );
}
/// Source action identities and default control bytes.
fn command(s: &str) -> (&str, &str) {
    match s {
        "scope" => ("tui.input.tab", "\t"),
        "sort" => ("app.session.toggleSort", "\x13"),
        "named" => ("app.session.toggleNamedFilter", "\x0e"),
        "path" => ("app.session.togglePath", "\x10"),
        _ => panic!("invalid command"),
    }
}
/// One independently selected manager policy.
fn binding(id: &str, key: Option<&str>) -> maestro_tui::KeybindingsConfig {
    vec![(
        id.into(),
        Some(key.map_or_else(
            || maestro_tui::KeybindingKeys::Multiple(vec![]),
            |key| maestro_tui::KeybindingKeys::Single(key.into()),
        )),
    )]
}

/// Only one manager receives the tested key.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ManagerInput {
    command: String,
    chosen_manager: String,
}
/// Before and after consuming one live key.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagerExpected {
    before: Frame,
    after: Frame,
    calls: Calls,
}
/// Distinguishes both directions of all independent manager routes.
pub fn managers() {
    replay(
        "session_selector_global_and_supplied_bindings_stay_independent",
        |i: ManagerInput, e: ManagerExpected| {
            let (id, _) = command(&i.command);
            assert!(matches!(i.chosen_manager.as_str(), "global" | "supplied"));
            set_keybindings(manager(binding(
                id,
                (i.chosen_manager == "global").then_some("ctrl+x"),
            )));
            let h = Harness::configured(|o| {
                o.keybindings = manager(binding(
                    id,
                    (i.chosen_manager == "supplied").then_some("ctrl+x"),
                ));
            });
            h.current_default();
            assert_eq!(h.frame(120), e.before);
            h.selector.handle_input("\x18");
            if !h.all.loads.borrow().is_empty() {
                h.all_default();
            }
            assert_eq!(h.frame(120), e.after);
            h.calls(&e.calls);
        },
    );
}

/// One remapped control.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    command: String,
}
/// Wrong-modifier and exact-modifier observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlsExpected {
    frames: Vec<Frame>,
    calls: Calls,
}
/// Shared global remapping for modifier witnesses.
fn remapped() -> maestro_tui::KeybindingsConfig {
    [("scope", "ctrl+t"), ("sort", "ctrl+s"), ("path", "ctrl+p")]
        .into_iter()
        .flat_map(|(c, k)| binding(command(c).0, Some(k)))
        .collect()
}
/// Wrong modifier must not dispatch the underlying same-key control.
pub fn modifiers() {
    replay(
        "session_selector_remapped_controls_reject_wrong_modifiers",
        |i: Command, e: ControlsExpected| {
            set_keybindings(manager(remapped()));
            let h = Harness::new();
            h.current_default();
            assert_eq!(e.frames.len(), 2);
            let (wrong, right) = match i.command.as_str() {
                "scope" => ("\x1b[116;7u", "\x14"),
                "sort" => ("\x1b[115;7u", "\x13"),
                "named" => ("\x1b[110;7u", "\x0e"),
                "path" => ("\x1b[112;7u", "\x10"),
                _ => panic!("invalid command"),
            };
            h.selector.handle_input(wrong);
            assert_eq!(h.frame(120), e.frames[0]);
            h.selector.handle_input(right);
            if i.command == "scope" {
                h.all_default();
            }
            assert_eq!(h.frame(120), e.frames[1]);
            h.calls(&e.calls);
        },
    );
}

/// Winning prefix among colliding controls.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Winner {
    winner: String,
}
/// Complete winning result and closed producer counts.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Collision {
    frame: Frame,
    calls: Calls,
}
/// Ordered dispatch selects scope, then sort, then named, before path.
pub fn collisions() {
    replay(
        "session_selector_control_collisions_follow_dispatch_order",
        |i: Winner, e: Collision| {
            let config = [
                ("scope", if i.winner == "scope" { "ctrl+x" } else { "tab" }),
                (
                    "sort",
                    if i.winner == "named" {
                        "ctrl+s"
                    } else {
                        "ctrl+x"
                    },
                ),
                ("path", "ctrl+x"),
            ]
            .into_iter()
            .flat_map(|(c, k)| binding(command(c).0, Some(k)))
            .collect();
            set_keybindings(manager(config));
            let h = Harness::configured(|o| {
                o.keybindings = manager(binding(command("named").0, Some("ctrl+x")));
            });
            h.current_default();
            h.selector.handle_input("\x18");
            if i.winner == "scope" {
                h.all_default();
            }
            assert_eq!(h.frame(120), e.frame);
            h.calls(&e.calls);
        },
    );
}

/// Standalone callback replacement observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotExpected {
    initial: Vec<String>,
    absent: Vec<String>,
    after: Vec<String>,
    selected: Option<String>,
    events: Vec<Vec<Value>>,
}
/// Checks identity and clears a slot from its own invocation.
fn slot_pair(
    list: &Rc<maestro_chat::SessionList>,
    command: &str,
    events: &Rc<RefCell<Vec<Vec<Value>>>>,
) {
    let weak = Rc::downgrade(list);
    let log = events.clone();
    let name = command.to_owned();
    if command == "path" {
        assert!(list.on_toggle_path().is_none());
        let callback: Rc<dyn Fn(bool)> = Rc::new(move |value| {
            log.borrow_mut()
                .push(vec![name.clone().into(), value.into()]);
            weak.upgrade().unwrap().set_on_toggle_path(None);
        });
        let old: Rc<dyn Fn(bool)> = Rc::new(|_| panic!("replaced callback"));
        list.set_on_toggle_path(Some(old));
        list.set_on_toggle_path(Some(callback.clone()));
        assert!(Rc::ptr_eq(&list.on_toggle_path().unwrap(), &callback));
    } else {
        let (get, set): SlotAccess = match command {
            "scope" => (
                maestro_chat::SessionList::on_toggle_scope,
                maestro_chat::SessionList::set_on_toggle_scope,
            ),
            "sort" => (
                maestro_chat::SessionList::on_toggle_sort,
                maestro_chat::SessionList::set_on_toggle_sort,
            ),
            "named" => (
                maestro_chat::SessionList::on_toggle_name_filter,
                maestro_chat::SessionList::set_on_toggle_name_filter,
            ),
            _ => panic!("invalid control"),
        };
        assert!(get(list).is_none());
        let callback: Rc<dyn Fn()> = Rc::new(move || {
            log.borrow_mut()
                .push(vec![name.clone().into(), Value::Null]);
            set(&weak.upgrade().unwrap(), None);
        });
        set(list, Some(Rc::new(|| panic!("replaced callback"))));
        set(list, Some(callback.clone()));
        assert!(Rc::ptr_eq(&get(list).unwrap(), &callback));
    }
}
/// Optional slots consume controls even when absent and remain reentrant.
pub fn slots() {
    replay(
        "session_list_control_slots_replace_clear_and_reenter",
        |i: Command, e: SlotExpected| {
            let world = World::new();
            let list = world.list(
                vec![session("a")],
                (false, SortMode::Threaded, NameFilter::All),
                None,
            );
            let events = Rc::new(RefCell::new(Vec::new()));
            (list.on_exit())();
            let exit: Rc<dyn Fn()> = Rc::new(|| {});
            list.set_on_exit(exit.clone());
            assert!(Rc::ptr_eq(&list.on_exit(), &exit));
            assert_eq!(list.render(80), e.initial);
            let (_, key) = command(&i.command);
            list.handle_input(key);
            assert_eq!(list.render(80), e.absent);
            slot_pair(&list, &i.command, &events);
            list.handle_input(key);
            list.handle_input(key);
            assert_eq!(list.render(80), e.after);
            assert_eq!(list.get_selected_session_path(), e.selected);
            assert_eq!(*events.borrow(), e.events);
            assert!(list.on_toggle_scope().is_none());
            assert!(list.on_toggle_sort().is_none());
            assert!(list.on_toggle_name_filter().is_none());
            assert!(list.on_toggle_path().is_none());
        },
    );
}

/// Terminal callback capability to invoke.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    action: String,
}
/// Failure before delivery and callback-visible cleared state afterward.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalExpected {
    failed: Frame,
    after: Frame,
    delays: Vec<u64>,
}
/// Terminal delivery sees cleared status and cancelled timers, not just a later frame.
pub fn terminal() {
    replay(
        "session_selector_terminal_callbacks_clear_status_before_delivery",
        |i: Action, e: TerminalExpected| {
            let h = Harness::build(|_| {}, true);
            h.current_default();
            h.selector.handle_input("\t");
            fail(&h, "all", "fixture load failed");
            assert_eq!(h.frame(120), e.failed);
            assert_eq!(h.world.runtime.pending(), 1);
            match i.action.as_str() {
                "exit" => (h.selector.get_session_list().on_exit())(),
                "confirm" => h.selector.handle_input("\r"),
                "cancel" => h.selector.handle_input("\x1b"),
                _ => panic!("invalid action"),
            }
            assert_eq!(h.world.runtime.pending(), 0);
            advance(&h.world.runtime, 4000);
            assert_eq!(h.frame(120), e.after);
            assert_eq!(h.world.runtime.pending(), e.delays.len());
            assert!(e.delays.is_empty());
        },
    );
}

/// Successful completions in both orders and visible scopes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EmptyInput {
    order: String,
    current_empty: bool,
    visible: String,
    replace_cancel: Option<bool>,
}
/// Every completion boundary remains observable.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct States {
    states: Vec<Frame>,
    calls: Calls,
}
/// Source completion orders, rejecting other fixture values.
fn order(s: &str) -> [&str; 2] {
    match s {
        "all-first" => ["all", "current"],
        "current-first" => ["current", "all"],
        _ => panic!("invalid order"),
    }
}
/// Requires settled successful empty records, not unknown or failed discovery.
pub fn empty_cancel() {
    replay(
        "session_scope_empty_cancel_uses_settled_records_in_both_orders",
        |i: EmptyInput, e: States| {
            let h = Harness::new();
            h.selector.handle_input("\t");
            if i.visible == "current" {
                h.selector.handle_input("\t");
            } else {
                assert_eq!(i.visible, "all");
            }
            if i.replace_cancel.unwrap_or(false) {
                let events = h.events.clone();
                h.selector
                    .get_session_list()
                    .set_on_cancel(Some(Rc::new(move || {
                        events.borrow_mut().push(vec!["replacement-cancel".into()]);
                    })));
            }
            assert_eq!(e.states.len(), 2);
            for (scope, frame) in order(&i.order).into_iter().zip(e.states) {
                h.finish(
                    scope,
                    if scope == "all" || i.current_empty {
                        vec![]
                    } else {
                        vec![session("c")]
                    },
                );
                assert_eq!(h.frame(120), frame);
            }
            h.calls(&e.calls);
            // A cache selection never redelivers automatic cancellation.
            let before = h.events.borrow().clone();
            h.selector.handle_input("\t");
            h.selector.handle_input("\t");
            assert_eq!(*h.events.borrow(), before);
        },
    );
}

/// Authored active identity passed through the constructor.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CurrentIdentity {
    current_path: String,
}
/// Active styling comes from constructor forwarding, not a setter.
pub fn identity() {
    replay(
        "session_selector_forwards_current_identity_to_list",
        |i: CurrentIdentity, e: Single| {
            let h = Harness::configured(|o| o.current_session_file_path = Some(i.current_path));
            h.current_default();
            assert_eq!(h.frame(120), e.frame);
        },
    );
}

/// Ordering with a failed Current and successful empty All.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Order {
    order: String,
}
/// Failed discovery cannot satisfy successful-empty cancellation.
pub fn failed_empty() {
    replay(
        "session_scope_load_failure_is_not_an_empty_record",
        |i: Order, e: States| {
            let h = Harness::new();
            h.selector.handle_input("\t");
            assert_eq!(e.states.len(), 2);
            for (scope, frame) in order(&i.order).into_iter().zip(e.states) {
                if scope == "all" {
                    h.finish("all", vec![]);
                } else {
                    fail(&h, "current", "current unavailable");
                }
                assert_eq!(h.frame(120), frame);
            }
            h.calls(&e.calls);
        },
    );
}

/// Constructor retains the original live manager rather than copying its configuration.
pub fn manager_identity() {
    replay(
        "session_selector_keeps_supplied_manager_identity",
        |_: Empty, e: Single| {
            let supplied = manager(binding(command("named").0, Some("ctrl+x")));
            let h = Harness::configured(|o| o.keybindings = supplied.clone());
            h.current_default();
            supplied.set_user_bindings(binding(command("named").0, Some("ctrl+n")));
            h.selector.handle_input("\x0e");
            assert_eq!(h.frame(120), e.frame);
        },
    );
}

/// Editable child array and fallible renderer observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContainerExpected {
    appended: Vec<String>,
    cleared: Vec<String>,
    invalidated: usize,
    error: String,
}
/// Caller-supplied child with visible output, invalidation and failure.
struct Child {
    invalidations: Rc<Cell<usize>>,
    fail: bool,
}
impl Component for Child {
    fn render(&self, _: usize) -> Vec<String> {
        vec!["caller child".into()]
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        if self.fail {
            Err(std::io::Error::other("caller render failed").into())
        } else {
            Ok(self.render(width))
        }
    }
    fn invalidate(&self) {
        self.invalidations.set(self.invalidations.get() + 1);
    }
}
/// Container editing retains old aliases and delegates the fallible child boundary.
pub fn container() {
    replay(
        "session_selector_keeps_container_aliases_and_fallible_delegation",
        |_: Empty, e: ContainerExpected| {
            let h = Harness::new();
            h.current_default();
            let old = h.selector.children();
            let count = Rc::new(Cell::new(0));
            let child = Rc::new(Child {
                invalidations: count.clone(),
                fail: false,
            });
            h.selector.add_child(child.clone());
            h.selector.invalidate();
            assert_eq!(h.selector.render(120), e.appended);
            assert_eq!(h.selector.try_render(120).unwrap(), e.appended);
            assert_eq!(count.get(), e.invalidated);
            h.selector.clear();
            old.borrow_mut().push(child);
            assert_eq!(h.selector.render(120), e.cleared);
            assert_eq!(old.borrow().len(), 10);
            h.selector.add_child(Rc::new(Child {
                invalidations: count,
                fail: true,
            }));
            assert_eq!(h.selector.render(120), vec!["caller child"]);
            assert_eq!(h.selector.try_render(120).unwrap_err().to_string(), e.error);
        },
    );
}

/// Surviving list's visible state and original callbacks.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Retained {
    lines: Vec<String>,
    selected: Option<String>,
    events: Vec<Vec<Value>>,
    calls: Calls,
}
/// List aliases retain discovery while the state has only a weak edge back.
pub fn retained() {
    replay(
        "session_selector_retained_list_owns_callback_state",
        |_: Empty, e: Retained| {
            let capture = Rc::new(Cell::new(0));
            let weak_capture = Rc::downgrade(&capture);
            let h = Harness::configured(|o| {
                o.request_render = Rc::new(move || capture.set(capture.get() + 1));
            });
            let Harness {
                selector,
                current,
                all,
                events,
                world,
            } = h;
            let list = selector.get_session_list();
            let weak_list = Rc::downgrade(&list);
            drop(selector);
            current.finish(Ok(vec![named("current", "Current")]));
            assert!(world.runtime.drain_futures().is_empty());
            list.handle_input("\r");
            list.handle_input("\t");
            let mut record = named("all", "All");
            record.cwd = "/other/project".into();
            all.finish(Ok(vec![record]));
            assert!(world.runtime.drain_futures().is_empty());
            list.handle_input("\r");
            assert_eq!(list.render(120), e.lines);
            assert_eq!(list.get_selected_session_path(), e.selected);
            assert_eq!(*events.borrow(), e.events);
            assert_eq!(
                Calls {
                    current: current.loads.borrow().len(),
                    all: all.loads.borrow().len()
                },
                e.calls
            );
            assert_eq!(world.runtime.futures(), 0);
            assert!(weak_capture.upgrade().is_some());
            drop(list);
            assert!(weak_list.upgrade().is_none());
            assert!(weak_capture.upgrade().is_none());
            assert_eq!(world.runtime.pending(), 0);
        },
    );
}

/// Exact application definition fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    id: String,
    keys: Vec<String>,
    description: String,
}
/// Only the newly supplied application rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definitions {
    definitions: Vec<Definition>,
}
/// The application owns functional defaults and descriptions, not widget fixtures.
pub fn definitions() {
    replay(
        "session_scope_action_definitions_keep_default_keys_and_descriptions",
        |_: Empty, e: Definitions| {
            let manager = KeybindingsManager::new(KEYBINDINGS.clone(), vec![]);
            assert_eq!(e.definitions.len(), 3);
            let session_actions: Vec<_> = manager
                .get_resolved_bindings()
                .into_iter()
                .filter(|(id, _)| id.starts_with("app.session."))
                .map(|(id, _)| id)
                .collect();
            assert_eq!(
                session_actions,
                [
                    "app.session.toggleNamedFilter",
                    "app.session.togglePath",
                    "app.session.toggleSort",
                ]
            );
            for row in e.definitions {
                assert_eq!(manager.get_keys(&row.id), row.keys);
                assert_eq!(
                    manager.get_definition(&row.id).unwrap().description,
                    Some(row.description)
                );
            }
        },
    );
}

/// Observes notification state inside the caller boundary, with a weak outer edge.
fn notifications(
    holder: Rc<RefCell<Option<std::rc::Weak<SessionSelectorComponent>>>>,
    events: Rc<RefCell<Vec<Vec<Value>>>>,
    runtime: manual_runtime::ManualRuntime,
    inspect: bool,
) -> SessionSelectorCallbacks {
    let emit_events = events;
    let emit_holder = holder;

    let emit = Rc::new(move |action: &str, path: Option<&str>| {
        let mut event = vec![Value::from(action)];
        if let Some(path) = path {
            event.push(path.into());
        }
        if inspect {
            let selector = emit_holder.borrow().as_ref().unwrap().upgrade().unwrap();
            event.push(
                selector
                    .render(120)
                    .iter()
                    .any(|line| line.contains("Failed to load sessions:"))
                    .into(),
            );
            event.push(runtime.pending().into());
        }
        emit_events.borrow_mut().push(event);
    });
    let select = emit.clone();
    let cancel = emit.clone();
    let exited = emit;
    SessionSelectorCallbacks {
        on_select: Rc::new(move |path| select("select", Some(path))),
        on_cancel: Rc::new(move || cancel("cancel", None)),
        on_exit: Rc::new(move || exited("exit", None)),
    }
}

/// Getter and setter pair for independently replaceable no-argument slots.
type SlotAccess = (
    fn(&maestro_chat::SessionList) -> Option<Rc<dyn Fn()>>,
    fn(&maestro_chat::SessionList, Option<Rc<dyn Fn()>>),
);
