//! Model selection through real catalog, settings and toolkit owners.
#![cfg(test)]
use crate::bindings;
use crate::manual_runtime;
use crate::model_selector_support;
use crate::published;
use crate::recording_terminal;
use crate::runtime_spy;
use crate::selector_failure_support;
#[cfg(not(target_arch = "wasm32"))]
use crate::subprocess;
use maestro_app::{Model, ScopedModel, SettingsManager, SharedHandle};
use maestro_chat::{ModelSelectorCallbacks, ModelSelectorComponent, ModelSelectorOptions};
use maestro_tui::{
    Component, TUI, TerminalImage,
    tui::{Focusable, InputHandler},
};
use model_selector_support::*;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    sync::{Arc, RwLock},
};

/// Execute each test group in a separate process, which owns global registry/binding state.
#[cfg(not(target_arch = "wasm32"))]
fn isolate(name: &str) -> bool {
    if !maestro_test_process::selection().is_child() {
        let credentials = std::env::current_exe()
            .unwrap()
            .join("absent-credentials.json")
            .to_string_lossy()
            .into_owned();
        assert!(!std::path::Path::new(&credentials).exists());
        let mut environment = AUTH_ENVIRONMENT.to_vec();
        environment.extend([
            ("LC_ALL", Some("en_US.UTF-8")),
            ("GOOGLE_APPLICATION_CREDENTIALS", Some(&credentials)),
        ]);
        subprocess::run_child(
            maestro_test_process::TestProcess::current(name)
                .unwrap()
                .case("*"),
            &environment,
        );
        return true;
    }
    false
}
/// The browser target has no child process, so every group shares this process.
#[cfg(target_arch = "wasm32")]
fn isolate(_name: &str) -> bool {
    false
}
fn replay(name: &str) {
    if isolate(name) {
        return;
    }
    for case in cases(name) {
        run(case);
    }
    if name == "registry_notice_renders_each_line_and_suppresses_name_only" {
        malformed_notice();
    }
}
/// Read a complete frame through public operations.
fn frame(
    selector: &ModelSelectorComponent,
    events: &RefCell<Vec<Vec<serde_json::Value>>>,
    width: usize,
    focus: bool,
) -> Frame {
    Frame {
        lines: selector.render(width),
        events: events.borrow().clone(),
        focus: focus.then(|| {
            [
                selector.focus_flag().get(),
                selector.get_search_input().focus_flag().get(),
            ]
        }),
    }
}
/// Shared caller observations retained by callbacks.
#[derive(Clone)]
struct Observations {
    /// Ordered user-visible notifications.
    events: Rc<RefCell<Vec<Vec<serde_json::Value>>>>,
    /// Caller settings owner.
    settings: Rc<RefCell<SettingsManager>>,
    /// Non-owning reentry target.
    slot: Rc<RefCell<Weak<ModelSelectorComponent>>>,
    /// Original selection handles.
    handles: Rc<RefCell<Vec<Arc<RwLock<Model>>>>>,
}
impl Observations {
    /// Constructs independent caller state.
    fn new() -> Self {
        Self {
            events: Rc::default(),
            settings: Rc::new(RefCell::new(SettingsManager::in_memory(
                serde_json::from_str("{}").unwrap(),
            ))),
            slot: Rc::default(),
            handles: Rc::default(),
        }
    }
    /// Records a confirmation after observing both already-saved defaults.
    fn selected(&self, model: Arc<RwLock<Model>>, reenter: bool) {
        let (provider, id, name) = {
            let m = model.read().unwrap();
            (m.provider.clone(), m.id.clone(), m.name.clone())
        };
        assert_eq!(
            self.settings.borrow().get_default_provider().as_deref(),
            Some(provider.as_str())
        );
        assert_eq!(
            self.settings.borrow().get_default_model().as_deref(),
            Some(id.as_str())
        );
        self.events.borrow_mut().push(vec![
            "save".into(),
            provider.clone().into(),
            id.clone().into(),
        ]);
        self.events.borrow_mut().push(vec![
            "select".into(),
            provider.into(),
            id.into(),
            name.into(),
        ]);
        self.handles.borrow_mut().push(model);
        if reenter {
            let target = self.slot.borrow().upgrade().unwrap();
            target.handle_input("\x1b[B");
        }
    }
    /// Builds notifications retaining one release witness.
    fn callbacks(&self, reenter: bool) -> (ModelSelectorCallbacks, Weak<()>) {
        let release = Rc::new(());
        let weak = Rc::downgrade(&release);
        let select = self.clone();
        let cancel = self.events.clone();
        (
            ModelSelectorCallbacks {
                on_select: Rc::new(move |model| {
                    assert_eq!(Rc::strong_count(&release), 1);
                    select.selected(model, reenter);
                }),
                on_cancel: Rc::new(move || cancel.borrow_mut().push(vec!["cancel".into()])),
            },
            weak,
        )
    }
}
/// A complete constructor transaction with retained owner observations.
struct Fixture {
    /// Public picker, separately owned for the release test.
    selector: Rc<ModelSelectorComponent>,
    /// Published theme controls.
    theme: maestro_theme::ThemeState,
    /// Caller observations.
    observed: Observations,
    /// Refreshed descriptors in original input order.
    refreshed: Vec<Arc<RwLock<Model>>>,
    /// Original scoped fallback descriptors.
    scoped: Vec<Arc<RwLock<Model>>>,
    /// Constructor read count.
    reads: Arc<std::sync::atomic::AtomicUsize>,
    /// Weak caller capture used for teardown.
    release: Weak<()>,
}
impl Fixture {
    /// Builds the public picker through real controlled owners.
    fn new(query: &mut Query) -> Self {
        assert!(!query.current_shared || !query.scoped.is_empty());
        bindings::replace_toolkit(&query.bindings);
        let (theme, live) = published::published(&colors(0));
        let (registry, reads) = registry(query);
        let runtime = manual_runtime::ManualRuntime::new();
        let tui = TUI::new(
            recording_terminal::RecordingTerminal::new(120, 24).handle(),
            runtime.handle(),
            TerminalImage::default(),
            None,
        );
        assert_eq!(runtime.pending(), 0);
        let observed = Observations::new();
        let (callbacks, release) = observed.callbacks(query.reenter);
        let scoped: Vec<_> = query.scoped.iter().map(Descriptor::handle).collect();
        let options = ModelSelectorOptions {
            current_model: if query.current_shared {
                Some(scoped[0].clone())
            } else {
                query.current.as_ref().map(Descriptor::handle)
            },
            scoped_models: scoped
                .iter()
                .map(|model| ScopedModel {
                    model: model.clone(),
                    thinking_level: None,
                })
                .collect(),
            initial_search_input: query.initial.take(),
            theme: live,
        };
        let result: Result<ModelSelectorComponent, Box<dyn std::error::Error>> =
            ModelSelectorComponent::new(
                &tui,
                observed.settings.clone(),
                &registry,
                callbacks,
                options,
            );
        let selector = Rc::new(result.unwrap());
        *observed.slot.borrow_mut() = Rc::downgrade(&selector);
        assert_eq!(runtime.pending(), 1);
        observed.events.borrow_mut().push(vec!["render".into()]);
        let refreshed = ordered_handles(&registry, &query.available);
        Self {
            selector,
            theme,
            observed,
            refreshed,
            scoped,
            reads,
            release,
        }
    }
    /// Retained Input remains usable after outer release without a cycle.
    fn retained(self, expected: Expected, width: usize) {
        let input = self.selector.get_search_input().clone();
        drop(self.selector);
        input.handle_input("\n");
        assert_eq!(input.render(width), expected.input_lines.unwrap());
        assert_eq!(*self.observed.events.borrow(), expected.events.unwrap());
        assert!(self.release.upgrade().is_some());
        drop(input);
        assert!(self.release.upgrade().is_none());
    }
    /// Reports only original registry or supplied fallback handles.
    fn identity(&self) {
        for handle in self.observed.handles.borrow().iter() {
            assert!(
                self.refreshed
                    .iter()
                    .chain(&self.scoped)
                    .any(|m| Arc::ptr_eq(m, handle))
            );
        }
    }
}
/// Execute one complete real-owner transaction.
fn run(case: Case) {
    let Case {
        test,
        input: mut query,
        expected,
    } = case;
    let fixture = Fixture::new(&mut query);
    if let Some(count) = expected.refresh_count {
        assert_eq!(
            fixture.reads.load(std::sync::atomic::Ordering::SeqCst),
            count
        );
    }
    if expected.input_lines.is_some() {
        fixture.retained(expected, query.width);
        return;
    }
    assert!(expected.events.is_none());
    extra_observations(&test, &fixture.selector);
    let focus = test.starts_with("focus_");
    assert_eq!(
        frame(
            &fixture.selector,
            &fixture.observed.events,
            query.width,
            focus
        ),
        expected.frames[0]
    );
    assert_eq!(expected.frames.len(), query.ops.len() + 1);
    for (op, wanted) in query
        .ops
        .into_iter()
        .zip(expected.frames.into_iter().skip(1))
    {
        match op {
            Operation::Input(data) => fixture.selector.handle_input(&data),
            Operation::Action(action) => action_run(
                action,
                &fixture.selector,
                &fixture.theme,
                &fixture.refreshed,
            ),
        }
        assert_eq!(
            frame(
                &fixture.selector,
                &fixture.observed.events,
                query.width,
                focus
            ),
            wanted
        );
    }
    fixture.identity();
}
/// Recover original descriptor order and duplicate occurrence identity after refresh.
fn ordered_handles(
    registry: &maestro_app::ModelRegistry,
    models: &[Descriptor],
) -> Vec<Arc<RwLock<Model>>> {
    let all = registry.get_all();
    let all = all.read().unwrap();
    let mut used = std::collections::HashSet::new();
    models
        .iter()
        .map(|m| {
            let (i, handle) = all
                .iter()
                .enumerate()
                .find(|(i, handle)| {
                    !used.contains(i) && {
                        let v = handle.read().unwrap();
                        v.provider == m.provider && v.id == m.id
                    }
                })
                .unwrap();
            used.insert(i);
            handle.clone()
        })
        .collect()
}
/// Dispatch only the recorded public action.
fn action_run(
    action: Action,
    selector: &ModelSelectorComponent,
    theme: &maestro_theme::ThemeState,
    models: &[Arc<RwLock<Model>>],
) {
    match action {
        Action::Bindings(keys) => bindings::replace_toolkit(&keys),
        Action::Theme(shift) => theme.set_theme_instance(colors(shift)).unwrap(),
        Action::Focus(value) => selector.focus_flag().set(value),
        Action::InputFocus(value) => selector.get_search_input().focus_flag().set(value),
        Action::InputSubmit(data) => selector.get_search_input().handle_input(&data),
        Action::SetSearch(text) => selector.get_search_input().set_value(text),
        Action::ClearChildren(value) => {
            assert!(value);
            selector.clear();
        }
        Action::MutateModel(Mutation { index, changes }) => {
            {
                let mut model = models[index].write().unwrap();
                if let Some(value) = changes.provider {
                    model.provider = value;
                }
                if let Some(value) = changes.id {
                    model.id = value;
                }
                if let Some(value) = changes.name {
                    model.name = value;
                }
            }
            selector.handle_input("");
        }
    }
}
#[test]
fn model_scope_defaults_and_current_identity() {
    replay("model_scope_defaults_and_current_identity");
}

