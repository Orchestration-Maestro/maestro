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

#[test]
#[cfg(unix)]
fn session_links_follow_relocated_referents_alias_first() {
    relocated_referent("session_links_follow_relocated_referents_alias_first", true);
}

#[test]
#[cfg(unix)]
fn session_links_follow_relocated_referents_referent_first() {
    relocated_referent(
        "session_links_follow_relocated_referents_referent_first",
        false,
    );
}

#[cfg(unix)]
fn relocated_referent(name: &str, alias_first: bool) {
    use std::{fs, os::unix::fs::symlink};
    if std::env::var_os("STARTUP_OPERATION").is_some() {
        maestro_app::configuration::migrations::migrate_sessions_from_agent_root().unwrap();
        println!("PROBE done");
        return;
    }
    let scratch = child::Scratch::new();
    let body = b"{\"type\":\"session\",\"cwd\":\"/work\"}\noriginal transcript";
    for file in ["a.jsonl", "b.jsonl"] {
        fs::write(scratch.0.join(file), body).unwrap();
    }
    let entries = || {
        fs::read_dir(&scratch.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>()
    };
    let order = entries();
    let alias_index = usize::from(!alias_first);
    let alias = &order[alias_index];
    let referent = &order[1 - alias_index];
    fs::remove_file(scratch.0.join(alias)).unwrap();
    symlink(referent, scratch.0.join(alias)).unwrap();
    assert_eq!(
        entries(),
        order,
        "qualify the actual native encounter order"
    );
    for _ in 0..2 {
        child::probe(
            name,
            &scratch.0,
            &[
                ("STARTUP_OPERATION", "sessions"),
                ("MAESTRO_CODING_AGENT_DIR", "."),
            ],
        );
        for file in [alias, referent] {
            assert_eq!(
                fs::read(scratch.0.join("sessions/--work--").join(file)).unwrap(),
                body
            );
            assert_eq!(
                fs::symlink_metadata(scratch.0.join(file))
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::NotFound
            );
        }
        assert!(
            fs::symlink_metadata(scratch.0.join("sessions/--work--").join(alias))
                .unwrap()
                .is_symlink()
        );
    }
}

#[test]
#[cfg(unix)]
fn relative_session_link_replaces_a_dangling_destination() {
    use std::{fs, os::unix::fs::symlink};
    let name = "relative_session_link_replaces_a_dangling_destination";
    if std::env::var_os("STARTUP_OPERATION").is_some() {
        maestro_app::configuration::migrations::migrate_sessions_from_agent_root().unwrap();
        println!("PROBE done");
        return;
    }
    let scratch = child::Scratch::new();
    let body = b"{\"type\":\"session\",\"cwd\":\"/work\"}\noriginal transcript";
    fs::write(scratch.0.join("original.data"), body).unwrap();
    symlink("original.data", scratch.0.join("alias.jsonl")).unwrap();
    let directory = scratch.0.join("sessions/--work--");
    fs::create_dir_all(&directory).unwrap();
    symlink("missing.data", directory.join("alias.jsonl")).unwrap();
    for _ in 0..2 {
        child::probe(
            name,
            &scratch.0,
            &[
                ("STARTUP_OPERATION", "sessions"),
                ("MAESTRO_CODING_AGENT_DIR", scratch.0.to_str().unwrap()),
            ],
        );
        assert_eq!(
            fs::symlink_metadata(scratch.0.join("alias.jsonl"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
        assert!(
            fs::symlink_metadata(directory.join("alias.jsonl"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(fs::read(directory.join("alias.jsonl")).unwrap(), body);
        assert_eq!(fs::read(scratch.0.join("original.data")).unwrap(), body);
        assert!(!directory.join("missing.data").exists());
    }
}
