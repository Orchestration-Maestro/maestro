//! Native startup conversions through their public application operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[cfg(unix)]
use crate::child;
#[cfg(unix)]
#[path = "support/startup.rs"]
mod startup;

#[test]
#[cfg(unix)]
fn file_limit_is_restored_before_child_exit() {
    startup::check_file_limit_restoration("file_limit_is_restored_before_child_exit");
}

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
    use std::fs;
    if std::env::var_os("STARTUP_WINDOWS_CHILD").is_some() {
        migrate_sessions_from_agent_root().unwrap();
        return;
    }
    let root = std::env::temp_dir().join(format!("maestro-startup-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let source = root.join("one.jsonl");
    let body = b"{\"type\":\"session\",\"cwd\":\"/work\"}\nopaque";
    fs::write(&source, body).unwrap();
    let output = maestro_test_process::TestProcess::current(
        "session_destinations_reuse_the_session_directory_owner",
    )
    .unwrap()
    .env("STARTUP_WINDOWS_CHILD", "1")
    .env("MAESTRO_CODING_AGENT_DIR", &root)
    .output()
    .unwrap();
    assert!(output.status.success());
    assert!(!source.exists());
    assert_eq!(
        fs::read(root.join("sessions/--work--/one.jsonl")).unwrap(),
        body
    );
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
    let entry = |file: &str| scratch.0.join(file);
    // Enumeration order follows names or creation, so try both until the wanted role is first.
    let (alias, referent) = [("a.jsonl", "b.jsonl"), ("b.jsonl", "a.jsonl")]
        .into_iter()
        .flat_map(|roles| [(roles, false), (roles, true)])
        .find_map(|((alias, referent), rewrite_referent)| {
            for file in [alias, referent] {
                let _ = fs::remove_file(entry(file));
                fs::write(entry(file), body).unwrap();
            }
            fs::remove_file(entry(alias)).unwrap();
            symlink(referent, entry(alias)).unwrap();
            if rewrite_referent {
                fs::remove_file(entry(referent)).unwrap();
                fs::write(entry(referent), body).unwrap();
            }
            let first = fs::read_dir(&scratch.0).unwrap().next().unwrap().unwrap();
            ((first.file_name() == alias) == alias_first).then_some((alias, referent))
        })
        .expect("the directory enumerates the wanted role first");
    for _ in 0..2 {
        child::probe(
            maestro_test_process::TestProcess::current(name)
                .unwrap()
                .name(),
            &scratch.0,
            &[
                ("STARTUP_OPERATION", "sessions"),
                ("MAESTRO_CODING_AGENT_DIR", "."),
            ],
            &[],
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
            maestro_test_process::TestProcess::current(name)
                .unwrap()
                .name(),
            &scratch.0,
            &[
                ("STARTUP_OPERATION", "sessions"),
                ("MAESTRO_CODING_AGENT_DIR", scratch.0.to_str().unwrap()),
            ],
            &[],
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

#[test]
#[cfg(unix)]
fn unchanged_absolute_session_link_moves_by_rename() {
    use std::{
        fs,
        os::unix::fs::{MetadataExt, symlink},
    };
    let name = "unchanged_absolute_session_link_moves_by_rename";
    if std::env::var_os("STARTUP_OPERATION").is_some() {
        maestro_app::configuration::migrations::migrate_sessions_from_agent_root().unwrap();
        println!("PROBE done");
        return;
    }
    let scratch = child::Scratch::new();
    let body = b"{\"type\":\"session\",\"cwd\":\"/work\"}\noriginal transcript";
    let referent = scratch.0.join("original.data");
    fs::write(&referent, body).unwrap();
    let source = scratch.0.join("alias.jsonl");
    symlink(&referent, &source).unwrap();
    let identity = fs::symlink_metadata(&source).unwrap().ino();
    let destination = scratch.0.join("sessions/--work--/alias.jsonl");
    for _ in 0..2 {
        child::probe(
            maestro_test_process::TestProcess::current(name)
                .unwrap()
                .name(),
            &scratch.0,
            &[
                ("STARTUP_OPERATION", "sessions"),
                ("MAESTRO_CODING_AGENT_DIR", scratch.0.to_str().unwrap()),
            ],
            &[],
        );
        assert_eq!(fs::symlink_metadata(&destination).unwrap().ino(), identity);
        assert_eq!(fs::read_link(&destination).unwrap(), referent);
        assert_eq!(fs::read(&destination).unwrap(), body);
        assert_eq!(fs::read(&referent).unwrap(), body);
        assert_eq!(
            fs::symlink_metadata(&source).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }
}

#[test]
#[cfg(unix)]
fn session_link_resolution_matrix() {
    if std::env::var_os("STARTUP_OPERATION").is_some() {
        maestro_app::configuration::migrations::migrate_sessions_from_agent_root().unwrap();
        println!("PROBE done");
        return;
    }
    for depth in 1..=3 {
        for payload in ["absolute", "relative", "absolute-relative", "mixed-case"] {
            for moving in ["none", "final", "intermediate", "deep", "blocked"]
                .into_iter()
                .filter(|moving| {
                    !(depth == 1 && matches!(*moving, "intermediate" | "blocked")
                        || depth < 3 && *moving == "deep"
                        || payload == "mixed-case" && *moving == "blocked")
                })
            {
                check_resolution_orders(depth, payload, moving);
            }
        }
    }
}

#[cfg(unix)]
fn check_resolution_orders(depth: usize, payload: &str, moving: &str) {
    let mut orders = std::collections::BTreeSet::new();
    for arrangement in 0..2 * (depth + 1) {
        let (scratch, paths) = resolution_chain(depth, payload, moving, arrangement);
        if payload == "mixed-case"
            && !scratch
                .0
                .join(paths[1].file_name().unwrap().to_ascii_uppercase())
                .exists()
        {
            return;
        }
        let entries: Vec<_> = std::fs::read_dir(&scratch.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        let next = if moving == "deep" { depth - 1 } else { 1 };
        orders.insert(
            entries.iter().position(|path| path == &paths[0])
                < entries.iter().position(|path| path == &paths[next]),
        );
        check_resolution_chain(&scratch, &paths, payload, moving);
        if moving == "none" || orders.len() == 2 {
            break;
        }
    }
    if moving != "none" {
        assert_eq!(
            orders.len(),
            2,
            "both native orders: {depth}/{payload}/{moving}"
        );
    }
}

#[cfg(unix)]
fn resolution_chain(
    depth: usize,
    payload: &str,
    moving: &str,
    arrangement: usize,
) -> (child::Scratch, Vec<std::path::PathBuf>) {
    let scratch = child::Scratch::new();
    let paths: Vec<_> = (0..=depth)
        .map(|index| {
            let relocates = index == 0
                || (index == depth && moving == "final")
                || (index < depth && matches!(moving, "intermediate" | "final" | "blocked"))
                || (index == depth - 1 && moving == "deep");
            let index_in_order = if arrangement.is_multiple_of(2) {
                index
            } else {
                depth - index
            };
            let number = (index_in_order + arrangement / 2) % (depth + 1);
            scratch.0.join(format!(
                "session-{number}.{}",
                if relocates { "jsonl" } else { "data" }
            ))
        })
        .collect();
    let mut entries: Vec<_> = paths.iter().enumerate().collect();
    entries.sort_by_key(|(_, path)| *path);
    for (index, path) in entries {
        if index == depth {
            std::fs::write(
                path,
                b"{\"type\":\"session\",\"cwd\":\"/work\"}\ntranscript",
            )
            .unwrap();
        } else {
            let target = &paths[index + 1];
            let target = if payload == "mixed-case" {
                scratch
                    .0
                    .join(target.file_name().unwrap().to_ascii_uppercase())
            } else if payload != "relative"
                && !(index > 0 && (moving == "blocked" || payload == "absolute-relative"))
            {
                target.clone()
            } else {
                std::path::PathBuf::from(target.file_name().unwrap())
            };
            std::os::unix::fs::symlink(target, path).unwrap();
            if moving == "blocked" && index > 0 {
                std::fs::write(path.with_extension("jsonl.migrating"), b"occupied").unwrap();
            }
        }
    }
    (scratch, paths)
}

#[cfg(unix)]
fn check_resolution_chain(
    scratch: &child::Scratch,
    paths: &[std::path::PathBuf],
    payload: &str,
    moving: &str,
) {
    use std::{fs, os::unix::fs::MetadataExt};
    let identity = fs::symlink_metadata(&paths[0]).unwrap().ino();
    let authored = fs::read_link(&paths[0]).unwrap();
    for _ in 0..2 {
        child::probe(
            maestro_test_process::TestProcess::current("session_link_resolution_matrix")
                .unwrap()
                .name(),
            &scratch.0,
            &[
                ("STARTUP_OPERATION", "sessions"),
                ("MAESTRO_CODING_AGENT_DIR", "."),
            ],
            &[],
        );
        for (index, path) in paths.iter().enumerate() {
            if index < paths.len() - 1 && path.extension().unwrap() == "data" && moving != "none" {
                continue;
            }
            let destination =
                if path.extension().unwrap() == "jsonl" && !(moving == "blocked" && index > 0) {
                    assert_eq!(
                        fs::symlink_metadata(path).unwrap_err().kind(),
                        std::io::ErrorKind::NotFound
                    );
                    scratch
                        .0
                        .join("sessions/--work--")
                        .join(path.file_name().unwrap())
                } else {
                    path.clone()
                };
            assert_eq!(
                fs::read(&destination).unwrap_or_else(|error| panic!(
                    "{paths:?}/{payload}/{moving} at {destination:?}: {error}"
                )),
                b"{\"type\":\"session\",\"cwd\":\"/work\"}\ntranscript",
                "{paths:?}/{payload}/{moving}"
            );
        }
        let destination = scratch
            .0
            .join("sessions/--work--")
            .join(paths[0].file_name().unwrap());
        assert!(fs::symlink_metadata(&destination).unwrap().is_symlink());
        if payload != "relative" && moving == "none" {
            assert_eq!(fs::symlink_metadata(&destination).unwrap().ino(), identity);
            assert_eq!(fs::read_link(&destination).unwrap(), authored);
        } else {
            assert!(fs::read_link(&destination).unwrap().is_absolute());
        }
    }
}

#[test]
#[cfg(unix)]
fn blocked_intermediate_retry_keeps_migrated_alias_readable() {
    use std::fs;
    if std::env::var_os("STARTUP_OPERATION").is_some() {
        maestro_app::configuration::migrations::migrate_sessions_from_agent_root().unwrap();
        return;
    }
    let (scratch, paths) = resolution_chain(2, "absolute", "blocked", 0);
    let alias = scratch
        .0
        .join("sessions/--work--")
        .join(paths[0].file_name().unwrap());
    for call in 0..3 {
        if call == 2 {
            fs::remove_file(paths[1].with_extension("jsonl.migrating")).unwrap();
        }
        child::probe(
            maestro_test_process::TestProcess::current(
                "blocked_intermediate_retry_keeps_migrated_alias_readable",
            )
            .unwrap()
            .name(),
            &scratch.0,
            &[
                ("STARTUP_OPERATION", "sessions"),
                ("MAESTRO_CODING_AGENT_DIR", "."),
            ],
            &[],
        );
        assert_eq!(
            fs::read(&alias).unwrap(),
            b"{\"type\":\"session\",\"cwd\":\"/work\"}\ntranscript"
        );
    }
    assert!(!paths[1].exists());
}
