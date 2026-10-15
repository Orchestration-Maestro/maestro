//! Retained session discovery through the public selector.
#![cfg(test)]
#[allow(
    dead_code,
    reason = "Shared header and list witnesses have their own target."
)]
#[path = "support/session_picker.rs"]
mod session_picker;
#[test]
fn session_scope_starts_current_and_cancels_only_empty_all() {
    session_picker::session_picker_scopes::initial();
}

#[test]
fn session_selector_composes_existing_borders_and_three_row_header() {
    session_picker::session_picker_scopes::layout();
}

#[test]
fn session_selector_focus_reaches_retained_search() {
    session_picker::session_picker_scopes::focus();
}

#[test]
fn session_selector_rename_hint_accepts_explicit_policy() {
    session_picker::session_picker_scopes::rename_hint();
}

#[test]
fn session_scope_late_all_completion_keeps_current_choice() {
    session_picker::session_picker_scopes::late();
}

#[test]
fn session_scope_pending_all_load_is_not_started_twice() {
    session_picker::session_picker_scopes::pending_once();
}

#[test]
fn session_scope_progress_belongs_to_visible_load() {
    session_picker::session_picker_scopes::progress();
}

#[test]
fn session_scope_inactive_current_completion_only_updates_cache() {
    session_picker::session_picker_scopes::inactive_current();
}

#[test]
fn session_scope_load_failure_obeys_scope_and_status_expiry() {
    session_picker::session_picker_scopes::failure();
}

#[test]
fn session_scope_failed_all_load_can_retry() {
    session_picker::session_picker_scopes::retry();
}
#[test]
fn session_selector_controls_reuse_query_and_live_list_setters() {
    session_picker::session_picker_scopes::controls();
}

#[test]
fn session_scope_switch_reapplies_retained_search() {
    session_picker::session_picker_scopes::query_scope();
}

#[test]
fn session_selector_global_and_supplied_bindings_stay_independent() {
    session_picker::session_picker_scopes::managers();
}

#[test]
fn session_selector_remapped_controls_reject_wrong_modifiers() {
    session_picker::session_picker_scopes::modifiers();
}

#[test]
fn session_selector_control_collisions_follow_dispatch_order() {
    session_picker::session_picker_scopes::collisions();
}

#[test]
fn session_list_control_slots_replace_clear_and_reenter() {
    session_picker::session_picker_scopes::slots();
}

#[test]
fn session_selector_terminal_callbacks_clear_status_before_delivery() {
    session_picker::session_picker_scopes::terminal();
}

#[test]
fn session_scope_empty_cancel_uses_settled_records_in_both_orders() {
    session_picker::session_picker_scopes::empty_cancel();
}

#[test]
fn session_selector_forwards_current_identity_to_list() {
    session_picker::session_picker_scopes::identity();
}

#[test]
fn session_scope_load_failure_is_not_an_empty_record() {
    session_picker::session_picker_scopes::failed_empty();
}

#[test]
fn session_selector_keeps_supplied_manager_identity() {
    session_picker::session_picker_scopes::manager_identity();
}

#[test]
fn session_selector_keeps_container_aliases_and_fallible_delegation() {
    session_picker::session_picker_scopes::container();
}

#[test]
fn session_selector_retained_list_owns_callback_state() {
    session_picker::session_picker_scopes::retained();
}

#[test]
fn session_scope_action_definitions_keep_default_keys_and_descriptions() {
    session_picker::session_picker_scopes::definitions();
}
