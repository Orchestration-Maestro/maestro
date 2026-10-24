//! Strict recorded whole-frame session widget replay.
use maestro_tui::tui::InputHandler;
#[allow(
    dead_code,
    reason = "The library tests use only the override shape and its conversion."
)]
#[path = "recorded_keys/bindings.rs"]
pub(crate) mod bindings;
#[allow(
    dead_code,
    reason = "The controlled host supports other terminal effects."
)]
#[path = "../../../maestro-test-terminal/src/manual_runtime.rs"]
pub(crate) mod manual_runtime;
#[allow(dead_code, reason = "Only theme publication helpers are shared.")]
#[path = "../../../maestro-theme/tests/published/mod.rs"]
pub(crate) mod published;
#[allow(
    dead_code,
    reason = "Shared terminal support exposes controls used by other crates."
)]
#[path = "../../../maestro-test-terminal/src/recording_terminal.rs"]
pub(crate) mod recording_terminal;
#[path = "registry.rs"]
pub(crate) mod registry;
use bindings::Binding;
use maestro_app::presentation_data::keybindings::KEYBINDINGS;
use maestro_chat::{
    NameFilter, SessionScope, SessionSelectorHeader, SessionSelectorHeaderOptions, SortMode,
};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeOptions};
use maestro_tui::{Component, KeybindingsManager, get_keybindings, set_keybindings};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{cell::Cell, rc::Rc};
/// An attributed input and independent expected observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    input: Value,
    expected: Value,
}
/// Decodes all rows belonging to one witness, retaining strict typed payloads.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(name: &str) -> Vec<(I, E)> {
    decode_cases(include_str!("../fixtures/session-picker.json"), name)
}
/// Strict shared decoder for both session-picker corpora.
pub(crate) fn decode_cases<I: DeserializeOwned, E: DeserializeOwned>(
    text: &str,
    name: &str,
) -> Vec<(I, E)> {
    let all: Vec<Case> = serde_json::from_str(text).unwrap();
    let rows: Vec<_> = all
        .into_iter()
        .filter(|c| c.test == name)
        .map(|c| {
            (
                serde_json::from_value(c.input).unwrap(),
                serde_json::from_value(c.expected).unwrap(),
            )
        })
        .collect();
    assert!(!rows.is_empty());
    rows
}
/// Sentinel semantic palette matching the source recording.
fn palette(shift: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Accent, ColorValue::Index(32 + shift)),
                (ThemeColor::Muted, ColorValue::Index(33 + shift)),
                (ThemeColor::Dim, ColorValue::Index(34 + shift)),
                (ThemeColor::Error, ColorValue::Index(35 + shift)),
                (ThemeColor::Warning, ColorValue::Index(36 + shift)),
            ],
            [(ThemeBg::SelectedBg, ColorValue::Index(237 + shift))],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Convert only the source scope vocabulary.
fn scope(s: &str) -> SessionScope {
    match s {
        "current" => SessionScope::Current,
        "all" => SessionScope::All,
        _ => panic!("invalid scope"),
    }
}
/// Convert only the source sort vocabulary.
fn mode(s: &str) -> SortMode {
    match s {
        "threaded" => SortMode::Threaded,
        "recent" => SortMode::Recent,
        "relevance" => SortMode::Relevance,
        _ => panic!("invalid sort"),
    }
}
/// Convert only the source name vocabulary.
fn filter(s: &str) -> NameFilter {
    match s {
        "all" => NameFilter::All,
        "named" => NameFilter::Named,
        _ => panic!("invalid filter"),
    }
}
/// Strict constructor operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policies {
    scope: String,
    mode: String,
    filter: String,
}
/// Exact rendered rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lines {
    lines: Vec<String>,
}
/// Executes the attributed constructor through its public renderer.
pub fn header_constructor(name: &str) {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(get_keybindings());
    set_keybindings(manager(Vec::default()));
    let (_state, theme) = published::published(&palette(0));
    for (i, e) in cases::<Policies, Lines>(name) {
        let h = SessionSelectorHeader::new(
            scope(&i.scope),
            mode(&i.mode),
            filter(&i.filter),
            SessionSelectorHeaderOptions {
                theme: theme.clone(),
                runtime: manual_runtime::ManualRuntime::new().handle(),
                request_render: Rc::new(|| {}),
                styles_enabled: Rc::new(|| true),
            },
        );
        assert_eq!(h.render(120), e.lines);
    }
}

