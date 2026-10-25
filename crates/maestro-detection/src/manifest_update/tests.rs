use super::*;
use crate::{agents::Agent, tests::Fixture};

fn remote_manifest(version: &str, contains: &str) -> String {
    format!(
        r#"
id = "codex"
version = "{version}"
min_engine_version = 1
[[rules]]
id = "idle"
state = "idle"
contains = ["{contains}"]
"#
    )
}

#[test]
fn newer_download_publishes_exact_manifest_bytes() {
    let fixture = Fixture::new();
    let content = remote_manifest("9999.01.01.1", "ready");
    let commit = process_agent_manifest(&fixture.catalog(), Agent::Codex, &content)
        .unwrap()
        .unwrap();
    assert_eq!(commit.agent, Agent::Codex);
    assert_eq!(
        commit.version,
        ManifestVersion::parse("9999.01.01.1").unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(remote_manifest_path(&fixture.catalog(), Agent::Codex)).unwrap(),
        content
    );
}

#[test]
fn dotted_versions_compare_numeric_and_zero_tail_segments() {
    use super::ManifestVersion;
    let parse = |s| ManifestVersion::parse(s).unwrap();
    assert!(parse("1.10") > parse("1.9"));
    assert_eq!(parse("1"), parse("1.0.0"));
    assert!(parse("1.0.1") > parse("1"));
}

#[test]
fn update_commit_reloads_before_result_callback() {
    let mut fixture = Fixture::new();
    let document = remote_manifest("9999.01.01.1", "auto-update-ready");
    let mut requests = Vec::new();
    let mut fetch = |request: FetchRequest<'_>| {
        requests.push(request.url.to_owned());
        assert_eq!(request.max_bytes, 262_144);
        assert_eq!(request.retries, 2);
        assert_eq!(request.connect_timeout, std::time::Duration::from_secs(5));
        assert_eq!(request.total_timeout, std::time::Duration::from_secs(15));
        match request.url {
            "https://catalog.invalid/detection/index.toml" => Ok(
                b"schema_version = 1\n[[agents]]\nid = \"codex\"\npath = \"codex.toml\"\n".to_vec(),
            ),
            "https://catalog.invalid/detection/codex.toml" => Ok(document.as_bytes().to_vec()),
            _ => panic!("unexpected catalog request"),
        }
    };
    let mut observed = false;
    let catalog = fixture.catalog();
    auto_update(
        &mut fixture.cache,
        &catalog,
        "https://catalog.invalid/detection/index.toml",
        &mut fetch,
        |cache, output| {
            assert_eq!(output.updated.len(), 1);
            assert_eq!(output.updated[0].agent, Agent::Codex);
            let explanation = cache.explain(Agent::Codex, "auto-update-ready");
            assert_eq!(explanation.state, crate::agents::AgentState::Idle);
            assert!(matches!(
                explanation.source,
                Some(crate::manifest::ManifestSource::Remote { .. })
            ));
            assert_eq!(
                explanation
                    .matched_rule
                    .as_ref()
                    .map(|rule| rule.id.as_str()),
                Some("idle")
            );
            observed = true;
        },
    )
    .unwrap();
    assert!(observed);
    assert_eq!(
        requests,
        [
            "https://catalog.invalid/detection/index.toml",
            "https://catalog.invalid/detection/codex.toml"
        ]
    );
}

#[test]
fn malformed_or_oversized_version_segments_are_rejected() {
    assert!(ManifestVersion::parse("").is_err());
    assert!(ManifestVersion::parse("2026.06.alpha").is_err());
    assert!(ManifestVersion::parse("2026..06").is_err());
    assert!(ManifestVersion::parse("2026.999999999999999999999999999999").is_err());
}

#[test]
fn downgrade_preserves_cached_manifest_bytes() {
    let fixture = Fixture::new();
    let current = remote_manifest("9999.01.01.1", "current");
    process_agent_manifest(&fixture.catalog(), Agent::Codex, &current).unwrap();

    let older = remote_manifest("9999.01.01.0", "older");
    assert!(process_agent_manifest(&fixture.catalog(), Agent::Codex, &older).is_err());
    assert_eq!(
        std::fs::read_to_string(remote_manifest_path(&fixture.catalog(), Agent::Codex)).unwrap(),
        current
    );
}

