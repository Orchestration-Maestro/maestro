//! Selected-profile credential sources through the public Bedrock stream, in a child process per
//! row with controlled files and loopback services.

#![cfg(unix)]

use crate::chat;
use crate::loopback;
use crate::profile_credentials;
use crate::profile_routes;

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

#[test]
fn login_credentials_follow_proxy_and_bypass() -> TestResult {
    profile_routes::check("login_credentials_follow_proxy_and_bypass")
}

#[test]
fn role_credentials_follow_proxy_and_bypass() -> TestResult {
    profile_routes::check("role_credentials_follow_proxy_and_bypass")
}

#[test]
fn web_credentials_follow_proxy_and_bypass() -> TestResult {
    profile_routes::check("web_credentials_follow_proxy_and_bypass")
}

#[test]
fn sso_credentials_follow_proxy_and_bypass() -> TestResult {
    profile_routes::check("sso_credentials_follow_proxy_and_bypass")
}

#[test]
fn oidc_credentials_follow_proxy_and_bypass() -> TestResult {
    profile_routes::check("oidc_credentials_follow_proxy_and_bypass")
}

#[test]
fn environment_web_identity_uses_the_shared_route() -> TestResult {
    profile_routes::check("environment_web_identity_uses_the_shared_route")
}

#[test]
fn metadata_credentials_stay_direct_with_proxy() -> TestResult {
    profile_routes::check("metadata_credentials_stay_direct_with_proxy")
}

#[test]
fn credential_https_endpoints_select_https_proxy() -> TestResult {
    profile_routes::check("credential_https_endpoints_select_https_proxy")
}

#[test]
fn credential_routes_are_selected_per_destination() -> TestResult {
    profile_routes::check("credential_routes_are_selected_per_destination")
}

#[test]
fn sso_services_work_when_direct_access_is_refused() -> TestResult {
    profile_routes::check("sso_services_work_when_direct_access_is_refused")
}

#[test]
fn credential_service_failures_keep_their_chain_outcome() -> TestResult {
    profile_routes::check("credential_service_failures_keep_their_chain_outcome")
}

#[test]
fn non_network_authentication_does_not_contact_credential_services() -> TestResult {
    profile_routes::check("non_network_authentication_does_not_contact_credential_services")
}

#[test]
fn credential_services_reuse_the_supplied_sender() -> TestResult {
    profile_routes::check("credential_services_reuse_the_supplied_sender")
}

#[test]
fn bedrock_socks_reuses_credential_route() -> TestResult {
    profile_routes::check("bedrock_socks_reuses_credential_route")
}

#[test]
fn idle_routed_shutdown_joins_without_connection_errors() -> TestResult {
    if std::env::var("MAESTRO_CHILD_CASE").ok().is_none() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "idle_routed_shutdown_joins_without_connection_errors",
            )?
            .nocapture()
            .env_clear()
            .env("MAESTRO_CHILD_CASE", "idle")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    for iteration in 0..20_000 {
        let server = loopback::start_routed(std::sync::Arc::new(|_| {
            panic!("wake-up connection must not reach the handler")
        }))?;
        server
            .finish()
            .map_err(|error| format!("idle shutdown {iteration}: {error}"))?;
    }
    Ok(())
}

#[test]
fn routed_shutdown_retains_handler_errors() -> TestResult {
    let (entered, ready) = std::sync::mpsc::channel();
    let server = loopback::start_routed(std::sync::Arc::new(move |_| {
        entered.send(()).unwrap();
        Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "served request reset",
        ))
    }))?;
    let connection = std::net::TcpStream::connect(&server.url[7..])?;
    ready.recv()?;
    assert_eq!(
        server.finish().unwrap_err().to_string(),
        "served request reset"
    );
    drop(connection);
    Ok(())
}
