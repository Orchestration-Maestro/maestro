//! Native startup conversions through their public application operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[cfg(unix)]
#[path = "support/child.rs"]
mod child;
#[cfg(unix)]
#[path = "support/startup.rs"]
mod startup;

#[test]
#[cfg(unix)]
fn current_auth_skips_both_legacy_sources() {
    startup::run("current_auth_skips_both_legacy_sources");
}

#[test]
#[cfg(unix)]
fn auth_conversion_merges_sources_and_keeps_oauth_precedence() {
    startup::run("auth_conversion_merges_sources_and_keeps_oauth_precedence");
}

#[test]
#[cfg(unix)]
fn auth_conversion_retains_property_order_and_number_spelling() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/startup_conversions.json")).unwrap();
    let raw = corpus["auth_conversion_retains_property_order_and_number_spelling"][0]["input"]["files"]["oauth.json"].as_str().unwrap();
    let document: serde_json::Value = serde_json::from_str(raw).unwrap();
    let zero = document["10"]["expires"].as_f64().unwrap();
    assert!(zero == 0.0 && zero.is_sign_negative());
    startup::run("auth_conversion_retains_property_order_and_number_spelling");
}

#[test]
#[cfg(unix)]
fn legacy_oauth_member_domains_keep_accepted_fields() {
    startup::run("legacy_oauth_member_domains_keep_accepted_fields");
}

#[test]
#[cfg(unix)]
fn legacy_oauth_document_domains_follow_their_read_boundary() {
    startup::run("legacy_oauth_document_domains_follow_their_read_boundary");
}

#[test]
#[cfg(unix)]
fn api_key_domains_remove_only_object_or_array_containers() {
    startup::run("api_key_domains_remove_only_object_or_array_containers");
}

#[test]
#[cfg(unix)]
fn unreadable_settings_do_not_prevent_oauth_conversion() {
    startup::run("unreadable_settings_do_not_prevent_oauth_conversion");
}

#[test]
#[cfg(unix)]
fn empty_sources_retire_without_creating_current_auth() {
    startup::run("empty_sources_retire_without_creating_current_auth");
}

#[test]
#[cfg(unix)]
fn absent_sources_leave_the_root_unchanged() {
    startup::run("absent_sources_leave_the_root_unchanged");
}

#[test]
#[cfg(unix)]
fn legacy_source_read_errors_leave_other_sources_usable() {
    startup::run("legacy_source_read_errors_leave_other_sources_usable");
}

#[test]
#[cfg(unix)]
fn credential_publication_failure_keeps_sources_for_retry() {
    startup::run("credential_publication_failure_keeps_sources_for_retry");
}

#[test]
#[cfg(unix)]
fn partial_auth_write_does_not_block_lossless_retry() {
    startup::run("partial_auth_write_does_not_block_lossless_retry");
}

#[test]
#[cfg(unix)]
fn provider_names_are_plain_keys_not_inherited_properties() {
    startup::run("provider_names_are_plain_keys_not_inherited_properties");
}

#[test]
#[cfg(unix)]
fn conversion_keeps_settings_links_and_target_permissions() {
    startup::run("conversion_keeps_settings_links_and_target_permissions");
}

#[test]
#[cfg(unix)]
fn conversion_follows_a_dangling_current_auth_link() {
    startup::run("conversion_follows_a_dangling_current_auth_link");
}

#[test]
#[cfg(unix)]
fn oauth_retirement_collision_keeps_both_credentials_usable() {
    startup::run("oauth_retirement_collision_keeps_both_credentials_usable");
}

#[test]
#[cfg(unix)]
fn failed_linked_auth_publication_retains_the_link_and_sources() {
    startup::run("failed_linked_auth_publication_retains_the_link_and_sources");
}

#[test]
#[cfg(unix)]
fn settings_retirement_failure_keeps_published_credentials() {
    startup::run("settings_retirement_failure_keeps_published_credentials");
}

#[test]
#[cfg(unix)]
fn configuration_lookup_reaches_each_conversion() {
    startup::run("configuration_lookup_reaches_each_conversion");
}

#[test]
#[cfg(unix)]
fn root_session_scan_moves_only_direct_exact_suffix_files() {
    startup::run("root_session_scan_moves_only_direct_exact_suffix_files");
}

#[test]
fn session_destinations_reuse_the_session_directory_owner() {
    #[cfg(unix)]
    startup::run("session_destinations_reuse_the_session_directory_owner");
    #[cfg(windows)]
    windows_basename();
}

#[cfg(windows)]
fn windows_basename() {
    use maestro_app::configuration::migrations::migrate_sessions_from_agent_root;
    use std::{fs, process::Command};
    if std::env::var_os("STARTUP_WINDOWS_CHILD").is_some() {
        migrate_sessions_from_agent_root().unwrap();
        return;
    }
    let root = std::env::temp_dir().join(format!("maestro-startup-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let source = root.join("one.jsonl");
    let body = b"{\"type\":\"session\",\"cwd\":\"/work\"}\nopaque";
    fs::write(&source, body).unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "session_destinations_reuse_the_session_directory_owner",
        ])
        .env("STARTUP_WINDOWS_CHILD", "1")
        .env("MAESTRO_CODING_AGENT_DIR", &root)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(!source.exists());
    assert!(fs::read(root.join("sessions/--work--/one.jsonl")).unwrap() == body);
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn unadmitted_first_lines_leave_the_source_unchanged() {
    startup::run("unadmitted_first_lines_leave_the_source_unchanged");
}

#[test]
#[cfg(unix)]
fn only_nonempty_string_cwds_admit_a_move() {
    startup::run("only_nonempty_string_cwds_admit_a_move");
}

#[test]
#[cfg(unix)]
fn first_header_fields_use_json_last_member_semantics() {
    startup::run("first_header_fields_use_json_last_member_semantics");
}

#[test]
#[cfg(unix)]
fn session_destination_collisions_preserve_both_files() {
    startup::run("session_destination_collisions_preserve_both_files");
}

#[test]
#[cfg(unix)]
fn session_scan_tolerates_file_and_directory_failures() {
    startup::run("session_scan_tolerates_file_and_directory_failures");
}

#[test]
#[cfg(unix)]
fn session_scan_moves_links_without_rewriting_their_targets() {
    startup::run("session_scan_moves_links_without_rewriting_their_targets");
}

#[test]
#[cfg(unix)]
fn session_scans_skip_absent_or_nondirectory_roots() {
    startup::run("session_scans_skip_absent_or_nondirectory_roots");
}

#[test]
#[cfg(unix)]
fn session_moves_preserve_opaque_bytes() {
    startup::run("session_moves_preserve_opaque_bytes");
}