#[test]
fn equal_version_changed_content_preserves_cached_bytes() {
    let fixture = Fixture::new();
    let current = remote_manifest("9999.01.01.1", "current");
    process_agent_manifest(&fixture.catalog(), Agent::Codex, &current).unwrap();

    let changed = remote_manifest("9999.01.01.1", "changed");
    assert!(process_agent_manifest(&fixture.catalog(), Agent::Codex, &changed).is_err());
    assert_eq!(
        std::fs::read_to_string(remote_manifest_path(&fixture.catalog(), Agent::Codex)).unwrap(),
        current
    );
}

#[test]
fn equal_version_identical_content_skips_commit() {
    let fixture = Fixture::new();
    let current = remote_manifest("9999.01.01.1", "current");
    process_agent_manifest(&fixture.catalog(), Agent::Codex, &current).unwrap();

    let result = process_agent_manifest(&fixture.catalog(), Agent::Codex, &current).unwrap();
    assert!(result.is_none());
}

#[test]
fn catalogue_resolves_known_labels_and_rejects_duplicate_identity() {
    let catalog = parse_catalog(
        r#"
schema_version = 1

[[agents]]
id = "codex"
path = "codex.toml"
"#,
    )
    .unwrap();
    assert_eq!(catalog[0].agent, Agent::Codex);
    assert_eq!(catalog[0].path, "codex.toml");

    assert!(
        parse_catalog(
            r#"
schema_version = 1

[[agents]]
id = "codex"
path = "codex.toml"

[[agents]]
id = "codex"
path = "codex-2.toml"
"#
        )
        .is_err()
    );
}

#[test]
fn catalogue_rejects_parent_directory_paths() {
    assert!(
        parse_catalog(
            r#"
schema_version = 1

[[agents]]
id = "codex"
path = "../codex.toml"
"#
        )
        .is_err()
    );
}

#[test]
fn fetch_boundaries_apply_before_catalog_decoding() {
    let fixture = Fixture::new();
    let mut at_limit = b"schema_version = 1\n".to_vec();
    at_limit.resize(MAX_FETCH_BYTES, b' ');
    let output = check_and_update(
        &fixture.catalog(),
        "https://catalog.invalid/index.toml",
        &mut |_| Ok(at_limit.clone()),
    )
    .unwrap();
    assert_eq!(output.status.last_result.as_deref(), Some("checked"));
    assert!(output.updated.is_empty());
    at_limit.push(b' ');
    assert_eq!(
        check_and_update(
            &fixture.catalog(),
            "https://catalog.invalid/index.toml",
            &mut |_| Ok(at_limit.clone())
        )
        .unwrap_err(),
        "response from https://catalog.invalid/index.toml exceeded 262144 bytes"
    );
    assert!(
        check_and_update(
            &fixture.catalog(),
            "https://catalog.invalid/index.toml",
            &mut |_| Ok(vec![0xff])
        )
        .unwrap_err()
        .starts_with("response was not UTF-8:")
    );
}

#[test]
fn independent_agent_failures_keep_healthy_commits_and_status() {
    let fixture = Fixture::new();
    assert_eq!(
        load_status(&fixture.catalog()),
        ManifestUpdateStatus::default()
    );
    std::fs::create_dir_all(fixture.catalog()).unwrap();
    std::fs::write(status_path(&fixture.catalog()), "not valid = [").unwrap();
    assert_eq!(
        load_status(&fixture.catalog()),
        ManifestUpdateStatus::default()
    );
    let document = remote_manifest("9999.1", "ready");
    let mut fetch = |request: FetchRequest<'_>| {
        match request.url {
        "https://catalog.invalid/root/index.toml" => Ok(b"schema_version = 1\n[[agents]]\nid = 'codex'\npath = 'good.toml'\n[[agents]]\nid = 'claude'\npath = 'bad.toml'\n".to_vec()),
        "https://catalog.invalid/root/good.toml" => Ok(document.as_bytes().to_vec()),
        "https://catalog.invalid/root/bad.toml" => Err("controlled unavailable document".to_owned()),
        _ => panic!("unexpected nested document endpoint"),
    }
    };
    let first = check_and_update(
        &fixture.catalog(),
        "https://catalog.invalid/root/index.toml",
        &mut fetch,
    )
    .unwrap();
    assert_eq!(first.updated.len(), 1);
    assert_eq!(first.status.agents["codex"].last_result, "updated");
    assert_eq!(
        first.status.agents["codex"].attempted_version.as_deref(),
        Some("9999.1")
    );
    assert_eq!(first.status.agents["claude"].last_result, "failed");
    assert_eq!(
        first.status.agents["claude"].last_error.as_deref(),
        Some("fetch failed: controlled unavailable document")
    );
    assert_eq!(load_status(&fixture.catalog()), first.status);
    let second = check_and_update(
        &fixture.catalog(),
        "https://catalog.invalid/root/index.toml",
        &mut fetch,
    )
    .unwrap();
    assert!(second.updated.is_empty());
    assert_eq!(second.status.agents["codex"].last_result, "current");
    assert_eq!(second.status.agents["codex"].attempted_version, None);
    assert_eq!(
        std::fs::read_to_string(remote_manifest_path(&fixture.catalog(), Agent::Codex)).unwrap(),
        document
    );
}

