//! Scoped model editing through supplied records and the terminal host.
#![cfg(test)]
#[allow(
    dead_code,
    reason = "Only the complete-transaction admission is reused."
)]
#[path = "support/keybindings.rs"]
mod keybindings;
#[allow(dead_code, reason = "Shared controlled host.")]
#[path = "../../maestro-test-terminal/tests/support/manual_runtime.rs"]
mod manual_runtime;
#[allow(dead_code, reason = "Shared model fixture constructors.")]
mod model_selector_support;
#[allow(dead_code, reason = "Shared theme publication controls.")]
#[path = "../../maestro-theme/tests/published/mod.rs"]
mod published;
mod scoped_model_support;
/// Caller-added child rendering failures propagate through the selector.
#[test]
fn caller_added_child_render_error_propagates() {
    use maestro_tui::Component;
    use std::{error::Error, io, rc::Rc};

    struct Fallible;
    impl Component for Fallible {
        fn render(&self, _: usize) -> Vec<String> {
            Vec::new()
        }
        fn try_render(&self, _: usize) -> Result<Vec<String>, Box<dyn Error>> {
            Err(io::Error::other("child render failed").into())
        }
    }

    let _guard = keybindings::bindings_guard();
    let fixture = scoped_model_support::Fixture::new(scoped_model_support::native_query());
    fixture.selector.add_child(Rc::new(Fallible));
    assert_eq!(
        fixture.selector.try_render(120).unwrap_err().to_string(),
        "child render failed"
    );
}

/// Ordered constructor scope with all callback payloads.
#[test]
fn initial_order_retains_explicit_duplicates_and_missing_ids() {
    scoped_model_support::replay("initial_order_retains_explicit_duplicates_and_missing_ids");
}
/// Repeated identities use the final supplied descriptor.
#[test]
fn duplicate_model_ids_use_last_descriptor_and_keep_rows() {
    scoped_model_support::replay("duplicate_model_ids_use_last_descriptor_and_keep_rows");
}

/// Replays the complete toggle uses all sentinel and first occurrence observations.
#[test]
fn toggle_uses_all_sentinel_and_first_occurrence() {
    scoped_model_support::replay("toggle_uses_all_sentinel_and_first_occurrence");
}

/// Replays the complete enable all preserves order and uses all sentinel observations.
#[test]
fn enable_all_preserves_order_and_uses_all_sentinel() {
    scoped_model_support::replay("enable_all_preserves_order_and_uses_all_sentinel");
}

/// Replays the complete enable all targets search results only observations.
#[test]
fn enable_all_targets_search_results_only() {
    scoped_model_support::replay("enable_all_targets_search_results_only");
}

/// Replays the complete clear all handles all explicit and filtered targets observations.
#[test]
fn clear_all_handles_all_explicit_and_filtered_targets() {
    scoped_model_support::replay("clear_all_handles_all_explicit_and_filtered_targets");
}

/// Replays the complete provider toggle ignores search scope and uses membership observations.
#[test]
fn provider_toggle_ignores_search_scope_and_uses_membership() {
    scoped_model_support::replay("provider_toggle_ignores_search_scope_and_uses_membership");
}

/// Replays the complete reorder changes order only for enabled in bounds observations.
#[test]
fn reorder_changes_order_only_for_enabled_in_bounds() {
    scoped_model_support::replay("reorder_changes_order_only_for_enabled_in_bounds");
}

/// Replays the complete reorder callback carries new scope order observations.
#[test]
fn reorder_callback_carries_new_scope_order() {
    scoped_model_support::replay("reorder_callback_carries_new_scope_order");
}

/// Replays the complete filtered reorder clamps both selection bounds observations.
#[test]
fn filtered_reorder_clamps_both_selection_bounds() {
    scoped_model_support::replay("filtered_reorder_clamps_both_selection_bounds");
}

/// Replays the complete enable all uses identity coverage not length observations.
#[test]
fn enable_all_uses_identity_coverage_not_length() {
    scoped_model_support::replay("enable_all_uses_identity_coverage_not_length");
}

/// Replays the complete search uses id provider and exact whitespace domain observations.
#[test]
fn search_uses_id_provider_and_exact_whitespace_domain() {
    scoped_model_support::replay("search_uses_id_provider_and_exact_whitespace_domain");
}

/// Replays the complete search editing clamps without resetting position observations.
#[test]
fn search_editing_clamps_without_resetting_position() {
    scoped_model_support::replay("search_editing_clamps_without_resetting_position");
}

/// Replays the complete navigation wraps and empty actions are inert observations.
#[test]
fn navigation_wraps_and_empty_actions_are_inert() {
    scoped_model_support::replay("navigation_wraps_and_empty_actions_are_inert");
}

/// Replays the complete no match notice and empty footer are exact observations.
#[test]
fn no_match_notice_and_empty_footer_are_exact() {
    scoped_model_support::replay("no_match_notice_and_empty_footer_are_exact");
}