#[test]
fn all_models_put_current_first_then_stable_locale_provider() {
    replay("all_models_put_current_first_then_stable_locale_provider");
}

#[test]
fn provider_order_uses_locale_without_id_tiebreak() {
    replay("provider_order_uses_locale_without_id_tiebreak");
}

#[test]
fn scoped_order_survives_current_model_and_scope_roundtrip() {
    replay("scoped_order_survives_current_model_and_scope_roundtrip");
}

#[test]
fn scoped_refresh_replaces_matches_retains_missing_and_duplicates() {
    replay("scoped_refresh_replaces_matches_retains_missing_and_duplicates");
}

#[test]
fn initial_search_uses_id_provider_and_combined_fuzzy_fields() {
    replay("initial_search_uses_id_provider_and_combined_fuzzy_fields");
}

#[test]
fn typing_filters_and_clamps_existing_selection() {
    replay("typing_filters_and_clamps_existing_selection");
}

#[test]
fn scope_switch_keeps_query_and_clamps_current_position() {
    replay("scope_switch_keeps_query_and_clamps_current_position");
}

#[test]
fn navigation_wraps_and_empty_confirm_is_inert() {
    if isolate("navigation_wraps_and_empty_confirm_is_inert") {
        return;
    }
    replay("navigation_wraps_and_empty_confirm_is_inert");
    for (count, start, previous, next) in [
        (0, 0, 0, 0),
        (1, 0, 0, 0),
        (3, 0, 2, 1),
        (3, 1, 0, 2),
        (3, 2, 1, 0),
    ] {
        for (key, expected) in [("\x1b[A", previous), ("\x1b[B", next)] {
            let models: Vec<_> = ["a", "b", "c"]
                .into_iter()
                .take(count)
                .map(|id| serde_json::json!({"provider": "p", "id": id, "name": id}))
                .collect();
            let mut query: Query = serde_json::from_value(serde_json::json!({
                "available": models, "scoped": models
            }))
            .unwrap();
            let fixture = Fixture::new(&mut query);
            for _ in 0..start {
                fixture.selector.handle_input("\x1b[B");
            }
            let before = fixture.selector.render(120);
            fixture.observed.events.borrow_mut().clear();
            fixture.selector.handle_input(key);
            fixture.selector.handle_input("\r");
            if count == 0 {
                assert_eq!(fixture.selector.render(120), before);
                assert!(fixture.observed.events.borrow().is_empty());
            } else {
                let id = ["a", "b", "c"][expected];
                assert_eq!(
                    *fixture.observed.events.borrow(),
                    vec![
                        vec![
                            serde_json::json!("save"),
                            serde_json::json!("p"),
                            serde_json::json!(id)
                        ],
                        vec![
                            serde_json::json!("select"),
                            serde_json::json!("p"),
                            serde_json::json!(id),
                            serde_json::json!(id)
                        ]
                    ]
                );
            }
        }
    }
}

