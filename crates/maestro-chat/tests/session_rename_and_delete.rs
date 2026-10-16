//! Session mutation behavior through retained public picker handles.
#![cfg(test)]
#[allow(
    dead_code,
    reason = "Shared picker witnesses use separate integration targets."
)]
#[path = "support/session_picker.rs"]
mod session_picker;

#[test]
fn session_delete_keys_respect_query_ownership() {
    session_picker::session_picker_scopes::mutations::run(
        "session_delete_keys_respect_query_ownership",
    );
}

#[test]
fn session_confirmation_freezes_target_and_intercepts_controls() {
    session_picker::session_picker_scopes::mutations::run(
        "session_confirmation_freezes_target_and_intercepts_controls",
    );
}

#[test]
fn session_empty_views_do_not_mutate() {
    session_picker::session_picker_scopes::mutations::run("session_empty_views_do_not_mutate");
}

#[test]
fn session_mutation_collisions_keep_dispatch_precedence() {
    session_picker::session_picker_scopes::mutations::run(
        "session_mutation_collisions_keep_dispatch_precedence",
    );
}

#[test]
fn session_mutation_bindings_use_global_manager() {
    session_picker::session_picker_scopes::mutations::run(
        "session_mutation_bindings_use_global_manager",
    );
}

#[test]
fn session_rename_hint_defaults_to_callback_capability() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_hint_defaults_to_callback_capability",
    );
}

#[test]
fn session_rename_prefills_and_saves_selected_path() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_prefills_and_saves_selected_path",
    );
}

#[test]
fn session_rename_trims_ecmascript_whitespace() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_trims_ecmascript_whitespace",
    );
}

#[test]
fn session_rename_cancel_restores_query_without_saving() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_cancel_restores_query_without_saving",
    );
}

#[test]
fn session_rename_focus_reaches_both_retained_inputs() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_focus_reaches_both_retained_inputs",
    );
}

#[test]
fn session_rename_reopening_preserves_edit_cursor() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_reopening_preserves_edit_cursor",
    );
}

#[test]
fn session_rename_layout_replaces_child_array() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_layout_replaces_child_array",
    );
}

#[test]
fn session_stale_rename_input_submission_does_not_save() {
    session_picker::session_picker_scopes::mutations::run(
        "session_stale_rename_input_submission_does_not_save",
    );
}

#[test]
fn session_rename_panel_keeps_prepared_text_and_live_borders() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_panel_keeps_prepared_text_and_live_borders",
    );
}

#[test]
fn session_file_delete_passes_literal_trash_arguments() {
    session_picker::session_picker_scopes::mutations::run(
        "session_file_delete_passes_literal_trash_arguments",
    );
}

#[test]
fn session_file_delete_selects_trash_or_unlink() {
    session_picker::session_picker_scopes::mutations::run(
        "session_file_delete_selects_trash_or_unlink",
    );
}

#[test]
fn session_file_delete_failure_combines_bounded_first_line_hint() {
    session_picker::session_picker_scopes::mutations::run(
        "session_file_delete_failure_combines_bounded_first_line_hint",
    );
}

#[test]
fn session_delete_success_updates_both_caches_and_status() {
    session_picker::session_picker_scopes::mutations::run(
        "session_delete_success_updates_both_caches_and_status",
    );
}

#[test]
fn session_delete_failure_keeps_records_without_refresh() {
    session_picker::session_picker_scopes::mutations::run(
        "session_delete_failure_keeps_records_without_refresh",
    );
}

#[test]
fn session_delete_refresh_failure_retains_local_removal() {
    session_picker::session_picker_scopes::mutations::run(
        "session_delete_refresh_failure_retains_local_removal",
    );
}

#[test]
fn session_rename_settles_callback_and_refresh_before_exit() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_settles_callback_and_refresh_before_exit",
    );
}

#[test]
fn session_mutation_refresh_uses_scope_at_effect_completion() {
    session_picker::session_picker_scopes::mutations::run(
        "session_mutation_refresh_uses_scope_at_effect_completion",
    );
}

#[test]
fn session_mutation_refresh_rejects_obsolete_writers() {
    session_picker::session_picker_scopes::mutations::run(
        "session_mutation_refresh_rejects_obsolete_writers",
    );
}

#[test]
fn session_retained_list_keeps_mutation_callbacks() {
    session_picker::session_picker_scopes::mutations::run(
        "session_retained_list_keeps_mutation_callbacks",
    );
}

#[test]
fn session_rename_submissions_keep_independent_completions() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_submissions_keep_independent_completions",
    );
}

#[test]
fn session_active_identity_refuses_deletion() {
    session_picker::session_picker_scopes::mutations::run(
        "session_active_identity_refuses_deletion",
    );
}

#[test]
fn session_mutation_slots_replace_clear_and_reenter() {
    session_picker::session_picker_scopes::mutations::run(
        "session_mutation_slots_replace_clear_and_reenter",
    );
}

#[test]
fn session_rename_waits_for_visible_scope_load() {
    session_picker::session_picker_scopes::mutations::run(
        "session_rename_waits_for_visible_scope_load",
    );
}

#[cfg(target_os = "linux")]
#[test]
fn session_native_file_effects_keep_process_and_unlink_failures() {
    session_picker::session_picker_scopes::mutations::native();
}