/// Replays the complete eight row window centers clamps and reports position observations.
#[test]
fn eight_row_window_centers_clamps_and_reports_position() {
    scoped_model_support::replay("eight_row_window_centers_clamps_and_reports_position");
}

/// Replays the complete footer shows exact hints counts and unsaved state observations.
#[test]
fn footer_shows_exact_hints_counts_and_unsaved_state() {
    scoped_model_support::replay("footer_shows_exact_hints_counts_and_unsaved_state");
}

/// Replays the complete save calls only persist with owned snapshot observations.
#[test]
fn save_calls_only_persist_with_owned_snapshot() {
    scoped_model_support::replay("save_calls_only_persist_with_owned_snapshot");
}

/// Replays the complete change callback snapshots are independent observations.
#[test]
fn change_callback_snapshots_are_independent() {
    scoped_model_support::replay("change_callback_snapshots_are_independent");
}

/// Replays the complete callbacks allow reentry without borrowed state observations.
#[test]
fn callbacks_allow_reentry_without_borrowed_state() {
    scoped_model_support::replay("callbacks_allow_reentry_without_borrowed_state");
}

/// Replays the complete persist reentry precedes dirty reset observations.
#[test]
fn persist_reentry_precedes_dirty_reset() {
    scoped_model_support::replay("persist_reentry_precedes_dirty_reset");
}

/// Replays the complete asynchronous callbacks run without input barrier observations.
#[test]
fn asynchronous_callbacks_run_without_input_barrier() {
    scoped_model_support::replay("asynchronous_callbacks_run_without_input_barrier");
}

/// Replays the complete control c clears search before cancelling observations.
#[test]
fn control_c_clears_search_before_cancelling() {
    scoped_model_support::replay("control_c_clears_search_before_cancelling");
}

/// Replays the complete escape is literal not select cancel action observations.
#[test]
fn escape_is_literal_not_select_cancel_action() {
    scoped_model_support::replay("escape_is_literal_not_select_cancel_action");
}

/// Replays the complete bindings are live and disabled actions fall through observations.
#[test]
fn bindings_are_live_and_disabled_actions_fall_through() {
    scoped_model_support::replay("bindings_are_live_and_disabled_actions_fall_through");
}

/// Replays the complete colliding actions follow source precedence observations.
#[test]
fn colliding_actions_follow_source_precedence() {
    scoped_model_support::replay("colliding_actions_follow_source_precedence");
}

/// Replays the complete wrong modifiers never trigger model actions observations.
#[test]
fn wrong_modifiers_never_trigger_model_actions() {
    scoped_model_support::replay("wrong_modifiers_never_trigger_model_actions");
}

/// Replays the complete focus forwards to retained input independently observations.
#[test]
fn focus_forwards_to_retained_input_independently() {
    scoped_model_support::replay("focus_forwards_to_retained_input_independently");
}

/// Replays the complete retained search edits do not refresh until dispatch observations.
#[test]
fn retained_search_edits_do_not_refresh_until_dispatch() {
    scoped_model_support::replay("retained_search_edits_do_not_refresh_until_dispatch");
}

/// Replays the complete clearing children keeps retained input and callbacks observations.
#[test]
fn clearing_children_keeps_retained_input_and_callbacks() {
    scoped_model_support::replay("clearing_children_keeps_retained_input_and_callbacks");
}

/// Replays the complete live theme repaints only rebuilt rows and footer observations.
#[test]
fn live_theme_repaints_only_rebuilt_rows_and_footer() {
    scoped_model_support::replay("live_theme_repaints_only_rebuilt_rows_and_footer");
}

/// Replays the complete remapped hints update footer not constructor header observations.
#[test]
fn remapped_hints_update_footer_not_constructor_header() {
    scoped_model_support::replay("remapped_hints_update_footer_not_constructor_header");
}

/// Replays the complete model rows preserve styles unicode and line breaks observations.
#[test]
fn model_rows_preserve_styles_unicode_and_line_breaks() {
    scoped_model_support::replay("model_rows_preserve_styles_unicode_and_line_breaks");
}

/// Replays the complete shared descriptors are read live but identity keys stay fixed observations.
#[test]
fn shared_descriptors_are_read_live_but_identity_keys_stay_fixed() {
    scoped_model_support::replay("shared_descriptors_are_read_live_but_identity_keys_stay_fixed");
}