#[test]
fn tab_without_scoped_models_is_consumed() {
    replay("tab_without_scoped_models_is_consumed");
}

#[test]
fn confirmation_saves_pair_before_callback_and_allows_reentry() {
    replay("confirmation_saves_pair_before_callback_and_allows_reentry");
}

#[test]
fn search_input_submit_selects_without_inserting_newline() {
    replay("search_input_submit_selects_without_inserting_newline");
}

#[test]
fn search_submit_remains_available_when_selector_confirm_disabled() {
    replay("search_submit_remains_available_when_selector_confirm_disabled");
}

#[test]
fn bindings_are_live_and_wrong_modifiers_do_not_select() {
    replay("bindings_are_live_and_wrong_modifiers_do_not_select");
}

#[test]
fn binding_collisions_follow_tab_up_down_confirm_cancel_order() {
    replay("binding_collisions_follow_tab_up_down_confirm_cancel_order");
}

#[test]
fn model_window_centers_and_clamps_ten_rows() {
    replay("model_window_centers_and_clamps_ten_rows");
}

#[test]
fn empty_catalog_and_no_match_have_exact_notice() {
    replay("empty_catalog_and_no_match_have_exact_notice");
}

#[test]
fn registry_notice_renders_each_line_and_suppresses_name_only() {
    replay("registry_notice_renders_each_line_and_suppresses_name_only");
}

