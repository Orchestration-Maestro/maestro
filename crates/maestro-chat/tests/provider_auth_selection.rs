//! Provider selection through the public terminal component.
#![cfg(test)]
#[allow(dead_code, reason = "Only shared keybinding admission is reused.")]
#[path = "support/keybindings.rs"]
mod keybindings;
#[allow(
    dead_code,
    reason = "Shared selector support requires theme file operations."
)]
#[path = "../../maestro-theme/tests/live/mod.rs"]
mod live;
#[allow(
    dead_code,
    reason = "Shared selector support requires the controlled runtime."
)]
#[path = "../../maestro-test-terminal/tests/support/manual_runtime.rs"]
mod manual_runtime;
mod provider_auth_support;
#[allow(dead_code, reason = "Only theme publication is reused.")]
#[path = "../../maestro-theme/tests/published/mod.rs"]
mod published;
#[allow(dead_code, reason = "Only controlled writer input delivery is reused.")]
#[path = "../../maestro-test-terminal/tests/support/recording_terminal.rs"]
mod recording_terminal;
#[allow(
    dead_code,
    reason = "Each selector consumes its own recorded input subset."
)]
mod selector_failure_support;
#[allow(
    dead_code,
    reason = "Shared keybinding admission and theme fixtures are reused."
)]
mod simple_selector_support;

#[test]
/// Provider panel preserves modes order and duplicates.
fn provider_panel_preserves_modes_order_and_duplicates() {
    provider_auth_support::replay("provider_panel_preserves_modes_order_and_duplicates");
}

#[test]
/// Stored auth modes have distinct indicators.
fn stored_auth_modes_have_distinct_indicators() {
    provider_auth_support::replay("stored_auth_modes_have_distinct_indicators");
}
#[test]
/// Provider environment configuration is visible.
fn provider_environment_configuration_is_visible() {
    provider_auth_support::replay("provider_environment_configuration_is_visible");
}

#[test]
/// Custom environment resolver configuration is visible.
fn custom_environment_resolver_configuration_is_visible() {
    provider_auth_support::replay("custom_environment_resolver_configuration_is_visible");
}

#[test]
/// Runtime and fallback keys are visible.
fn runtime_and_fallback_keys_are_visible() {
    provider_auth_support::replay("runtime_and_fallback_keys_are_visible");
}

#[test]
/// Model file key configuration is visible.
fn model_file_key_configuration_is_visible() {
    provider_auth_support::replay("model_file_key_configuration_is_visible");
}
#[test]
/// Model file command configuration is visible.
fn model_file_command_configuration_is_visible() {
    provider_auth_support::replay("model_file_command_configuration_is_visible");
}

#[test]
/// Unconfigured sources ignore configured flag.
fn unconfigured_sources_ignore_configured_flag() {
    provider_auth_support::replay("unconfigured_sources_ignore_configured_flag");
}

#[test]
/// Subscription entries skip external key status.
fn subscription_entries_skip_external_key_status() {
    provider_auth_support::replay("subscription_entries_skip_external_key_status");
}

#[test]
/// Fuzzy queries search name id and auth kind.
fn fuzzy_queries_search_name_id_and_auth_kind() {
    provider_auth_support::replay("fuzzy_queries_search_name_id_and_auth_kind");
}

#[test]
/// Fuzzy whitespace and unicode keep shared semantics.
fn fuzzy_whitespace_and_unicode_keep_shared_semantics() {
    provider_auth_support::replay("fuzzy_whitespace_and_unicode_keep_shared_semantics");
}

#[test]
/// Filtering keeps and clamps selected position.
fn filtering_keeps_and_clamps_selected_position() {
    provider_auth_support::replay("filtering_keeps_and_clamps_selected_position");
}

#[test]
/// Provider navigation clamps without wrapping.
fn provider_navigation_clamps_without_wrapping() {
    provider_auth_support::replay("provider_navigation_clamps_without_wrapping");
}

#[test]
/// Configurable keys preserve up down confirm cancel priority.
fn configurable_keys_preserve_up_down_confirm_cancel_priority() {
    provider_auth_support::replay("configurable_keys_preserve_up_down_confirm_cancel_priority");
}

#[test]
/// Selector and child submission keep provider identity.
fn selector_and_child_submission_keep_provider_identity() {
    provider_auth_support::replay("selector_and_child_submission_keep_provider_identity");
}

#[test]
/// Cancellation and repeat confirmation do not write credentials.
fn cancellation_and_repeat_confirmation_do_not_write_credentials() {
    provider_auth_support::replay("cancellation_and_repeat_confirmation_do_not_write_credentials");
}

#[test]
/// Provider callbacks allow reentry.
fn provider_callbacks_allow_reentry() {
    provider_auth_support::replay("provider_callbacks_allow_reentry");
}

#[test]
/// Eight row window has absolute position counts.
fn eight_row_window_has_absolute_position_counts() {
    provider_auth_support::replay("eight_row_window_has_absolute_position_counts");
}

#[test]
/// Provider rows keep cell truncation and escape style.
fn provider_rows_keep_cell_truncation_and_escape_style() {
    provider_auth_support::replay("provider_rows_keep_cell_truncation_and_escape_style");
    provider_auth_support::narrow_rows_fit();
}

#[test]
/// Selector focus forwards to retained input.
fn selector_focus_forwards_to_retained_input() {
    provider_auth_support::replay("selector_focus_forwards_to_retained_input");
    provider_auth_support::writer_focus_and_teardown();
}

