//! Selected-profile credential sources through the public Bedrock stream, in a child process per
//! row with controlled files and loopback services.

#![cfg(unix)]

#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bedrock.rs"]
mod bedrock;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/chat.rs"]
mod chat;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/child_process.rs"]
mod child_process;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/loopback.rs"]
mod loopback;
#[path = "support/profile_credentials.rs"]
mod profile_credentials;
#[path = "support/profile_files.rs"]
mod profile_files;
#[path = "support/profile_services.rs"]
mod profile_services;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/transport.rs"]
mod transport;

use chat::TestResult;
use profile_credentials::check_rows;

#[test]
fn chained_role_precedes_root_static_keys() -> TestResult {
    check_rows("chained_role_precedes_root_static_keys")
}

#[test]
fn chained_role_uses_selected_sso_source() -> TestResult {
    check_rows("chained_role_uses_selected_sso_source")
}

#[test]
fn chained_role_uses_selected_web_identity_source() -> TestResult {
    check_rows("chained_role_uses_selected_web_identity_source")
}

#[test]
fn conflicting_role_sources_leave_leaf_selection() -> TestResult {
    check_rows("conflicting_role_sources_leave_leaf_selection")
}

#[test]
fn container_failure_does_not_try_instance_metadata() -> TestResult {
    check_rows("container_failure_does_not_try_instance_metadata")
}

#[test]
fn container_selection_excludes_instance_metadata() -> TestResult {
    check_rows("container_selection_excludes_instance_metadata")
}

#[test]
fn credentials_file_keys_override_config_keys() -> TestResult {
    check_rows("credentials_file_keys_override_config_keys")
}

#[test]
fn environment_role_source_signs_the_assumed_role() -> TestResult {
    check_rows("environment_role_source_signs_the_assumed_role")
}

#[test]
fn exhausted_chain_ends_without_a_request() -> TestResult {
    check_rows("exhausted_chain_ends_without_a_request")
}

#[test]
fn failed_process_continues_to_container() -> TestResult {
    check_rows("failed_process_continues_to_container")
}

#[test]
fn incomplete_profile_keys_allow_process() -> TestResult {
    check_rows("incomplete_profile_keys_allow_process")
}

#[test]
fn incomplete_profile_keys_continue_to_container() -> TestResult {
    check_rows("incomplete_profile_keys_continue_to_container")
}

#[test]
fn incomplete_role_source_continues_to_container() -> TestResult {
    check_rows("incomplete_role_source_continues_to_container")
}

#[test]
fn incomplete_standalone_web_identity_continues_to_container() -> TestResult {
    check_rows("incomplete_standalone_web_identity_continues_to_container")
}

#[test]
fn instance_metadata_is_used_without_container() -> TestResult {
    check_rows("instance_metadata_is_used_without_container")
}

#[test]
fn invalid_process_results_continue_to_container() -> TestResult {
    check_rows("invalid_process_results_continue_to_container")
}

#[test]
fn invalid_sso_stops_before_container() -> TestResult {
    check_rows("invalid_sso_stops_before_container")
}

#[test]
fn missing_environment_role_source_continues_to_container() -> TestResult {
    check_rows("missing_environment_role_source_continues_to_container")
}

#[test]
fn missing_mfa_callback_stops_before_fallback() -> TestResult {
    check_rows("missing_mfa_callback_stops_before_fallback")
}

#[test]
fn missing_or_cyclic_role_sources_continue_to_container() -> TestResult {
    check_rows("missing_or_cyclic_role_sources_continue_to_container")
}

#[test]
fn multiple_role_hops_are_applied_from_source_to_root() -> TestResult {
    check_rows("multiple_role_hops_are_applied_from_source_to_root")
}

#[test]
fn named_remote_role_sources_supply_parent() -> TestResult {
    check_rows("named_remote_role_sources_supply_parent")
}

#[test]
fn process_failure_does_not_select_lower_profile_sources() -> TestResult {
    check_rows("process_failure_does_not_select_lower_profile_sources")
}

#[test]
fn process_precedes_sso_and_login() -> TestResult {
    check_rows("process_precedes_sso_and_login")
}

#[test]
fn recursive_credential_source_without_role_supplies_parent() -> TestResult {
    check_rows("recursive_credential_source_without_role_supplies_parent")
}

#[test]
fn recursive_static_keys_precede_further_role_traversal() -> TestResult {
    check_rows("recursive_static_keys_precede_further_role_traversal")
}

#[test]
fn role_options_reach_native_assumption() -> TestResult {
    check_rows("role_options_reach_native_assumption")
}

#[test]
fn role_service_failure_stops_before_fallback() -> TestResult {
    check_rows("role_service_failure_stops_before_fallback")
}

#[test]
fn role_source_process_failure_reaches_root_standalone_process() -> TestResult {
    check_rows("role_source_process_failure_reaches_root_standalone_process")
}

#[test]
fn root_credential_source_without_role_is_not_a_provider() -> TestResult {
    check_rows("root_credential_source_without_role_is_not_a_provider")
}