#[test]
fn focus_propagates_to_retained_input_cursor() {
    replay("focus_propagates_to_retained_input_cursor");
}

#[test]
fn scope_hint_refreshes_keys_on_toggle_and_rows_use_live_theme() {
    replay("scope_hint_refreshes_keys_on_toggle_and_rows_use_live_theme");
}

#[test]
fn model_render_wraps_metadata_badges_and_unicode() {
    replay("model_render_wraps_metadata_badges_and_unicode");
}

/// Linux refuses new readers once a writer queues behind an existing reader.
#[cfg(target_os = "linux")]
#[test]
fn shared_current_filtering_finishes_with_a_queued_writer() {
    if isolate("model_selection::shared_current_filtering_finishes_with_a_queued_writer") {
        return;
    }
    let mut query: Query = serde_json::from_value(serde_json::json!({
        "available": [],
        "scoped": [{"provider": "missing", "id": "fallback", "name": "Fallback"}],
        "currentShared": true,
        "readFailure": {"path": "<models-file>", "message": "controlled read failure"}
    }))
    .unwrap();
    let fixture = Fixture::new(&mut query);
    let before = frame(
        &fixture.selector,
        &fixture.observed.events,
        query.width,
        false,
    );
    assert!(
        before
            .lines
            .iter()
            .any(|line| line.contains("\x1b[38;5;34m ✓\x1b[39m"))
    );
    let model = &fixture.scoped[0];
    let reader = model.read().unwrap();
    let writer_model = model.clone();
    let writer = std::thread::spawn(move || {
        writer_model.write().unwrap().name = "Edited".into();
    });
    loop {
        match model.try_read() {
            Err(std::sync::TryLockError::WouldBlock) => break,
            Err(std::sync::TryLockError::Poisoned(_)) => panic!("descriptor poisoned"),
            Ok(guard) => drop(guard),
        }
        std::thread::yield_now();
    }
    eprintln!("writer admission established with reader retained; rebuilding current row");
    fixture.selector.handle_input("");
    assert_eq!(
        frame(
            &fixture.selector,
            &fixture.observed.events,
            query.width,
            false
        ),
        before
    );
    drop(reader);
    writer.join().unwrap();
    assert_eq!(model.read().unwrap().name, "Edited");
}

