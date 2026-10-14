//! Model selection through real catalog, settings and toolkit owners.
#![cfg(test)]
#[allow(
    dead_code,
    reason = "Shared host controls are reused by other components."
)]
#[path = "../../maestro-test-terminal/tests/support/manual_runtime.rs"]
mod manual_runtime;
mod model_selector_support;
#[allow(dead_code, reason = "Only theme publication is reused.")]
#[path = "../../maestro-theme/tests/published/mod.rs"]
mod published;
#[allow(dead_code, reason = "Only controlled terminal construction is reused.")]
#[path = "../../maestro-test-terminal/tests/support/recording_terminal.rs"]
mod recording_terminal;
#[path = "../../maestro-catalog/tests/support/subprocess.rs"]
mod subprocess;
use maestro_app::{Model, ScopedModel, SettingsManager};
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
fn replay(name: &str) {
    if std::env::var_os("MAESTRO_DYNAMIC_CHILD").is_none() {
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
        subprocess::run_child(name, &environment);
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
    reads: Rc<std::cell::Cell<usize>>,
    /// Weak caller capture used for teardown.
    release: Weak<()>,
}
impl Fixture {
    /// Builds the public picker through real controlled owners.
    fn new(query: &mut Query) -> Self {
        assert!(!query.current_shared || !query.scoped.is_empty());
        bindings(std::mem::take(&mut query.bindings));
        let (theme, live) = published::published(&colors(0));
        let (mut registry, reads) = registry(query);
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
                &mut registry,
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
        assert_eq!(fixture.reads.get(), count);
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
        Action::Bindings(keys) => bindings(keys),
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
    replay("navigation_wraps_and_empty_confirm_is_inert");
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
    let mut registry = malformed_registry();
    let selector = ModelSelectorComponent::new(
        &tui,
        Rc::new(RefCell::new(SettingsManager::in_memory(
            serde_json::from_str("{}").unwrap(),
        ))),
        &mut registry,
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
