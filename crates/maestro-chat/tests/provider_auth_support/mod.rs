//! Complete recorded provider-picker replay through public interfaces.
pub mod failures;
mod records;
use crate::bindings;
use maestro_app::{AuthSource, AuthStatus, AuthStorage};
use maestro_chat::{OAuthSelectorComponent, OAuthSelectorMode, OAuthSelectorOptions};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions, ThemeState};
use maestro_tui::{Component, Focusable, Input, tui::InputHandler};
use records::*;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashSet},
    rc::{Rc, Weak},
    sync::Arc,
};

/// Distinguishable role palette used by the recording.
pub fn colors(shift: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Border, 31),
                (ThemeColor::Accent, 32),
                (ThemeColor::Text, 33),
                (ThemeColor::Muted, 34),
                (ThemeColor::Success, 35),
                (ThemeColor::Warning, 36),
            ]
            .map(|(key, value)| (key, ColorValue::Index(value + shift))),
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Mutable caller observations without a strong path back to the component.
#[derive(Default)]
struct Caller {
    events: RefCell<Vec<Vec<String>>>,
    calls: RefCell<Vec<String>>,
    statuses: RefCell<BTreeMap<String, Status>>,
    panel: RefCell<Weak<OAuthSelectorComponent>>,
    reentered: Cell<bool>,
}
impl Caller {
    /// Notify first, then optionally re-enter through the public input route.
    fn notify(&self, id: Option<&str>, reenter: bool) {
        self.events.borrow_mut().push(id.map_or_else(
            || vec!["cancel".into()],
            |id| vec!["select".into(), id.into()],
        ));
        if reenter && !self.reentered.replace(true) {
            let panel = self.panel.borrow().upgrade().unwrap();
            if id.is_some() {
                panel.handle_input("\x1b[B").unwrap();
                panel.render(20);
            }
            panel.handle_input("\r").unwrap();
        }
    }
    /// Resolve only when the picker asks for a visible row.
    fn status(&self, id: &str) -> AuthStatus {
        self.calls.borrow_mut().push(id.into());
        self.statuses.borrow().get(id).cloned().map_or(
            AuthStatus {
                configured: false,
                source: None,
                label: None,
            },
            Into::into,
        )
    }
}
/// Take caller choices or use the existing default provider order.
fn take_providers(query: &mut Query) -> Vec<Provider> {
    query.providers.take().unwrap_or_else(|| {
        [("zeta", "Zed"), ("alpha", "Alpha"), ("beta", "Beta")]
            .map(|(id, name)| Provider {
                id: id.into(),
                name: name.into(),
                auth_type: Kind::ApiKey,
            })
            .into()
    })
}
/// One complete transaction, including retained input and child-array aliases.
struct Replay {
    panel: Rc<OAuthSelectorComponent>,
    input: maestro_tui::tui::ComponentHandle,
    children: maestro_tui::tui::ChildArray,
    theme: ThemeState,
    caller: Rc<Caller>,
    storage: Arc<AuthStorage>,
    width: usize,
    focus: bool,
}
impl Replay {
    /// Construct the real component and its externally retained child.
    fn new(query: &mut Query, focus: bool) -> Self {
        let storage = Arc::new(AuthStorage::in_memory(
            std::mem::take(&mut query.stored)
                .into_iter()
                .map(|(id, value)| (id, value.into()))
                .collect(),
        ));
        for (id, key) in &query.runtime {
            storage.set_runtime_api_key(id, key);
        }
        if let Some(key) = query.fallback.take() {
            storage.set_fallback_resolver(move |_| Ok(Some(key.clone())));
        }
        let (theme, live) = super::published::published(&colors(0));
        let caller = Rc::new(Caller::default());
        let resolver = query.statuses.take().map(|statuses| {
            caller.statuses.replace(statuses);
            let caller = caller.clone();
            Rc::new(move |id: &str| Ok(caller.status(id)))
                as Rc<dyn Fn(&str) -> Result<AuthStatus, Box<dyn std::error::Error>>>
        });
        let selected = caller.clone();
        let cancelled = caller.clone();
        let reenter = query.reenter;
        let reenter_cancel = query.reenter_cancel;
        let providers = take_providers(query);
        let panel = Rc::new(
            OAuthSelectorComponent::new(
                storage.clone(),
                providers.into_iter().map(Into::into).collect(),
                Rc::new(move |id| selected.notify(Some(id), reenter)),
                Rc::new(move || cancelled.notify(None, reenter_cancel)),
                OAuthSelectorOptions {
                    mode: match query.mode.as_deref() {
                        None | Some("login") => OAuthSelectorMode::Login,
                        Some("logout") => OAuthSelectorMode::Logout,
                        _ => unreachable!(),
                    },
                    theme: live,
                    get_auth_status: resolver,
                    runtime: super::manual_runtime::ManualRuntime::new().handle(),
                },
            )
            .unwrap(),
        );
        caller.panel.replace(Rc::downgrade(&panel));
        let children = panel.children();
        let input = children.borrow()[4].clone();
        Self {
            panel,
            input,
            children,
            theme,
            caller,
            storage,
            width: query.width.unwrap_or(80),
            focus,
        }
    }
    /// Downcast the publicly retained concrete input child.
    fn search(&self) -> &Input {
        (&*self.input as &dyn std::any::Any).downcast_ref().unwrap()
    }
    /// Observe every field recorded by the oracle.
    fn snapshot(&self) -> Frame {
        Frame {
            lines: self.panel.render(self.width),
            events: self.caller.events.borrow().clone(),
            status_calls: self.caller.calls.borrow().clone(),
            focus: self.focus.then(|| {
                [
                    self.panel.focus_flag().get(),
                    self.search().focus_flag().get(),
                ]
            }),
        }
    }
    /// Apply one ordered operation and keep caller effects outside mutable borrows.
    fn apply(&self, operation: Operation) {
        match operation {
            Operation::Key(key) => self.panel.handle_input(&key).unwrap(),
            Operation::Query { query } => {
                self.search().set_value(query);
                self.panel.handle_input("").unwrap();
            }
            Operation::Bindings {
                bindings: overrides,
            } => bindings::replace_toolkit(&overrides),
            Operation::ChildSubmit { child_submit } => self.search().handle_input(&child_submit),
            Operation::Focus { focus } => self.panel.focus_flag().set(focus),
            Operation::InputFocus { input_focus } => self.search().focus_flag().set(input_focus),
            Operation::Theme { theme } => {
                self.theme.set_theme_instance(colors(theme)).unwrap();
            }
            Operation::Invalidate { invalidate } => {
                assert!(invalidate);
                self.panel.invalidate();
            }
            Operation::SetStored { set_stored } => self
                .storage
                .set(&set_stored.id, set_stored.credential.into()),
            Operation::RemoveStored { remove_stored } => self.storage.remove(&remove_stored),
            Operation::Statuses { statuses } => {
                self.caller.statuses.replace(statuses);
            }
            Operation::Clear { clear } => {
                assert!(clear);
                self.panel.clear();
            }
            Operation::ReplaceChildren { replace_children } => {
                assert!(replace_children);
                self.panel.clear();
                self.panel.add_child(Rc::new(maestro_tui::Text::new(
                    "replacement".into(),
                    0,
                    0,
                    None,
                )));
            }
            Operation::RestoreChildren { restore_children } => {
                assert!(restore_children);
                self.panel.set_children(self.children.clone());
            }
        }
    }
}
/// Decode one nonempty attributed group; no orphan or duplicated query is admitted.
fn cases(name: &str) -> Vec<Case> {
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/provider_auth_selection.json")).unwrap();
    let names: Vec<_> = include_str!("../provider_auth_selection.rs")
        .lines()
        .filter_map(|line| {
            line.strip_prefix("fn ")
                .and_then(|line| line.strip_suffix("() {"))
        })
        .collect();
    let mut queries = HashSet::new();
    for row in &rows {
        assert!(names.contains(&row.test.as_str()), "orphan corpus group");
        assert!(
            queries.insert((row.test.as_str(), row.input.to_string())),
            "duplicate corpus query"
        );
    }
    let cases: Vec<_> = rows.into_iter().filter(|row| row.test == name).collect();
    assert!(!cases.is_empty(), "missing corpus group");
    cases
}
/// Replay all rows owned by a named witness.
pub fn replay(name: &str) {
    let _admission = super::keybindings::bindings_guard();
    let selection = maestro_test_process::selection();
    let mut matched = false;
    for (index, case) in cases(name).into_iter().enumerate() {
        let mut query: Query = serde_json::from_value(case.input).unwrap();
        if selection.is_child() {
            if !selection.runs_case(&index.to_string()) {
                continue;
            }
        } else if !query.env.is_empty() || name == "provider_environment_configuration_is_visible" {
            child(name, index, &query.env);
            continue;
        }
        matched = true;
        run(case.expected, &mut query, name);
    }
    assert!(
        !selection.is_child() || matched,
        "selected auth row missing"
    );
}
/// Drive one constructor and compare full output without exposing credentials in failures.
fn run(expected: serde_json::Value, query: &mut Query, name: &str) {
    let expected: Expected = serde_json::from_value(expected).unwrap();
    bindings::replace_toolkit(&query.bindings);
    let replay = Replay::new(query, name == "selector_focus_forwards_to_retained_input");
    let stored = replay.storage.get_all();
    let mut frames = vec![replay.snapshot()];
    for operation in std::mem::take(&mut query.ops) {
        replay.apply(operation);
        frames.push(replay.snapshot());
    }
    assert_eq!(frames, expected.frames);
    if let Some(unchanged) = expected.stored_unchanged {
        assert_eq!(unchanged, replay.storage.get_all() == stored);
    }
    if let Some(runtime) = expected.runtime_unchanged {
        for (id, key) in runtime {
            assert!(
                query.runtime.get(&id) == Some(&key),
                "runtime constructor input differs"
            );
            assert_eq!(
                replay.storage.get_auth_status(&id).unwrap().source,
                Some(AuthSource::Runtime)
            );
        }
    }
    let weak = Rc::downgrade(&replay.panel);
    drop(replay);
    assert!(weak.upgrade().is_none());
}
/// Isolate environment cases in the existing test executable, waiting for process completion.
fn child(name: &str, index: usize, env: &BTreeMap<String, String>) {
    let output = maestro_test_process::TestProcess::current(name)
        .unwrap()
        .nocapture()
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_OAUTH_TOKEN")
        .envs(env)
        .case(index.to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "controlled child failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// The corrected toolkit never overflows a one-cell viewport.
pub fn narrow_rows_fit() {
    let _admission = super::keybindings::bindings_guard();
    bindings::replace_toolkit(&BTreeMap::new());
    let panel = Replay::new(&mut Query::default(), false);
    let lines = panel.panel.render(1);
    assert!(!lines.is_empty());
    assert!(
        lines
            .iter()
            .all(|line| maestro_tui::visible_width(line) <= 1)
    );
}
/// Actual writer focus forwards immediately, and retained children release captures on drop.
pub fn writer_focus_and_teardown() {
    let _admission = super::keybindings::bindings_guard();
    bindings::replace_toolkit(&BTreeMap::new());
    let replay = Replay::new(&mut Query::default(), false);
    let runtime = super::manual_runtime::ManualRuntime::new();
    let terminal = super::recording_terminal::RecordingTerminal::new(80, 24);
    let tui = maestro_tui::TUI::new(
        terminal.handle(),
        runtime.handle(),
        maestro_tui::TerminalImage::new(|_| None, || 1),
        Some(false),
    );
    tui.add_child(replay.panel.clone());
    tui.set_focus(Some(replay.panel.clone()));
    assert!(replay.search().focus_flag().get());
    assert!(
        replay
            .panel
            .render(80)
            .iter()
            .any(|line| line.contains(maestro_tui::CURSOR_MARKER))
    );
    let parent = Rc::downgrade(&replay.panel);
    let caller = Rc::downgrade(&replay.caller);
    replay.panel.clear();
    tui.start().unwrap();
    terminal.send_input("\r");
    assert_eq!(*replay.caller.events.borrow(), vec![vec!["select", "zeta"]]);
    tui.set_focus(None);
    assert!(!replay.search().focus_flag().get());
    tui.clear();
    tui.stop().unwrap();
    drop(tui);
    let input = replay.input.clone();
    let weak_input = Rc::downgrade(&input);
    drop(replay);
    assert!(parent.upgrade().is_none());
    input.input_handler().unwrap().handle_input("\r");
    assert_eq!(
        *caller.upgrade().unwrap().events.borrow(),
        vec![vec!["select", "zeta"], vec!["select", "zeta"]]
    );
    drop(input);
    assert!(weak_input.upgrade().is_none());
    assert!(caller.upgrade().is_none());
    runtime.settle().unwrap();
    assert_eq!(runtime.pending(), 0);
}
/// Shipped themes color the new surface through their semantic roles.
pub fn brand_roles() {
    let _admission = super::keybindings::bindings_guard();
    bindings::replace_toolkit(&BTreeMap::new());
    for name in ["dark", "light"] {
        let path = format!(
            "{}/../maestro-theme/assets/theme/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let theme = Rc::new(
            maestro_theme::load_theme_from_path(&path, Some(ColorMode::Truecolor)).unwrap(),
        );
        let (_state, live) = super::published::published(&theme);
        let panel = brand_panel(live);
        let lines = panel.render(80);
        assert_eq!(
            lines[0],
            theme.fg(&ThemeColor::Border, &"─".repeat(80)).unwrap()
        );
        assert!(
            lines[2].contains(
                &theme
                    .fg(
                        &ThemeColor::Accent,
                        &Theme::bold("Select provider to configure:", true)
                    )
                    .unwrap()
            )
        );
        assert!(lines[6].contains(&theme.fg(&ThemeColor::Accent, "→ ").unwrap()));
        assert!(lines[6].contains(&theme.fg(&ThemeColor::Success, " ✓ configured").unwrap()));
        assert!(lines[7].contains(&theme.fg(&ThemeColor::Muted, " • ").unwrap()));
        assert!(
            lines[7].contains(
                &theme
                    .fg(&ThemeColor::Warning, "API key configured")
                    .unwrap()
            )
        );
        assert!(lines[8].contains(&theme.fg(&ThemeColor::Text, "Unconfigured").unwrap()));
        assert!(lines[8].contains(&theme.fg(&ThemeColor::Muted, " • unconfigured").unwrap()));
    }
}

/// Construct stored, mismatched and empty authentication rows for role integration.
fn brand_panel(live: maestro_theme::LiveTheme) -> OAuthSelectorComponent {
    let storage = Arc::new(AuthStorage::in_memory(indexmap::IndexMap::new()));
    storage.set(
        "key",
        maestro_app::AuthCredential::ApiKey(maestro_app::ApiKeyCredential {
            env: None,
            key: "controlled-key".into(),
        }),
    );
    let providers = [
        ("key", "Configured", maestro_chat::AuthType::ApiKey),
        ("key", "Mismatch", maestro_chat::AuthType::OAuth),
        ("unset", "Unconfigured", maestro_chat::AuthType::OAuth),
    ]
    .map(|(id, name, auth_type)| maestro_chat::AuthSelectorProvider {
        id: id.into(),
        name: name.into(),
        auth_type,
    })
    .into();
    OAuthSelectorComponent::new(
        storage,
        providers,
        Rc::new(|_| {}),
        Rc::new(|| {}),
        OAuthSelectorOptions {
            mode: OAuthSelectorMode::Login,
            theme: live,
            get_auth_status: None,
            runtime: super::manual_runtime::ManualRuntime::new().handle(),
        },
    )
    .unwrap()
}