#[test]
fn shared_selected_model_metadata_is_read_at_confirmation() {
    replay("shared_selected_model_metadata_is_read_at_confirmation");
}

#[test]
fn container_clear_does_not_destroy_retained_input_submission() {
    replay("container_clear_does_not_destroy_retained_input_submission");
}

#[test]
fn search_input_mutations_wait_for_selector_filter_dispatch() {
    replay("search_input_mutations_wait_for_selector_filter_dispatch");
}

#[test]
fn focus_forwarding_keeps_independent_selector_and_input_flags() {
    replay("focus_forwarding_keeps_independent_selector_and_input_flags");
}

#[test]
fn retained_search_input_confirms_after_outer_release() {
    replay("retained_search_input_confirms_after_outer_release");
}

/// Real decoding diagnostics remain visible through the picker constructor.
fn malformed_notice() {
    let (_, theme) = published::published(&colors(0));
    let runtime = manual_runtime::ManualRuntime::new();
    let tui = TUI::new(
        recording_terminal::RecordingTerminal::new(120, 24).handle(),
        runtime.handle(),
        TerminalImage::default(),
        None,
    );
    let registry = malformed_registry();
    let selector = ModelSelectorComponent::new(
        &tui,
        Rc::new(RefCell::new(SettingsManager::in_memory(
            serde_json::from_str("{}").unwrap(),
        ))),
        &registry,
        ModelSelectorCallbacks {
            on_select: Rc::new(|_| {}),
            on_cancel: Rc::new(|| {}),
        },
        ModelSelectorOptions {
            current_model: None,
            scoped_models: vec![],
            initial_search_input: None,
            theme,
        },
    )
    .unwrap();
    assert!(
        selector
            .render(120)
            .iter()
            .any(|line| line.contains("Failed to parse models.json"))
    );
}

/// The picker delegates editing to the same retained child array.
fn inherited_children(selector: &ModelSelectorComponent) {
    let extra: Rc<dyn Component> = Rc::new(maestro_tui::Text::new("extra".into(), 0, 0, None));
    selector.add_child(extra.clone());
    assert!(selector.render(120).last().unwrap().starts_with("extra"));
    selector.remove_child(&extra);
    assert!(!selector.render(120).last().unwrap().starts_with("extra"));
    let original = selector.children();
    selector.set_children(Rc::default());
    assert!(selector.render(120).is_empty());
    selector.set_children(original);
    assert!(!selector.render(120).is_empty());
}

/// Qualifies native bounded rendering and delegated child editing.
fn extra_observations(test: &str, selector: &ModelSelectorComponent) {
    if test == "model_render_wraps_metadata_badges_and_unicode" {
        assert!(
            selector
                .render(1)
                .iter()
                .all(|line| maestro_tui::visible_width(line) <= 1)
        );
    }
    if test == "container_clear_does_not_destroy_retained_input_submission" {
        inherited_children(selector);
    }
}