/// Installs recorded outer application actions without adding widget registration policy.
fn manager(overrides: maestro_tui::KeybindingsConfig) -> KeybindingsManager {
    let mut definitions = KEYBINDINGS.clone();
    for (id, key) in [("delete", "ctrl+d"), ("rename", "ctrl+r")] {
        definitions.push((
            format!("app.session.{id}"),
            maestro_tui::KeybindingDefinition {
                default_keys: maestro_tui::KeybindingKeys::Single(key.into()),
                description: None,
            },
        ));
    }
    KeybindingsManager::new(definitions, overrides)
}
/// Loading operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Loading {
    scope: String,
    progress: Option<[usize; 2]>,
}
/// Three observations of the same loading state.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadingFrames {
    loading: Vec<String>,
    cleared: Vec<String>,
    finished: Vec<String>,
}
/// Observes progress and repeated loading reset through the header.
pub fn header_loading(name: &str) {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(get_keybindings());
    set_keybindings(manager(Vec::default()));
    let (_state, theme) = published::published(&palette(0));
    for (i, e) in cases::<Loading, LoadingFrames>(name) {
        let h = SessionSelectorHeader::new(
            scope(&i.scope),
            SortMode::Threaded,
            NameFilter::All,
            SessionSelectorHeaderOptions {
                theme: theme.clone(),
                runtime: manual_runtime::ManualRuntime::new().handle(),
                request_render: Rc::new(|| {}),
                styles_enabled: Rc::new(|| true),
            },
        );
        h.set_loading(true);
        if let Some([loaded, total]) = i.progress {
            h.set_progress(loaded, total);
        }
        assert_eq!(h.render(120), e.loading);
        h.set_loading(true);
        assert_eq!(h.render(120), e.cleared);
        h.set_loading(false);
        assert_eq!(h.render(120), e.finished);
    }
}
/// Header operations recorded independently.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct HeaderInput {
    scope: Option<String>,
    mode: Option<String>,
    filter: Option<String>,
    path_on: Option<bool>,
    rename: Option<bool>,
    overrides: Option<std::collections::BTreeMap<String, Binding>>,
    #[serde(rename = "type")]
    kind: Option<String>,
    width: Option<usize>,
    state: Option<String>,
    delay: Option<u64>,
    next: Option<String>,
}
/// Recorded overrides, none when absent.
fn overrides(
    values: Option<&std::collections::BTreeMap<String, Binding>>,
) -> maestro_tui::KeybindingsConfig {
    values.map(bindings::bindings).unwrap_or_default()
}
/// Strict possible header observation fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct HeaderExpected {
    lines: Option<Vec<String>>,
    events: Option<Vec<String>>,
    normal: Option<Vec<String>>,
    confirmation: Option<Vec<String>>,
    status: Option<Vec<String>>,
    restored: Option<Vec<String>>,
    cleared: Option<Vec<String>>,
    before: Option<Vec<String>>,
    early: Option<HeaderFrame>,
    after: Option<HeaderFrame>,
    pending: Option<Vec<u64>>,
    old_boundary: Option<HeaderFrame>,
    before_new: Option<HeaderFrame>,
}
/// Timed header observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeaderFrame {
    lines: Vec<String>,
    events: Vec<String>,
}
/// Replays setters and rendering without scheduling deadlines of its own.
pub fn header(name: &str) {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(get_keybindings());
    for (i, e) in cases::<HeaderInput, HeaderExpected>(name) {
        HeaderScenario::new(name, i, e).run(name);
    }
}
/// One header operation and its controlled callbacks.
struct HeaderScenario {
    h: Rc<SessionSelectorHeader>,
    i: HeaderInput,
    e: HeaderExpected,
    runtime: manual_runtime::ManualRuntime,
    events: Rc<std::cell::RefCell<Vec<String>>>,
    _state: maestro_theme::ThemeState,
}
impl HeaderScenario {
    /// Retains the isolated theme and operation inputs.
    fn new(name: &str, i: HeaderInput, e: HeaderExpected) -> Self {
        set_keybindings(manager(Vec::default()));
        let (state, theme) = published::published(&palette(0));
        let runtime = manual_runtime::ManualRuntime::new();
        let events = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let log = events.clone();
        let policies = match name {
            "session_header_rename_hint_follows_configuration" => {
                (SessionScope::All, SortMode::Recent, NameFilter::Named)
            }
            "session_header_clips_three_rows_with_cell_width" => {
                (SessionScope::All, SortMode::Relevance, NameFilter::Named)
            }
            _ => (SessionScope::Current, SortMode::Threaded, NameFilter::All),
        };
        let h = SessionSelectorHeader::new(
            policies.0,
            policies.1,
            policies.2,
            SessionSelectorHeaderOptions {
                theme,
                runtime: runtime.handle(),
                request_render: Rc::new(move || log.borrow_mut().push("render".into())),
                styles_enabled: Rc::new(|| true),
            },
        );
        Self {
            h,
            i,
            e,
            runtime,
            events,
            _state: state,
        }
    }
    /// Dispatches one recorded operation and observes its resulting frame.
    fn run(&self, name: &str) {
        match name {
            "session_header_setters_keep_path_and_rename_hints_independent" => self.setters(),
            "session_header_rename_hint_follows_configuration" => self.rename(),
            "session_header_hints_read_live_binding_order" => self.bindings(),
            "session_header_confirmation_precedes_status_and_hints" => {
                self.priority();
                return;
            }
            "session_header_clips_three_rows_with_cell_width" => self.clipping(),
            "session_status_expires_only_at_nonzero_caller_delay" => {
                self.expiry();
                return;
            }
            "session_status_replacement_cancels_previous_callback" => {
                self.replacement();
                return;
            }
            _ => panic!("unknown header witness"),
        }
        if let Some(lines) = &self.e.lines {
            assert_eq!(self.h.render(self.i.width.unwrap_or(120)), *lines);
        }
        if let Some(lines) = &self.e.confirmation {
            assert_eq!(self.h.render(120), *lines);
        }
        if let Some(expected) = &self.e.events {
            assert_eq!(*self.events.borrow(), *expected);
        }
    }
    /// Replays the recorded setters operation.
    fn setters(&self) {
        let Self { h, i, .. } = self;
        h.set_scope(scope(i.scope.as_deref().unwrap()));
        h.set_sort_mode(mode(i.mode.as_deref().unwrap()));
        h.set_name_filter(filter(i.filter.as_deref().unwrap()));
        h.set_show_path(true);
        h.set_show_rename_hint(true);
        h.set_loading(true);
        h.set_progress(1, 2);
        h.set_loading(false);
        h.set_confirming_delete_path(None);
        h.set_status_message(None, None);
    }
    /// Replays the recorded rename operation.
    fn rename(&self) {
        let Self { h, i, .. } = self;
        h.set_show_path(i.path_on.unwrap());
        h.set_show_rename_hint(i.rename.unwrap());
    }
    /// Replays the recorded bindings operation.
    fn bindings(&self) {
        let Self { h, i, e, .. } = self;
        set_keybindings(manager(overrides(i.overrides.as_ref())));
        h.set_show_rename_hint(true);
        assert_eq!(h.render(120), *e.normal.as_ref().unwrap());
        h.set_confirming_delete_path(Some(String::new()));
    }
    /// Replays the recorded priority operation.
    fn priority(&self) {
        let Self { h, i, e, .. } = self;
        let text = "Status 界 e\u{301} 👩‍💻".into();
        let msg = if i.kind.as_deref() == Some("error") {
            maestro_chat::StatusMessage::Error(text)
        } else {
            maestro_chat::StatusMessage::Info(text)
        };
        h.set_status_message(Some(msg), None);
        assert_eq!(h.render(120), *e.status.as_ref().unwrap());
        h.set_confirming_delete_path(Some("/missing/a".into()));
        assert_eq!(h.render(120), *e.confirmation.as_ref().unwrap());
        h.set_confirming_delete_path(None);
        assert_eq!(h.render(120), *e.restored.as_ref().unwrap());
        h.set_status_message(None, None);
        assert_eq!(h.render(120), *e.cleared.as_ref().unwrap());
    }
    /// Replays the recorded clipping operation.
    fn clipping(&self) {
        let Self { h, i, .. } = self;
        h.set_show_rename_hint(true);
        match i.state.as_deref().unwrap() {
            "normal" => {}
            "loading" => {
                h.set_loading(true);
                h.set_progress(2, 3);
            }
            "status" => h.set_status_message(
                Some(maestro_chat::StatusMessage::Info(
                    "Status 界 e\u{301} 👩‍💻".repeat(5),
                )),
                None,
            ),
            "confirmation" => h.set_confirming_delete_path(Some("/missing/a".into())),
            _ => panic!("invalid state"),
        }
    }
    /// Replays the recorded expiry operation.
    fn expiry(&self) {
        let Self {
            h,
            i,
            e,
            runtime,
            events,
            ..
        } = self;
        h.set_status_message(
            Some(maestro_chat::StatusMessage::Info("visible".into())),
            i.delay.map(manual_runtime::ms),
        );
        assert_eq!(h.render(80), *e.before.as_ref().unwrap());
        advance(runtime, i.delay.unwrap_or(0).saturating_sub(1));
        assert_frame(h, events, e.early.as_ref().unwrap());
        advance(runtime, i.delay.unwrap_or(0).saturating_sub(1) + 1);
        assert_frame(h, events, e.after.as_ref().unwrap());
        assert_eq!(runtime.pending(), e.pending.as_ref().unwrap().len());
        for &delay in e.pending.as_ref().unwrap() {
            assert_eq!(runtime.next_deadline(), Some(manual_runtime::ms(delay)));
        }
    }
    /// Replays the recorded replacement operation.
    fn replacement(&self) {
        let Self {
            h,
            i,
            e,
            runtime,
            events,
            ..
        } = self;
        h.set_status_message(
            Some(maestro_chat::StatusMessage::Error("old".into())),
            Some(manual_runtime::ms(2000)),
        );
        advance(runtime, 1000);
        let next = i.next.as_deref().unwrap();
        h.set_status_message(
            (next != "clear").then(|| maestro_chat::StatusMessage::Info("new".into())),
            (next == "replace").then(|| manual_runtime::ms(3000)),
        );
        advance(runtime, 2000);
        assert_frame(h, events, e.old_boundary.as_ref().unwrap());
        advance(runtime, 3999);
        assert_frame(h, events, e.before_new.as_ref().unwrap());
        advance(runtime, 4000);
        assert_frame(h, events, e.after.as_ref().unwrap());
    }
}
/// Drive only due scheduled work on the same controlled operation.
fn advance(runtime: &manual_runtime::ManualRuntime, time: u64) {
    for result in runtime.advance_to(manual_runtime::ms(time)) {
        result.unwrap();
    }
}
/// Compare a timed complete frame and actual notifications.
fn assert_frame(
    h: &SessionSelectorHeader,
    events: &std::cell::RefCell<Vec<String>>,
    e: &HeaderFrame,
) {
    assert_eq!(h.render(80), e.lines);
    assert_eq!(*events.borrow(), e.events);
}
/// List age input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Age {
    now: f64,
    modified: f64,
}
/// A public selected-path and complete rendered-frame observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListFrame {
    lines: Vec<String>,
    selected: Option<String>,
}
/// Default record shared by recorded widget constructors.
fn session(id: &str) -> maestro_app::SessionInfo {
    maestro_app::SessionInfo {
        path: format!("/missing/{id}.jsonl"),
        id: maestro_app::Presence::Present(serde_json::json!(id)),
        cwd: String::new(),
        name: None,
        parent_session_path: maestro_app::Presence::Missing,
        created: 0.0,
        modified: 1_767_225_600_000.0,
        message_count: 1,
        first_message: "hello".into(),
        all_messages_text: "hello".into(),
    }
}
/// Checks elapsed-time display through the list renderer.
pub fn age(name: &str) {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(get_keybindings());
    set_keybindings(manager(Vec::default()));
    let (_state, theme) = published::published(&palette(0));
    for (i, e) in cases::<Age, ListFrame>(name) {
        let mut record = session("a");
        record.modified = i.modified;
        let list = maestro_chat::SessionList::new(
            vec![record],
            maestro_chat::SessionListOptions {
                runtime: manual_runtime::ManualRuntime::new().handle(),
                show_cwd: false,
                sort_mode: SortMode::Threaded,
                name_filter: NameFilter::All,
                current_session_file_path: None,
                theme: theme.clone(),
                now_ms: Rc::new(move || i.now),
                home: Rc::new(|| "/oracle-home".into()),
                styles_enabled: Rc::new(|| true),
            },
        );
        assert_eq!(list.render(80), e.lines);
        assert_eq!(list.get_selected_session_path(), e.selected);
    }
}
/// Strict recorded session metadata, preserving raw parent values.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Record {
    path: String,
    id: Value,
    cwd: String,
    modified: f64,
    created: Option<f64>,
    message_count: usize,
    first_message: String,
    all_messages_text: String,
    name: Option<String>,
    #[serde(default, deserialize_with = "present")]
    parent_session_path: Option<Value>,
}
/// Retains explicit null separately from absence.
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}
impl Record {
    /// Projects directly into the original owner, without narrowing parent metadata.
    fn owned(self) -> maestro_app::SessionInfo {
        maestro_app::SessionInfo {
            path: self.path,
            id: maestro_app::Presence::Present(self.id),
            cwd: self.cwd,
            modified: self.modified,
            message_count: self.message_count,
            first_message: self.first_message,
            all_messages_text: self.all_messages_text,
            name: self.name,
            parent_session_path: match self.parent_session_path {
                None => maestro_app::Presence::Missing,
                Some(Value::Null) => maestro_app::Presence::Null,
                Some(v) => maestro_app::Presence::Present(v),
            },
            created: self.created.unwrap_or(0.0),
        }
    }
}
/// Recorded live clock and home operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observations {
    now: f64,
    home: String,
}
/// Recorded list constructor and input operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ListInput {
    sessions: Option<Vec<Record>>,
    home: Option<String>,
    path: Option<String>,
    mode: Option<String>,
    filter: Option<String>,
    query: Option<String>,
    show_cwd: Option<bool>,
    show_path: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    name: Option<Value>,
    text: Option<String>,
    width: Option<usize>,
    count: Option<usize>,
    index: Option<usize>,
    ops: Option<Vec<String>>,
    selected: Option<usize>,
    current: Option<String>,
    confirmation: Option<String>,
    root: Option<String>,
    retarget: Option<String>,
    initial: Option<Observations>,
    next: Option<Observations>,
    overrides: Option<std::collections::BTreeMap<String, Binding>>,
    kind: Option<String>,
    level: Option<u8>,
}
/// Ordered callback slots observed through their public getters.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Slots {
    initial_select_absent: Flag,
    initial_cancel_absent: Flag,
    select_identity: Flag,
    cancel_identity: Flag,
    cleared_select: Flag,
    cleared_cancel: Flag,
}
/// Boolean fixture observation without a multi-boolean Rust record.
#[derive(Deserialize)]
#[serde(transparent)]
struct Flag(bool);
/// Frames may carry actual cumulative callbacks or the parent focus state.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    lines: Option<Vec<String>>,
    list: Option<Vec<String>>,
    selected: Option<String>,
    events: Option<Vec<Vec<String>>>,
    parent: Option<bool>,
    header: Option<Vec<String>>,
}
/// Independent parent focus observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parent {
    parent: bool,
}
/// Complete list operation result shapes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListExpected {
    lines: Option<Vec<String>>,
    selected: Option<String>,
    events: Option<Vec<Vec<String>>>,
    slots: Option<Slots>,
    frames: Option<Vec<Frame>>,
    filtered: Option<ListFrame>,
    replaced: Option<ListFrame>,
    named: Option<ListFrame>,
    all: Option<ListFrame>,
    before: Option<Frame>,
    after: Option<Frame>,
    invalidated: Option<Frame>,
    rebuild: Option<ListFrame>,
    independent: Option<Parent>,
    reapplied: Option<Frame>,
}
/// Checks the original authored identity separately from display text.
fn check(list: &maestro_chat::SessionList, width: usize, expected: &ListFrame) {
    assert_eq!(list.render(width), expected.lines);
    assert_eq!(list.get_selected_session_path(), expected.selected);
}
/// Live effects and owners shared by one scenario only.
pub struct World {
    state: maestro_theme::ThemeState,
    theme: maestro_theme::LiveTheme,
    runtime: manual_runtime::ManualRuntime,
    now: Rc<Cell<f64>>,
    home: Rc<std::cell::RefCell<String>>,
    styles: Rc<Cell<bool>>,
}
impl World {
    /// Creates isolated observations while registry admission belongs to the outer witness.
    fn new() -> Self {
        let (state, theme) = published::published(&palette(0));
        Self {
            state,
            theme,
            runtime: manual_runtime::ManualRuntime::new(),
            now: Rc::new(Cell::new(1_767_225_600_000.0)),
            home: Rc::new(std::cell::RefCell::new("/oracle-home".into())),
            styles: Rc::new(Cell::new(true)),
        }
    }
    /// Builds a list with independent policy operands and live effects.
    fn list(
        &self,
        records: Vec<maestro_app::SessionInfo>,
        policies: (bool, SortMode, NameFilter),
        current: Option<String>,
    ) -> Rc<maestro_chat::SessionList> {
        let now = self.now.clone();
        let home = self.home.clone();
        let styles = self.styles.clone();
        maestro_chat::SessionList::new(
            records,
            maestro_chat::SessionListOptions {
                runtime: self.runtime.handle(),
                show_cwd: policies.0,
                sort_mode: policies.1,
                name_filter: policies.2,
                current_session_file_path: current,
                theme: self.theme.clone(),
                now_ms: Rc::new(move || now.get()),
                home: Rc::new(move || home.borrow().clone()),
                styles_enabled: Rc::new(move || styles.get()),
            },
        )
    }
    /// Builds the header using the same live theme/style observations.
    fn header(&self) -> Rc<SessionSelectorHeader> {
        let styles = self.styles.clone();
        SessionSelectorHeader::new(
            SessionScope::Current,
            SortMode::Threaded,
            NameFilter::All,
            SessionSelectorHeaderOptions {
                theme: self.theme.clone(),
                runtime: self.runtime.handle(),
                request_render: Rc::new(|| {}),
                styles_enabled: Rc::new(move || styles.get()),
            },
        )
    }
}
/// Calls only the private render-state adapter supplied by library tests.
pub type Configure = fn(&maestro_chat::SessionList, bool, Option<&str>, Option<usize>);
/// Replays every recorded list case; private tests supply their existing state adapter.
pub fn list(name: &str, configure: Option<Configure>) {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(get_keybindings());
    for (i, e) in cases::<ListInput, ListExpected>(name) {
        replay_list(name, i, e, configure);
    }
}
/// Constructs records for generated-input corpus groups.
fn generated(name: &str, i: &ListInput) -> Vec<maestro_app::SessionInfo> {
    let record = |id: &str| session(id);
    match name {
        "session_list_setters_reapply_query_and_clamp_selection"
        | "session_row_style_precedence_and_selected_background" => {
            let mut a = record("a");
            a.name = Some("named".into());
            vec![a, record("b")]
        }
        "session_message_normalization_preserves_name_presence_and_control_rules" => {
            let mut a = record("a");
            a.name = i.name.as_ref().and_then(Value::as_str).map(str::to_owned);
            a.first_message = "fallback".into();
            vec![a]
        }
        "session_row_clipping_keeps_cells_and_final_ellipsis" => {
            let mut a = record("a");
            a.first_message = i.text.clone().unwrap();
            a.cwd = "/long/path".into();
            a.message_count = 12;
            vec![a]
        }
        "session_window_centers_ten_rows_and_reports_scroll_position"
        | "session_navigation_clamps_and_routes_confirm_cancel_search" => (0..i.count.unwrap())
            .map(|n| record(&format!("id-{n}")))
            .collect(),
        "session_list_routes_live_bindings_and_wrong_modifiers"
        | "session_navigation_binding_collision_prefers_up"
        | "session_callbacks_can_replace_clear_and_reenter" => vec![record("a"), record("b")],
        "session_render_reads_live_theme_and_invalidation_is_inert" => {
            let mut a = record("a");
            a.name = Some("named".into());
            vec![a]
        }
        "session_list_focus_emits_search_cursor_on_focus_changes"
        | "session_input_submit_uses_live_selected_path" => vec![record("a")],
        "session_paths_shorten_home_prefix_without_component_boundary" => {
            let mut a = record("a");
            a.cwd = i.path.clone().unwrap();
            vec![a]
        }
        "session_bold_decorations_read_live_style_policy" => {
            let mut a = record("a");
            match i.kind.as_deref().unwrap() {
                "empty" => a.name = Some(String::new()),
                "named" => a.name = Some("Named".into()),
                _ => {}
            }
            vec![a, record("other")]
        }
        _ => Vec::new(),
    }
}
/// Per-case operation transcript; comparisons retain order and repeated callbacks.
fn replay_list(name: &str, i: ListInput, e: ListExpected, configure: Option<Configure>) {
    let scenario = ListScenario::new(name, i, e);
    scenario.prepare(name, configure);
    scenario.run(name, configure);
}
/// Installs only recorded per-operation outer key overrides.
fn list_registry(name: &str, i: &ListInput) {
    let bindings = match name {
        "session_list_routes_live_bindings_and_wrong_modifiers" => {
            serde_json::json!({"tui.select.up":"k","tui.select.down":"j","tui.select.pageUp":"u","tui.select.pageDown":"v","tui.select.confirm":"x","tui.select.cancel":"q"})
        }
        "session_navigation_binding_collision_prefers_up" => {
            serde_json::json!({"tui.select.up":"x","tui.select.down":"x","tui.select.confirm":"x","tui.select.cancel":"x"})
        }
        "session_input_submit_uses_live_selected_path" => {
            serde_json::json!({"tui.input.submit":"x","tui.select.confirm":"y"})
        }
        _ => Value::Null,
    };
    let special = if bindings.is_null() {
        None
    } else {
        Some(
            serde_json::from_value::<std::collections::BTreeMap<String, Binding>>(bindings)
                .unwrap(),
        )
    };
    set_keybindings(manager(overrides(
        i.overrides.as_ref().or(special.as_ref()),
    )));
}
/// Keeps canonical lookup identity separate from authored current path.
fn current_path(name: &str, i: &ListInput, filesystem: Option<&Filesystem>) -> Option<String> {
    if name == "session_bold_decorations_read_live_style_policy" {
        match i.kind.as_deref() {
            Some("active") => Some("/missing/a.jsonl".into()),
            Some("empty_current") => Some(String::new()),
            _ => None,
        }
    } else {
        i.current
            .clone()
            .map(|p| filesystem.map_or(p.clone(), |fs| fs.expand(&p)))
    }
}
/// Uses each group's recorded viewport, not a test-owned limit.
fn list_width(name: &str, i: &ListInput) -> usize {
    i.width.unwrap_or(match name {
        "session_message_normalization_preserves_name_presence_and_control_rules"
        | "session_row_style_precedence_and_selected_background"
        | "session_window_centers_ten_rows_and_reports_scroll_position"
        | "session_callbacks_can_replace_clear_and_reenter"
        | "session_list_focus_emits_search_cursor_on_focus_changes"
        | "session_input_submit_uses_live_selected_path"
        | "session_navigation_binding_collision_prefers_up" => 80,
        "session_metadata_controls_are_displayed_without_terminal_commands" => 512,
        _ => 120,
    })
}
/// Retains source operands for native reconstruction while passing originals to the widget.
fn records(
    name: &str,
    i: &mut ListInput,
    filesystem: Option<&Filesystem>,
) -> Vec<maestro_app::SessionInfo> {
    match i.sessions.take() {
        None => generated(name, i),
        Some(records) => records
            .into_iter()
            .map(Record::owned)
            .map(|mut record| {
                if let Some(fs) = filesystem {
                    fs.record(&mut record);
                }
                record
            })
            .collect(),
    }
}
/// One invocation owns all host observations and its full output transcript.
struct ListScenario {
    i: ListInput,
    e: ListExpected,
    world: World,
    list: Rc<maestro_chat::SessionList>,
    filesystem: Option<Filesystem>,
    width: usize,
    events: Rc<std::cell::RefCell<Vec<Vec<String>>>>,
}
impl ListScenario {
    /// Owns constructor observations, with no ambient runner changes.
    fn new(name: &str, mut i: ListInput, e: ListExpected) -> Self {
        let world = World::new();
        if let Some(home) = &i.home {
            world.home.replace(home.clone());
        }
        if let Some(obs) = &i.initial {
            world.now.set(obs.now);
            world.home.replace(obs.home.clone());
        }
        set_keybindings(manager(Vec::default()));
        let filesystem = i.root.as_ref().map(|root| {
            assert_eq!(root, "$FS");
            Filesystem::new()
        });
        let records = records(name, &mut i, filesystem.as_ref());
        let cwd = i.show_cwd.unwrap_or(matches!(
            name,
            "session_paths_shorten_home_prefix_without_component_boundary"
                | "session_row_clipping_keeps_cells_and_final_ellipsis"
                | "session_date_and_home_observations_refresh_each_render"
                | "session_metadata_controls_are_displayed_without_terminal_commands"
        ));
        let current = current_path(name, &i, filesystem.as_ref());
        if let Some(level) = i.level {
            world.styles.set(level != 0);
        }
        let list = world.list(
            records,
            (
                cwd,
                i.mode.as_deref().map_or(SortMode::Threaded, mode),
                i.filter.as_deref().map_or(NameFilter::All, filter),
            ),
            current,
        );
        Self {
            width: list_width(name, &i),
            world,
            list,
            i,
            e,
            filesystem,
            events: Rc::new(std::cell::RefCell::new(Vec::new())),
        }
    }
    /// Applies only recorded render state and input actions before observation.
    fn prepare(&self, name: &str, configure: Option<Configure>) {
        let list = &self.list;
        let i = &self.i;
        if let Some(configure) = configure {
            configure(
                list,
                i.show_path.unwrap_or(matches!(
                    name,
                    "session_date_and_home_observations_refresh_each_render"
                        | "session_metadata_controls_are_displayed_without_terminal_commands"
                )),
                i.confirmation.as_deref(),
                i.selected,
            );
        }
        if let Some(query) = &i.query {
            list.handle_input(query);
        }
        for _ in 0..i.index.unwrap_or(0) {
            list.handle_input("\x1b[B");
        }
        if name == "session_navigation_binding_collision_prefers_up" {
            assert_eq!(
                list.get_selected_session_path().as_deref(),
                Some("/missing/b.jsonl")
            );
        }
        list_registry(name, i);
    }
    /// Dispatches and compares the actual ordered operation output.
    fn run(&self, name: &str, configure: Option<Configure>) {
        match name {
            "session_list_setters_reapply_query_and_clamp_selection" => self.setters(),
            "session_list_setters_retain_nonempty_search" => self.query(),
            "session_navigation_clamps_and_routes_confirm_cancel_search"
            | "session_list_routes_live_bindings_and_wrong_modifiers" => self.navigation(),
            "session_navigation_binding_collision_prefers_up"
            | "session_input_submit_uses_live_selected_path" => self.confirm(name),
            "session_callbacks_can_replace_clear_and_reenter" => self.slots(),
            "session_list_focus_emits_search_cursor_on_focus_changes" => focus(
                &self.list,
                self.e.frames.as_ref().unwrap(),
                self.e.independent.as_ref().unwrap(),
                self.e.reapplied.as_ref().unwrap(),
                configure.unwrap(),
            ),
            "session_render_reads_live_theme_and_invalidation_is_inert" => self.theme(),
            "session_canonical_aliases_thread_and_mark_active" if self.i.retarget.is_some() => {
                self.retarget();
            }
            "session_date_and_home_observations_refresh_each_render" => self.observations(),
            "session_bold_decorations_read_live_style_policy" => self.styles(),
            _ => {}
        }
        if let Some(lines) = &self.e.lines {
            let actual = normalize_list(&self.list, self.width, self.filesystem.as_ref());
            assert_eq!(actual.lines, *lines);
            assert_eq!(actual.selected, self.e.selected);
        }
        if let Some(events) = &self.e.events {
            assert_eq!(*self.events.borrow(), *events);
        }
    }
    /// Replays the attributed setters operation.
    fn setters(&self) {
        let Self { list, i, e, .. } = self;
        let width = self.width;
        list.handle_input("\x1b[B");
        list.set_sort_mode(mode(i.mode.as_deref().unwrap()));
        list.set_name_filter(filter(i.filter.as_deref().unwrap()));
        check(list, width, e.filtered.as_ref().unwrap());
        let mut c = session("c");
        c.cwd = "/oracle-home/project".into();
        c.name = Some("new".into());
        list.set_sessions(vec![c], true);
        check(list, width, e.replaced.as_ref().unwrap());
    }
    /// Replays the attributed query operation.
    fn query(&self) {
        let Self { list, e, .. } = self;
        let width = self.width;
        list.handle_input("\x1b[B");
        list.set_name_filter(NameFilter::Named);
        check(list, width, e.named.as_ref().unwrap());
        list.set_name_filter(NameFilter::All);
        list.set_sort_mode(SortMode::Recent);
        check(list, width, e.all.as_ref().unwrap());
        let mut c = session("c");
        c.all_messages_text = "no matching content".into();
        list.set_sessions(vec![c], true);
        check(list, width, e.replaced.as_ref().unwrap());
    }
    /// Replays the attributed navigation operation.
    fn navigation(&self) {
        let Self {
            list, i, e, events, ..
        } = self;

        callbacks(list, events);
        let ops = i.ops.as_ref().unwrap();
        let frames = e.frames.as_ref().unwrap();
        assert_eq!(frames.len(), ops.len());
        for (op, f) in ops.iter().zip(frames) {
            list.handle_input(op);
            check_frame(list, 80, f, events, None);
        }
    }
    /// Replays the attributed confirm operation.
    fn confirm(&self, name: &str) {
        let Self { list, events, .. } = self;

        callbacks(list, events);
        if name == "session_input_submit_uses_live_selected_path" {
            list.set_sessions(vec![session("b")], false);
        }
        list.handle_input("x");
    }
    /// Replays the attributed slots operation.
    fn slots(&self) {
        let Self {
            list, e, events, ..
        } = self;

        callback_slots(list, events, e.slots.as_ref().unwrap());
    }
    /// Replays the attributed theme operation.
    fn theme(&self) {
        let Self {
            list,
            e,
            events,
            world,
            ..
        } = self;

        let h = world.header();
        check_frame(list, 80, e.before.as_ref().unwrap(), events, Some(&h));
        world.state.set_theme_instance(palette(10)).unwrap();
        check_frame(list, 80, e.after.as_ref().unwrap(), events, Some(&h));
        h.invalidate();
        list.invalidate();
        check_frame(list, 80, e.invalidated.as_ref().unwrap(), events, Some(&h));
    }
    /// Replays the attributed retarget operation.
    fn retarget(&self) {
        let Self {
            list,
            i,
            e,
            filesystem,
            ..
        } = self;
        let width = self.width;
        let fs = filesystem.as_ref().unwrap();
        assert!(matches!(i.retarget.as_deref(), Some("current" | "record")));
        check_native_frame(list, width, e.before.as_ref().unwrap(), fs);
        fs.retarget();
        check_native_frame(list, width, e.after.as_ref().unwrap(), fs);
        let records = fs.saved_records.borrow_mut().take().unwrap();
        list.set_sessions(records, false);
        let actual = normalize_list(list, width, Some(fs));
        let expected = e.rebuild.as_ref().unwrap();
        assert_eq!(actual.lines, expected.lines);
        assert_eq!(actual.selected, expected.selected);
    }
    /// Replays the attributed observations operation.
    fn observations(&self) {
        let Self {
            list,
            i,
            e,
            world,
            events,
            ..
        } = self;
        let width = self.width;
        check_frame(list, width, e.before.as_ref().unwrap(), events, None);
        let next = i.next.as_ref().unwrap();
        world.now.set(next.now);
        world.home.replace(next.home.clone());
        check_frame(list, width, e.after.as_ref().unwrap(), events, None);
    }
    /// Replays the attributed styles operation.
    fn styles(&self) {
        let Self {
            list,
            e,
            world,
            events,
            ..
        } = self;
        let width = self.width;
        let h = world.header();
        check_frame(list, width, e.before.as_ref().unwrap(), events, Some(&h));
        world.styles.set(!world.styles.get());
        check_frame(list, width, e.after.as_ref().unwrap(), events, Some(&h));
    }
}
/// Observes actual callback events without borrowing any widget state.
fn callbacks(list: &maestro_chat::SessionList, events: &Rc<std::cell::RefCell<Vec<Vec<String>>>>) {
    let selected = events.clone();
    list.set_on_select(Some(Rc::new(move |path| {
        selected
            .borrow_mut()
            .push(vec!["select".into(), path.into()]);
    })));
    let cancelled = events.clone();
    list.set_on_cancel(Some(Rc::new(move || {
        cancelled.borrow_mut().push(vec!["cancel".into()]);
    })));
}
/// Compares each complete frame through its consuming renderer.
fn check_frame(
    list: &maestro_chat::SessionList,
    width: usize,
    f: &Frame,
    events: &std::cell::RefCell<Vec<Vec<String>>>,
    h: Option<&SessionSelectorHeader>,
) {
    assert_eq!(
        list.render(width),
        *f.lines.as_ref().or(f.list.as_ref()).unwrap()
    );
    if f.list.is_none() {
        assert_eq!(list.get_selected_session_path(), f.selected);
    }
    if let Some(e) = &f.events {
        assert_eq!(*events.borrow(), *e);
    }
    if let Some(parent) = f.parent {
        use maestro_tui::tui::Focusable;
        assert_eq!(list.focus_flag().get(), parent);
    }
    if let Some(header) = &f.header {
        assert_eq!(h.unwrap().render(120), *header);
    }
}
/// Exercises slot replacement, identity, clearing and bounded reentry.
fn callback_slots(
    list: &Rc<maestro_chat::SessionList>,
    events: &Rc<std::cell::RefCell<Vec<Vec<String>>>>,
    s: &Slots,
) {
    use maestro_tui::tui::InputHandler;
    assert_eq!(list.on_select().is_none(), s.initial_select_absent.0);
    assert_eq!(list.on_cancel().is_none(), s.initial_cancel_absent.0);
    let old = events.clone();
    list.set_on_select(Some(Rc::new(move |path| {
        old.borrow_mut()
            .push(vec!["old-select".into(), path.into()]);
    })));
    let old = events.clone();
    list.set_on_cancel(Some(Rc::new(move || {
        old.borrow_mut().push(vec!["old-cancel".into()]);
    })));
    let weak = Rc::downgrade(list);
    let selected = events.clone();
    let select: Rc<dyn Fn(&str)> = Rc::new(move |path| {
        selected
            .borrow_mut()
            .push(vec!["select".into(), path.into()]);
        let list = weak.upgrade().unwrap();
        list.set_on_select(None);
        list.set_sessions(vec![session("c")], true);
    });
    let weak = Rc::downgrade(list);
    let canceled = events.clone();
    let cancel: Rc<dyn Fn()> = Rc::new(move || {
        canceled.borrow_mut().push(vec!["cancel".into()]);
        weak.upgrade().unwrap().set_on_cancel(None);
    });
    list.set_on_select(Some(select.clone()));
    list.set_on_cancel(Some(cancel.clone()));
    assert_eq!(
        Rc::ptr_eq(&list.on_select().unwrap(), &select),
        s.select_identity.0
    );
    assert_eq!(
        Rc::ptr_eq(&list.on_cancel().unwrap(), &cancel),
        s.cancel_identity.0
    );
    for key in ["\r", "\r", "\x1b", "\x1b"] {
        list.handle_input(key);
    }
    assert_eq!(list.on_select().is_none(), s.cleared_select.0);
    assert_eq!(list.on_cancel().is_none(), s.cleared_cancel.0);
}
/// Parent forwarding is observed through actual Input cursor rows, not child introspection.
fn focus(
    list: &maestro_chat::SessionList,
    frames: &[Frame],
    independent: &Parent,
    reapplied: &Frame,
    configure: Configure,
) {
    use maestro_tui::tui::Focusable;
    assert_eq!(frames.len(), 3);
    for (value, f) in [false, true, false].into_iter().zip(frames) {
        list.focus_flag().set(value);
        assert_eq!(list.focus_flag().get(), f.parent.unwrap());
        assert_eq!(list.render(80), *f.lines.as_ref().unwrap());
    }
    list.focus_flag().set(true);
    configure(list, false, None, Some(usize::MAX));
    assert_eq!(list.focus_flag().get(), independent.parent);
    list.focus_flag().set(true);
    assert_eq!(list.focus_flag().get(), reapplied.parent.unwrap());
    assert_eq!(list.render(80), *reapplied.lines.as_ref().unwrap());
}
/// Disposable native path aliases used only by native corpus witnesses.
struct Filesystem {
    root: String,
    /// The disposable root that removes the padded tree.
    scratch: maestro_test_directory::Scratch,
    saved_records: std::cell::RefCell<Option<Vec<maestro_app::SessionInfo>>>,
}
impl Filesystem {
    /// Creates the fixture below the disposable root; the fixed width retains oracle padding.
    fn new() -> Self {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let parent = scratch.path().to_string_lossy().into_owned();
        let root = format!(
            "{parent}/{}",
            "x".repeat(100_usize.saturating_sub(parent.len() + 1))
        );
        assert_eq!(root.len(), 100);
        std::fs::create_dir(&root).unwrap();
        let fs = Self {
            root,
            scratch,
            saved_records: std::cell::RefCell::new(Some(Vec::new())),
        };
        for directory in ["real", "real-b"] {
            std::fs::create_dir(fs.expand(&format!("$FS/{directory}"))).unwrap();
            for file in ["parent", "child"] {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(fs.expand(&format!("$FS/{directory}/{file}.jsonl")))
                    .unwrap();
            }
        }
        fs.link("alias-a", "real");
        fs.link("alias-b", "real");
        fs
    }
    /// Real native symlinks exercise the resource owner, not a selector mock.
    ///
    /// Only Windows and Unix hosts create links. Every other host, the browser
    /// target included, resolves both paths and discards them.
    fn link(&self, alias: &str, target: &str) {
        let target = self.expand(&format!("$FS/{target}"));
        let alias = self.expand(&format!("$FS/{alias}"));
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, alias).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(target, alias).unwrap();
        #[cfg(not(any(unix, windows)))]
        let _ = (alias, target);
    }

    /// Replaces only the fixture root, preserving authored aliases and all metadata.
    fn expand(&self, value: &str) -> String {
        value.replace("$FS", &self.root)
    }
    /// Projects authored fixture paths into the native scratch root.
    fn record(&self, record: &mut maestro_app::SessionInfo) {
        record.path = self.expand(&record.path);
        if let maestro_app::Presence::Present(Value::String(parent)) =
            &mut record.parent_session_path
        {
            *parent = self.expand(parent);
        }
        if record.path.contains("metadata-") {
            let path = std::path::Path::new(&record.path);
            std::fs::create_dir(path.parent().unwrap()).unwrap();
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
        }
        self.saved_records
            .borrow_mut()
            .as_mut()
            .unwrap()
            .push(maestro_app::SessionInfo {
                path: record.path.clone(),
                id: record.id.clone(),
                cwd: record.cwd.clone(),
                modified: record.modified,
                message_count: record.message_count,
                first_message: record.first_message.clone(),
                all_messages_text: record.all_messages_text.clone(),
                name: record.name.clone(),
                parent_session_path: record.parent_session_path.clone(),
                created: record.created,
            });
    }
    /// Replaces one alias while current-session identity stays captured.
    fn retarget(&self) {
        std::fs::remove_file(self.expand("$FS/alias-b")).unwrap();
        self.link("alias-b", "real-b");
    }
}
impl Drop for Filesystem {
    fn drop(&mut self) {
        // The padded tree lies inside the disposable root, which removes it on drop.
        assert!(
            self.root
                .starts_with(self.scratch.path().to_string_lossy().as_ref())
        );
    }
}
/// Normalizes only the disposable native root, retaining every byte otherwise.
fn normalize_list(
    list: &maestro_chat::SessionList,
    width: usize,
    fs: Option<&Filesystem>,
) -> ListFrame {
    let normalize = |s: String| fs.map_or(s.clone(), |fs| s.replace(&fs.root, "$FS"));
    ListFrame {
        lines: list.render(width).into_iter().map(&normalize).collect(),
        selected: list.get_selected_session_path().map(normalize),
    }
}
/// Observes native alias retargeting while preserving full rendered frames.
fn check_native_frame(list: &maestro_chat::SessionList, width: usize, e: &Frame, fs: &Filesystem) {
    let actual = normalize_list(list, width, Some(fs));
    assert_eq!(actual.lines, *e.lines.as_ref().unwrap());
    assert_eq!(actual.selected, e.selected);
}
/// Last header owner revokes pending delivery and releases redraw captures.
pub fn status_drop() {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(get_keybindings());
    set_keybindings(manager(Vec::default()));
    let world = World::new();
    let capture = Rc::new(Cell::new(0));
    let weak_capture = Rc::downgrade(&capture);
    let header = SessionSelectorHeader::new(
        SessionScope::Current,
        SortMode::Threaded,
        NameFilter::All,
        SessionSelectorHeaderOptions {
            theme: world.theme.clone(),
            runtime: world.runtime.handle(),
            request_render: Rc::new(move || capture.set(capture.get() + 1)),
            styles_enabled: Rc::new(|| true),
        },
    );
    let weak = Rc::downgrade(&header);
    header.set_status_message(
        Some(maestro_chat::StatusMessage::Info("visible".into())),
        Some(manual_runtime::ms(2000)),
    );
    assert_eq!(world.runtime.pending(), 1);
    assert!(weak_capture.upgrade().is_some());
    drop(header);
    assert!(weak.upgrade().is_none());
    assert!(weak_capture.upgrade().is_none());
    assert_eq!(world.runtime.pending(), 0);
    assert!(
        world
            .runtime
            .advance_to(manual_runtime::ms(3000))
            .is_empty()
    );
}

/// Retained selector replay using the same palette, registry and host.
#[allow(
    dead_code,
    reason = "Scope witnesses run in their own integration target."
)]
pub mod session_picker_scopes;

/// Shared subprocess isolation for native picker observations.
#[cfg(unix)]
#[path = "../../../maestro-app/tests/support/child.rs"]
pub mod native_child;
