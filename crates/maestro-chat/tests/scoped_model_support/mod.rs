//! Strict scope recordings with ordered callback and host observations.
use crate::{
    bindings::{self, Binding},
    manual_runtime::ManualRuntime,
    model_selector_support::Descriptor,
    published,
};
use maestro_chat::components::{ModelsCallbacks, ModelsConfig, ScopedModelsSelectorComponent};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions, ThemeState};
use maestro_tui::{
    Component, Focusable,
    tui::{InputHandler, LocalFuture},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
    sync::{Arc, RwLock},
};
/// Supplied constructor and subsequent actions.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Query {
    /// Optional ordered selection.
    pub enabled: Option<Vec<String>>,
    /// Supplied shared descriptors.
    #[serde(default = "basic")]
    pub models: Vec<Descriptor>,
    /// Ordered actions.
    #[serde(default)]
    pub ops: Vec<Operation>,
    /// Mutate callback-owned snapshots.
    #[serde(default)]
    pub mutate_callback: bool,
    /// Callback kind to reenter once.
    pub reenter: Option<String>,
    /// Callback kind returning asynchronous work.
    #[serde(rename = "async")]
    pub asynchronous: Option<String>,
    /// Initial overrides.
    #[serde(default)]
    pub bindings: BTreeMap<String, Binding>,
    /// Observe both independent focus flags.
    #[serde(default)]
    pub focus: bool,
    /// Frame width.
    #[serde(default = "width")]
    pub width: usize,
}
/// Default three distinguishable descriptors.
fn basic() -> Vec<Descriptor> {
    [("p", "a", "One"), ("p", "b", "Two"), ("q", "c", "Three")]
        .into_iter()
        .map(|(provider, id, name)| Descriptor {
            provider: provider.into(),
            id: id.into(),
            name: name.into(),
        })
        .collect()
}
/// Default observation width.
fn width() -> usize {
    120
}
/// Raw input or structured owner operation.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Operation {
    /// Selector input.
    Key(String),
    /// Structured operation.
    Action(Action),
}
/// Operations on the public retained owners.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// Direct text edit.
    SetSearch(String),
    /// Direct Input dispatch.
    Input(String),
    /// Parent focus change.
    Focus(bool),
    /// Child focus change.
    InputFocus(bool),
    /// Live palette replacement.
    Theme(u8),
    /// Live binding replacement.
    Bindings(BTreeMap<String, Binding>),
    /// Edit the original supplied descriptor.
    MutateModel(Mutation),
    /// Clear inherited children.
    ClearChildren(bool),
    /// Release every controlled asynchronous tail.
    Complete(bool),
}
/// Descriptor edit with original occurrence identity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mutation {
    /// Supplied position.
    index: usize,
    /// New metadata.
    fields: crate::model_selector_support::Changes,
}
/// Full observable frame, preserving order and exact bytes.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// Full styled lines.
    lines: Vec<String>,
    /// Ordered notifications and payloads.
    events: Vec<Vec<Value>>,
    /// Independently observed focus flags.
    focus: Option<[bool; 2]>,
}
/// Complete successful trace.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Reference completion category.
    branch: String,
    /// Frames surrounding every action.
    frames: Vec<Frame>,
}
/// Named independently recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Owning test.
    test: String,
    /// Operands.
    input: Query,
    /// Observations.
    expected: Expected,
    /// Complete macOS layout when its key names differ.
    expected_macos: Option<Expected>,
}
/// Exact role palette supplied by the recording host.
fn colors(shift: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Border, 31),
                (ThemeColor::Accent, 32),
                (ThemeColor::Muted, 33),
                (ThemeColor::Success, 34),
                (ThemeColor::Dim, 35),
                (ThemeColor::Warning, 36),
            ]
            .into_iter()
            .map(|(key, index)| (key, ColorValue::Index(index + shift))),
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Caller observations without a strong path back into the component.
#[derive(Default)]
pub struct Observations {
    /// Complete ordered transcript.
    pub events: RefCell<Vec<Vec<Value>>>,
    /// Non-owning reentry target.
    slot: RefCell<Weak<ScopedModelsSelectorComponent>>,
    /// Single reentry admission.
    reentered: Cell<bool>,
    /// Completion signal for pending work.
    complete: Cell<bool>,
}
impl Observations {
    /// Invokes immediate effects before returning independently owned work.
    fn callback(
        self: &Rc<Self>,
        kind: &'static str,
        mut value: Option<Vec<String>>,
        query: &Query,
    ) -> Option<LocalFuture> {
        self.events
            .borrow_mut()
            .push(vec![kind.into(), json!(value)]);
        if query.mutate_callback
            && let Some(ids) = &mut value
        {
            ids.push("callback-only".into());
        }
        if query.reenter.as_deref() == Some(kind) && !self.reentered.replace(true) {
            let target = self.slot.borrow().upgrade().unwrap();
            target.handle_input("\r");
            let lines = target.render(query.width);
            self.events
                .borrow_mut()
                .push(vec!["reentrant-render".into(), json!(lines)]);
        }
        if query.asynchronous.as_deref() != Some(kind) {
            return None;
        }
        let observed = self.clone();
        Some(Box::pin(std::future::poll_fn(move |_| {
            if !observed.complete.get() {
                return std::task::Poll::Pending;
            }
            observed
                .events
                .borrow_mut()
                .push(vec![format!("{kind}-done").into(), json!(value)]);
            std::task::Poll::Ready(Ok(()))
        })))
    }
}
/// Retained component transaction and its supplied owners.
pub struct Fixture {
    /// Component under test.
    pub selector: Rc<ScopedModelsSelectorComponent>,
    /// Controlled future admission owner.
    pub runtime: ManualRuntime,
    /// Caller transcript.
    pub observed: Rc<Observations>,
    /// Live palette publisher.
    theme: ThemeState,
    /// Original model occurrence handles.
    models: Vec<Arc<RwLock<maestro_app::Model>>>,
    /// Callback release witness.
    pub release: Weak<()>,
}
impl Fixture {
    /// Builds a controlled public component with no external registry effects.
    pub fn new(mut query: Query) -> Self {
        bindings::replace(&query.bindings);
        let models: Vec<_> = query.models.iter().map(Descriptor::handle).collect();
        let config = ModelsConfig {
            all_models: models.clone(),
            enabled_model_ids: query.enabled.take(),
        };
        let (theme, live) = published::published(&colors(0));
        let runtime = ManualRuntime::new();
        let observed = Rc::new(Observations::default());
        let query = Rc::new(query);
        let (change, persist, cancel) = (observed.clone(), observed.clone(), observed.clone());
        let (cq, pq) = (query.clone(), query);
        let release = Rc::new(());
        let weak = Rc::downgrade(&release);
        let selector = Rc::new(ScopedModelsSelectorComponent::new(
            config,
            ModelsCallbacks {
                on_change: Rc::new(move |value| {
                    assert_eq!(Rc::strong_count(&release), 1);
                    change.callback("change", value, &cq)
                }),
                on_persist: Rc::new(move |value| persist.callback("persist", value, &pq)),
                on_cancel: Rc::new(move || cancel.events.borrow_mut().push(vec!["cancel".into()])),
            },
            live,
            runtime.handle(),
        ));
        *observed.slot.borrow_mut() = Rc::downgrade(&selector);
        Self {
            selector,
            runtime,
            observed,
            theme,
            models,
            release: weak,
        }
    }
    /// Observes complete public output without normalization.
    fn frame(&self, width: usize, focus: bool) -> Frame {
        Frame {
            lines: self.selector.render(width),
            events: self.observed.events.borrow().clone(),
            focus: focus.then(|| {
                [
                    self.selector.focus_flag().get(),
                    self.selector.get_search_input().focus_flag().get(),
                ]
            }),
        }
    }
    /// Applies one recorded operation without synthetic transcript labels.
    fn apply(&self, op: Operation) {
        match op {
            Operation::Key(key) => self.selector.handle_input(&key),
            Operation::Action(action) => match action {
                Action::SetSearch(text) => self.selector.get_search_input().set_value(text),
                Action::Input(key) => self.selector.get_search_input().handle_input(&key),
                Action::Focus(value) => self.selector.focus_flag().set(value),
                Action::InputFocus(value) => {
                    self.selector.get_search_input().focus_flag().set(value);
                }
                Action::Theme(shift) => self.theme.set_theme_instance(colors(shift)).unwrap(),
                Action::Bindings(keys) => bindings::replace(&keys),
                Action::ClearChildren(value) => {
                    assert!(value);
                    self.selector.clear();
                }
                Action::Complete(value) => {
                    assert!(value);
                    self.observed.complete.set(true);
                    assert!(self.runtime.drain_futures().is_empty());
                }
                Action::MutateModel(edit) => {
                    let mut model = self.models[edit.index].write().unwrap();
                    if let Some(value) = edit.fields.provider {
                        model.provider = value;
                    }
                    if let Some(value) = edit.fields.id {
                        model.id = value;
                    }
                    if let Some(value) = edit.fields.name {
                        model.name = value;
                    }
                }
            },
        }
    }
}
/// Replays all attributed cases under one whole-transaction binding guard.
pub fn replay(name: &str) {
    let _guard = crate::keybindings::bindings_guard();
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/scoped_model_selection.json")).unwrap();
    let raw: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/scoped_model_selection.json")).unwrap();
    let mut seen = std::collections::HashSet::new();
    assert!(
        raw.iter()
            .all(|row| seen.insert((row["test"].to_string(), row["input"].to_string())))
    );
    let selected: Vec<_> = cases.into_iter().filter(|case| case.test == name).collect();
    assert!(!selected.is_empty());
    for mut case in selected {
        if cfg!(target_os = "macos") {
            case.expected = case.expected_macos.take().unwrap_or(case.expected);
        }
        assert_eq!(case.expected.branch, "success");
        let Query {
            ops, width, focus, ..
        } = &case.input;
        let (width, focus) = (*width, *focus);
        let expected_count = ops.len() + 1;
        let mut query = case.input;
        let ops = std::mem::take(&mut query.ops);
        let fixture = Fixture::new(query);
        assert_eq!(case.expected.frames.len(), expected_count);
        assert_eq!(
            fixture.frame(width, focus),
            case.expected.frames[0],
            "{name}: initial"
        );
        for (index, (op, expected)) in ops
            .into_iter()
            .zip(case.expected.frames.into_iter().skip(1))
            .enumerate()
        {
            fixture.apply(op);
            assert_eq!(
                fixture.frame(width, focus),
                expected,
                "{name}: action {index}"
            );
        }
        assert_eq!(fixture.runtime.futures(), 0);
    }
}
/// Minimal native query for host and ownership witnesses.
pub fn native_query() -> Query {
    serde_json::from_str("{\"enabled\":[]}").unwrap()
}