/// Real registry whose file notice is superseded by the consumed fallback failure.
fn failing_registry(
    input: &mut selector_failure_support::Input,
) -> (
    maestro_app::ModelRegistry,
    Arc<std::sync::atomic::AtomicBool>,
) {
    let query: Query = serde_json::from_value(serde_json::json!({"available":[], "readFailure":{"message":input.notice.take().unwrap(), "path":"<models-file>"}})).unwrap();
    let (registry, _) = registry(&query);
    assert!(registry.get_error().is_some());
    registry
        .register_provider(
            "maestro-unconfigured",
            SharedHandle::new(RwLock::new(maestro_app::ProviderConfigInput {
                callbacks: maestro_app::ProviderCallbacks {
                    oauth: Some(SharedHandle::new(RejectingModels)),
                    ..Default::default()
                },
                models: Some(vec![
                    Descriptor {
                        provider: "maestro-unconfigured".into(),
                        id: "m".into(),
                        name: "Model m".into(),
                    }
                    .definition(),
                ]),
                base_url: Some("https://example.invalid".into()),
                api: Some("openai-completions".into()),
                ..Default::default()
            })),
        )
        .unwrap();
    let reached = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = Arc::clone(&reached);
    let message = input.message.take().unwrap();
    registry.auth_storage().set_fallback_resolver(move |id| {
        if id != "maestro-unconfigured" {
            return Ok(None);
        }
        observed.store(true, std::sync::atomic::Ordering::SeqCst);
        let mut error = selector_failure_support::errors::failure();
        error.diagnostic.message.clone_from(&message);
        Err(error)
    });
    (registry, reached)
}
/// Original scope and initial query, independently of whether loading succeeds.
fn failed_picker_options(input: &mut selector_failure_support::Input) -> ModelSelectorOptions {
    let scoped = if input.scoped.unwrap() {
        vec![ScopedModel {
            model: Descriptor {
                provider: "maestro-scoped".into(),
                id: "s".into(),
                name: "Model s".into(),
            }
            .handle(),
            thinking_level: None,
        }]
    } else {
        Vec::new()
    };
    let (_, theme) = published::published(&colors(0));
    ModelSelectorOptions {
        current_model: None,
        scoped_models: scoped,
        initial_search_input: input.initial.take(),
        theme,
    }
}
#[test]
fn failed_picker_clears_all_and_scoped_views_but_keeps_original_help() {
    let name = "failed_picker_clears_all_and_scoped_views_but_keeps_original_help";
    if isolate(name) {
        return;
    }
    for case in selector_failure_support::cases(name) {
        bindings::replace_toolkit(&std::collections::BTreeMap::default());
        let mut input = case.input;
        let (registry, reached) = failing_registry(&mut input);
        let runtime = runtime_spy::Spy::default();
        let tui = TUI::new(
            recording_terminal::RecordingTerminal::new(100, 24).handle(),
            Rc::new(runtime.clone()),
            TerminalImage::default(),
            None,
        );
        let events = runtime.events.clone();
        let cancelled = events.clone();
        let settings = Rc::new(RefCell::new(SettingsManager::in_memory(
            serde_json::from_str("{}").unwrap(),
        )));
        let options = failed_picker_options(&mut input);
        let selector = ModelSelectorComponent::new(
            &tui,
            settings.clone(),
            &registry,
            ModelSelectorCallbacks {
                on_select: Rc::new(|_| panic!("failed load cannot select")),
                on_cancel: Rc::new(move || {
                    cancelled.borrow_mut().push(serde_json::json!(["cancel"]));
                }),
            },
            options,
        )
        .unwrap();
        assert!(reached.load(std::sync::atomic::Ordering::SeqCst));
        let width = input.width.unwrap();
        let mut frames =
            vec![serde_json::json!({"lines":selector.render(width),"events":*events.borrow()})];
        for key in input.ops.unwrap() {
            selector.handle_input(&key);
            frames.push(
                serde_json::json!({"lines":selector.render(width),"events":*events.borrow()}),
            );
        }
        assert_eq!(
            serde_json::json!({"branch":"ok","frames":frames}),
            case.expected,
            "recorded failed model frames"
        );
        assert!(settings.borrow().get_default_model().is_none());
        assert!(settings.borrow().get_default_provider().is_none());
    }
}