#[test]
fn failure_callback_and_status_publication_errors_remain_distinct() {
    let mut fixture = Fixture::new();
    let mut callbacks = Vec::new();
    let catalog = fixture.catalog();
    auto_update(
        &mut fixture.cache,
        &catalog,
        "https://catalog.invalid/index.toml",
        &mut |_| Err("offline transport".to_owned()),
        |_, output| callbacks.push(output),
    )
    .unwrap();
    assert_eq!(callbacks.len(), 1);
    assert!(callbacks[0].updated.is_empty());
    assert_eq!(
        callbacks[0].status.last_result.as_deref(),
        Some("failed: offline transport")
    );
    assert!(callbacks[0].status.last_check_unix.is_some());
    std::fs::remove_file(status_path(&fixture.catalog())).unwrap();
    std::fs::create_dir(status_path(&fixture.catalog())).unwrap();
    let output = check_and_update(
        &fixture.catalog(),
        "https://catalog.invalid/index.toml",
        &mut |_| Ok(b"schema_version = 1\n".to_vec()),
    )
    .unwrap();
    assert!(
        output
            .status
            .last_result
            .as_deref()
            .unwrap()
            .starts_with("failed_to_save_status:")
    );
}

#[test]
fn catalog_paths_versions_and_aliases_keep_source_semantics() {
    for version in [" 01.002.0 ", "18446744073709551615"] {
        assert_eq!(
            ManifestVersion::parse(version).unwrap().to_string(),
            version.trim()
        );
    }
    assert_eq!(
        ManifestVersion::parse("01.002.0").unwrap(),
        ManifestVersion::parse("1.2").unwrap()
    );
    assert!(ManifestVersion::parse("1.2").unwrap() < ManifestVersion::parse("1.2.1").unwrap());
    assert_eq!(
        ManifestVersion::parse("18446744073709551616").unwrap_err(),
        "version \"18446744073709551616\" contains an oversized segment"
    );
    for path in [
        "",
        " ",
        "/absolute",
        "https://elsewhere.invalid/x",
        "a/../b",
    ] {
        let content = format!("schema_version = 1\n[[agents]]\nid = 'codex'\npath = {path:?}\n");
        assert!(parse_catalog(&content).is_err());
    }
    for path in [
        "codex.toml",
        "nested/codex.toml",
        "./codex.toml",
        "a..b",
        " padded.toml ",
    ] {
        let entries = parse_catalog(&format!(
            "schema_version = 1\n[[agents]]\nid = 'codex'\npath = {path:?}\n"
        ))
        .unwrap();
        assert_eq!(entries[0].path, path);
    }
    assert!(parse_catalog("schema_version = 1\n[[agents]]\nid = 'claude'\npath = 'a'\n[[agents]]\nid = 'claude-code'\npath = 'b'").unwrap_err().contains("duplicate agent"));
    assert!(
        parse_catalog("schema_version = 1\n[[agents]]\nid = 'unknown-agent'\npath = '../ignored'")
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        parse_catalog("schema_version = 2").unwrap_err(),
        "unsupported catalog schema_version 2"
    );
}