/// Returns a controlled failing tail only for the selected callback kind.
fn failing_callback(
    fails: bool,
    count: std::rc::Rc<std::cell::Cell<usize>>,
    ready: std::rc::Rc<std::cell::Cell<bool>>,
) -> std::rc::Rc<dyn Fn(Option<Vec<String>>) -> Option<maestro_tui::tui::LocalFuture>> {
    std::rc::Rc::new(move |_| {
        count.set(count.get() + 1);
        if !fails {
            return None;
        }
        let ready = ready.clone();
        Some(Box::pin(std::future::poll_fn(move |_| {
            if !ready.get() {
                return std::task::Poll::Pending;
            }
            std::task::Poll::Ready(Err(
                Box::new(std::io::Error::from_raw_os_error(32)) as Box<dyn std::error::Error>
            ))
        })))
    })
}
/// Host failures preserve their native cause and never block later input.
#[test]
fn callback_failures_leave_selection_usable() {
    let _guard = keybindings::bindings_guard();
    for persist in [false, true] {
        failed_tail(persist);
    }
}
/// Observes one independent callback failure before and after settlement.
fn failed_tail(persist: bool) {
    use maestro_chat::components::{ModelsCallbacks, ModelsConfig, ScopedModelsSelectorComponent};
    use maestro_tui::{Component, tui::InputHandler};
    use std::{cell::Cell, rc::Rc};
    scoped_model_support::bindings(std::collections::BTreeMap::default());
    let runtime = manual_runtime::ManualRuntime::new();
    let ready = Rc::new(Cell::new(false));
    let changes = Rc::new(Cell::new(0));
    let saves = Rc::new(Cell::new(0));
    let selector = ScopedModelsSelectorComponent::new(
        ModelsConfig {
            all_models: scoped_model_support::native_query()
                .models
                .iter()
                .map(model_selector_support::Descriptor::handle)
                .collect(),
            enabled_model_ids: Some(vec![]),
        },
        ModelsCallbacks {
            on_change: failing_callback(!persist, changes.clone(), ready.clone()),
            on_persist: failing_callback(persist, saves.clone(), ready.clone()),
            on_cancel: Rc::new(|| panic!("unexpected cancel")),
        },
        maestro_theme::LiveTheme::default(),
        runtime.handle(),
    );
    selector.handle_input(if persist { "\x13" } else { "\r" });
    assert_eq!(
        (changes.get(), saves.get()),
        if persist { (0, 1) } else { (1, 0) }
    );
    assert_eq!(runtime.futures(), 1);
    selector.handle_input("\x1b[B");
    let before = selector.render(120);
    assert!(before.iter().any(|line| line.contains("Model Name: Two")));
    assert!(runtime.drain_futures().is_empty());
    assert_eq!(runtime.futures(), 1);
    ready.set(true);
    let errors = runtime.drain_futures();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0]
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .raw_os_error(),
        Some(32)
    );
    assert_eq!(selector.render(120), before);
    selector.handle_input("\x1b[B");
    assert!(
        selector
            .render(120)
            .iter()
            .any(|line| line.contains("Model Name: Three"))
    );
    assert_eq!(
        (changes.get(), saves.get()),
        if persist { (0, 1) } else { (1, 0) }
    );
    assert_eq!(runtime.futures(), 0);
}

#[allow(dead_code, reason = "Controlled terminal operations are shared.")]
#[path = "../../maestro-test-terminal/tests/support/recording_terminal.rs"]
mod recording_terminal;
/// Frame-writer ownership retains focus and releases the component graph.
#[test]
fn registered_scope_component_retains_focus_and_releases_children() {
    use maestro_tui::{Component, Focusable, TUI, TerminalImage, tui::InputHandler};
    use std::rc::Rc;
    let _guard = keybindings::bindings_guard();
    let fixture = scoped_model_support::Fixture::new(scoped_model_support::native_query());
    let tui = TUI::new(
        recording_terminal::RecordingTerminal::new(120, 24).handle(),
        fixture.runtime.handle(),
        TerminalImage::default(),
        None,
    );
    let input = fixture.selector.get_search_input().clone();
    let child_weak = Rc::downgrade(&input);
    let weak = Rc::downgrade(&fixture.selector);
    let release = fixture.release.clone();
    tui.add_child(fixture.selector.clone());
    tui.set_focus(Some(fixture.selector.clone()));
    assert!(input.focus_flag().get());
    let observed = fixture.observed.clone();
    drop(fixture);
    let retained = weak.upgrade().unwrap();
    retained.handle_input("b");
    retained.handle_input("\r");
    assert_eq!(
        observed.events.borrow()[0],
        vec![serde_json::json!("change"), serde_json::json!(["p/b"])]
    );
    assert!(
        retained
            .render(120)
            .iter()
            .any(|line| line.contains("\x1b_maestro:c\x07"))
    );
    retained.invalidate();
    let handle: Rc<dyn Component> = retained;
    tui.set_focus(None);
    tui.remove_child(&handle);
    drop(handle);
    assert!(weak.upgrade().is_none());
    assert!(release.upgrade().is_none());
    assert!(!input.focus_flag().get());
    input.set_value("still editable".into());
    input.handle_input("\r");
    assert_eq!(input.get_value(), "still editable");
    assert_eq!(observed.events.borrow().len(), 1);
    drop(input);
    assert!(child_weak.upgrade().is_none());
}