/// Provider whose stored credentials reject a catalog transformation.
struct RejectingModels;
impl maestro_app::OAuthProviderInterface for RejectingModels {
    fn id(&self) -> &'static str {
        "maestro-refresh-failure"
    }
    fn name(&self) -> &'static str {
        "Controlled refresh"
    }
    fn login(
        &self,
        _: maestro_app::OAuthCallbacks,
        _: Option<maestro_app::Fetch>,
    ) -> maestro_app::BoxFuture<Result<maestro_app::OAuthCredentials, maestro_app::OAuthError>>
    {
        Box::pin(async { Err(selector_failure_support::errors::failure()) })
    }
    fn refresh_token(
        &self,
        _: maestro_app::OAuthCredentials,
        _: Option<maestro_app::Fetch>,
    ) -> maestro_app::BoxFuture<Result<maestro_app::OAuthCredentials, maestro_app::OAuthError>>
    {
        Box::pin(async { Err(selector_failure_support::errors::failure()) })
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a maestro_app::OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, maestro_app::OAuthError> {
        Ok(std::borrow::Cow::Borrowed(&credentials.access))
    }
    fn modify_models<'m>(
        &self,
        _: &'m [Model],
        _: &maestro_app::OAuthCredentials,
    ) -> Option<Result<Vec<std::borrow::Cow<'m, Model>>, maestro_app::OAuthError>> {
        Some(Err(selector_failure_support::errors::failure()))
    }
}
/// Real retained provider whose transformation is first consumed by the next refresh.
fn rejecting_registry() -> maestro_app::ModelRegistry {
    let registry = maestro_app::ModelRegistry::in_memory(
        SharedHandle::new(maestro_app::AuthStorage::in_memory(
            std::iter::empty().collect(),
        )),
        no_commands(),
    )
    .unwrap();
    registry
        .register_provider(
            "maestro-refresh-failure",
            SharedHandle::new(RwLock::new(maestro_app::ProviderConfigInput {
                callbacks: maestro_app::ProviderCallbacks {
                    oauth: Some(SharedHandle::new(RejectingModels)),
                    ..Default::default()
                },
                models: Some(vec![
                    Descriptor {
                        provider: "maestro-refresh-failure".into(),
                        id: "m".into(),
                        name: "Model m".into(),
                    }
                    .definition(),
                ]),
                base_url: Some("https://example.invalid".into()),
                api: Some("openai-completions".into()),
                ..Default::default()
            })),
        )
        .unwrap();
    registry.auth_storage().set(
        "maestro-refresh-failure",
        maestro_app::AuthCredential::OAuth(maestro_app::OAuthCredentials {
            access: "access".into(),
            refresh: "refresh".into(),
            expires: 0.0,
            extra: serde_json::Map::default(),
        }),
    );
    registry
        .auth_storage()
        .set_fallback_resolver(|_| panic!("availability must not run"));
    registry
}
#[test]
fn picker_refresh_failure_is_not_availability_notice() {
    let name = "picker_refresh_failure_is_not_availability_notice";
    if isolate(name) {
        return;
    }
    for case in selector_failure_support::cases(name) {
        assert_eq!(case.input.phase.as_deref(), Some("refresh"));
        let registry = rejecting_registry();
        let runtime = runtime_spy::Spy::default();
        let tui = TUI::new(
            recording_terminal::RecordingTerminal::new(100, 24).handle(),
            Rc::new(runtime.clone()),
            TerminalImage::default(),
            None,
        );
        let (_, theme) = published::published(&colors(0));
        let result = ModelSelectorComponent::new(
            &tui,
            Observations::new().settings,
            &registry,
            ModelSelectorCallbacks {
                on_select: Rc::new(|_| panic!("no picker")),
                on_cancel: Rc::new(|| panic!("no picker")),
            },
            ModelSelectorOptions {
                current_model: None,
                scoped_models: vec![],
                initial_search_input: None,
                theme,
            },
        );
        let error = result.err().unwrap();
        let error = error.downcast_ref::<maestro_app::CatalogError>().unwrap();
        let maestro_app::CatalogError::OAuth(error) = error else {
            panic!("original error")
        };
        let actual = serde_json::json!({"branch":"error", "error":selector_failure_support::errors::error_value(error), "events":*runtime.events.borrow(), "calls":[]});
        assert!(actual == case.expected, "recorded refresh error boundary");
    }
}
