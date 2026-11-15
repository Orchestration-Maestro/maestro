//! Completed TLS observations through the default cloud conversation transport.
#![cfg(target_os = "linux")]

#[test]
fn bedrock_force_http1_changes_tls_protocol() -> crate::TestResult {
    crate::bedrock_tls_routes::run_cases("bedrock_force_http1_changes_tls_protocol")
}

#[test]
fn bedrock_https_destination_routes_use_selected_proxy() -> crate::TestResult {
    crate::bedrock_tls_routes::run_cases("bedrock_https_destination_routes_use_selected_proxy")
}