#[test]
fn selected_web_identity_failure_stops_before_fallback() -> TestResult {
    check_rows("selected_web_identity_failure_stops_before_fallback")
}

#[test]
fn self_referenced_static_source_terminates_successfully() -> TestResult {
    check_rows("self_referenced_static_source_terminates_successfully")
}

#[test]
fn sso_credentials_sign_the_request() -> TestResult {
    check_rows("sso_credentials_sign_the_request")
}

#[test]
fn sso_service_failure_stops_before_container() -> TestResult {
    check_rows("sso_service_failure_stops_before_container")
}

#[test]
fn legacy_sso_token_is_never_renewed() -> TestResult {
    check_rows("legacy_sso_token_is_never_renewed")
}

#[test]
fn standalone_process_retries_after_profile_failure() -> TestResult {
    check_rows("standalone_process_retries_after_profile_failure")
}

#[test]
fn standalone_web_identity_file_failure_is_final() -> TestResult {
    check_rows("standalone_web_identity_file_failure_is_final")
}

#[test]
fn standalone_web_identity_follows_process() -> TestResult {
    check_rows("standalone_web_identity_follows_process")
}

#[test]
fn static_profile_keys_precede_other_leaf_sources() -> TestResult {
    check_rows("static_profile_keys_precede_other_leaf_sources")
}

#[test]
fn static_profile_keys_precede_process() -> TestResult {
    check_rows("static_profile_keys_precede_process")
}

#[test]
fn unconfigured_profile_continues_to_container() -> TestResult {
    check_rows("unconfigured_profile_continues_to_container")
}

#[test]
fn unsupported_role_source_continues_to_container() -> TestResult {
    check_rows("unsupported_role_source_continues_to_container")
}

#[test]
fn web_identity_precedes_process() -> TestResult {
    check_rows("web_identity_precedes_process")
}

#[test]
fn login_cache_signs_each_profile_route() -> TestResult {
    check_rows("login_cache_signs_each_profile_route")
}

#[test]
fn default_login_profile_yields_to_environment_keys() -> TestResult {
    check_rows("default_login_profile_yields_to_environment_keys")
}

#[test]
fn login_cache_directory_override_selects_the_token() -> TestResult {
    check_rows("login_cache_directory_override_selects_the_token")
}

#[test]
fn higher_priority_profile_sources_do_not_load_login() -> TestResult {
    check_rows("higher_priority_profile_sources_do_not_load_login")
}

#[test]
fn login_source_supplies_chained_role() -> TestResult {
    check_rows("login_source_supplies_chained_role")
}

#[test]
fn login_refresh_signs_with_renewed_credentials() -> TestResult {
    check_rows("login_refresh_signs_with_renewed_credentials")
}

#[test]
fn login_access_denied_stops_before_fallback() -> TestResult {
    check_rows("login_access_denied_stops_before_fallback")
}

#[test]
fn login_refresh_failure_reaches_container() -> TestResult {
    check_rows("login_refresh_failure_reaches_container")
}

#[test]
fn login_refresh_classification_ignores_error_text() -> TestResult {
    check_rows("login_refresh_classification_ignores_error_text")
}

#[test]
fn refreshed_login_credentials_survive_cache_write_failure() -> TestResult {
    check_rows("refreshed_login_credentials_survive_cache_write_failure")
}

#[test]
fn login_cache_failure_stops_before_fallback() -> TestResult {
    check_rows("login_cache_failure_stops_before_fallback")
}

#[test]
fn default_profile_keeps_later_credential_sources() -> TestResult {
    check_rows("default_profile_keeps_later_credential_sources")
}

#[test]
fn login_failure_reaches_root_process_after_role_source() -> TestResult {
    check_rows("login_failure_reaches_root_process_after_role_source")
}

#[test]
fn dropped_routed_server_closes_its_listener() -> TestResult {
    let lifetime = std::sync::Arc::new(());
    let observer = std::sync::Arc::downgrade(&lifetime);
    let routed = loopback::serve_routed(move |_| {
        std::hint::black_box(&lifetime);
        unreachable!("no request is sent")
    })?;
    assert!(observer.upgrade().is_some());
    drop(routed);
    assert!(observer.upgrade().is_none());
    Ok(())
}

#[test]
fn routed_server_reports_a_failed_connection_after_joining() -> TestResult {
    use std::io::{Read, Write};
    let routed = loopback::serve_routed(|_| panic!("controlled handler failure"))?;
    let address = routed.url.trim_start_matches("http://").to_owned();
    let mut client = std::net::TcpStream::connect(address)?;
    client.write_all(b"GET / HTTP/1.1\r\nhost: controlled\r\n\r\n")?;
    client.read_to_end(&mut Vec::new())?;
    let error = routed.finish().err().ok_or("finish hid the failure")?;
    assert!(error.to_string().contains("connection thread panicked"));
    Ok(())
}

#[test]
fn duplicate_fixture_ids_are_rejected() -> TestResult {
    profile_credentials::duplicate_ids_are_rejected()
}