#[test]
/// Provider panel rebuilds only owned theme surfaces.
fn provider_panel_rebuilds_only_owned_theme_surfaces() {
    provider_auth_support::replay("provider_panel_rebuilds_only_owned_theme_surfaces");
}

#[test]
/// Selector children keep shared container identity.
fn selector_children_keep_shared_container_identity() {
    provider_auth_support::replay("selector_children_keep_shared_container_identity");
}

#[test]
/// Resolver is visible only when rows rebuild.
fn resolver_is_visible_only_when_rows_rebuild() {
    provider_auth_support::replay("resolver_is_visible_only_when_rows_rebuild");
}

#[test]
/// Application auth exports use existing owners.
fn application_auth_exports_use_existing_owners() {
    use maestro_app::{
        ApiKeyCredential, AuthCredential, AuthSource, AuthStatus, AuthStorage, AuthStorageBackend,
        InMemoryAuthStorageBackend, OAuthCredential,
    };
    let backend = InMemoryAuthStorageBackend::default();
    let _: &dyn AuthStorageBackend = &backend;
    let storage = AuthStorage::from_storage(backend);
    let key = AuthCredential::ApiKey(ApiKeyCredential {
        key: "controlled-key".into(),
    });
    let oauth = AuthCredential::OAuth(OAuthCredential {
        access: "controlled-access".into(),
        refresh: "controlled-refresh".into(),
        expires: 60000.0,
        extra: serde_json::Map::new(),
    });
    storage.set("key", key.clone());
    storage.set("subscription", oauth.clone());
    let key_matches = storage.get("key") == Some(key);
    assert!(key_matches);
    let oauth_matches = storage.get("subscription") == Some(oauth);
    assert!(oauth_matches);
    let status: AuthStatus = storage.get_auth_status("key").unwrap();
    assert_eq!(
        status,
        AuthStatus {
            configured: true,
            source: Some(AuthSource::Stored),
            label: None
        }
    );
}

#[test]
/// Provider panel uses current brand roles.
fn provider_panel_uses_current_brand_roles() {
    provider_auth_support::brand_roles();
}

#[test]
fn provider_metadata_errors_keep_their_consuming_phase() {
    let _keys = keybindings::bindings_guard();
    provider_auth_support::failures::phases();
}
#[test]
fn provider_rebuild_failure_keeps_filter_and_navigation_effects() {
    let _keys = keybindings::bindings_guard();
    provider_auth_support::failures::rebuilding();
}
#[test]
fn provider_stored_and_subscription_paths_skip_failing_metadata() {
    let _keys = keybindings::bindings_guard();
    provider_auth_support::failures::bypasses();
}

#[test]
fn provider_trait_reports_input_failure_once_without_render() {
    use maestro_tui::tui::InputHandler;
    use provider_auth_support::failures::{Rig, providers};
    use std::rc::Rc;
    let _keys = keybindings::bindings_guard();
    let rig = Rig::new(false, "maestro-custom-fallback");
    let panel = Rc::new(
        rig.build(
            providers(&[
                ("maestro-earlier", "Earlier"),
                ("maestro-custom-fallback", "Custom"),
            ]),
            true,
            maestro_chat::OAuthSelectorMode::Login,
        )
        .unwrap(),
    );
    rig.enable_failure();
    for error in [
        panel.handle_input("\x1b[B").unwrap_err(),
        InputHandler::try_handle_input(panel.as_ref(), "\x1b[B").unwrap_err(),
    ] {
        assert_eq!(
            selector_failure_support::error_value(error.as_ref()),
            selector_failure_support::errors::error_value(
                &selector_failure_support::errors::failure()
            )
        );
        assert_eq!(rig.runtime.futures(), 0);
        assert_eq!(rig.runtime.pending(), 0);
    }
    provider_host_routes(&panel, &rig);
}

/// Observe separate component and writer hosts across both dispatch routes.
fn provider_host_routes(
    panel: &std::rc::Rc<maestro_chat::OAuthSelectorComponent>,
    rig: &provider_auth_support::failures::Rig,
) {
    use maestro_tui::{Component, TUI, TerminalImage, tui::InputHandler};
    let terminal = recording_terminal::RecordingTerminal::new(100, 24);
    let writer_runtime = manual_runtime::ManualRuntime::new();
    let tui = TUI::new(
        terminal.handle(),
        writer_runtime.handle(),
        TerminalImage::default(),
        None,
    );
    tui.set_focus(Some(panel.clone()));
    tui.add_child(panel.clone());
    tui.start().unwrap();
    writer_runtime.settle().unwrap();
    terminal.clear_writes();
    terminal.send_input("\x1b[B");
    assert_eq!(writer_runtime.futures(), 1);
    assert_eq!(rig.runtime.futures(), 0);
    let errors = writer_runtime.drain_futures();
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .downcast_ref::<maestro_app::OAuthError>()
            .is_some()
    );
    assert_eq!(writer_runtime.pending(), 0);
    assert!(terminal.writes().is_empty());
    InputHandler::handle_input(panel.as_ref(), "\x1b[B");
    assert_eq!(rig.runtime.futures(), 1);
    assert_eq!(writer_runtime.futures(), 0);
    let errors = rig.runtime.drain_futures();
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .downcast_ref::<maestro_app::OAuthError>()
            .is_some()
    );
    assert_eq!(rig.runtime.pending(), 0);
    let calls = rig.caller.calls.lock().unwrap().len();
    panel.render(100);
    panel.invalidate();
    assert_eq!(rig.caller.calls.lock().unwrap().len(), calls);
    tui.stop().unwrap();
}
