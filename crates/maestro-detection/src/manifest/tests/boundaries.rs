const REGION_CASES: &[(&str, &str, &str)] = &[
    (
        "whole_recent",
        "before\n› now\nafter\n",
        "before\n› now\nafter\n",
    ),
    (
        "after_last_prompt_marker",
        "before\n› now\nafter\n",
        "after\n",
    ),
    ("after_last_prompt_marker", "no prompt\n", "no prompt\n"),
    (
        "before_current_prompt_marker",
        "before\n› now\nafter\n",
        "before\n",
    ),
    (
        "before_current_prompt_marker",
        "› old\n• new block\n",
        "› old\n• new block\n",
    ),
    ("whole_recent_without_current_prompt_marker", "› live\n", ""),
    (
        "whole_recent_without_current_prompt_marker",
        "plain\n",
        "plain\n",
    ),
    (
        "current_prompt_block_marker",
        "• old\n■ newest\n› live\n",
        "■ newest",
    ),
    ("current_prompt_block_marker", "› old\n• newer\n", ""),
    (
        "after_current_prompt_block_marker",
        "• block\nanswer\n› live\n",
        "• block\nanswer\n› live\n",
    ),
    ("after_current_prompt_block_marker", "› live\n", ""),
    (
        "prompt_box_body",
        "above\n─── title\nbody\n─\nbelow\n",
        "body\n",
    ),
    ("prompt_box_body", "─\nbody\n", ""),
    (
        "above_prompt_box",
        "above\n─── title\nbody\n─\nbelow\n",
        "above\n",
    ),
    ("above_prompt_box", "─\nbody\n", "─\nbody\n"),
    (
        "last_non_empty_above_prompt_box",
        "a\nlast\n\n───\nbody\n───\n",
        "last",
    ),
    (
        "after_last_horizontal_rule",
        "a\n─\nb\n─── title\nlast\n",
        "last\n",
    ),
    (
        "after_last_horizontal_rule",
        "─ title\nunchanged\n",
        "─ title\nunchanged\n",
    ),
    ("bottom_lines(0)", "a\nb\n", ""),
    ("bottom_lines(+1)", "a\nb\n", "b\n"),
    ("bottom_lines(99)", "a\nb", "a\nb"),
    ("bottom_non_empty_lines(0)", "a\n\nb\n", ""),
    ("bottom_non_empty_lines(2)", "a\n\nb\n", "a\n\nb\n"),
    ("top_non_empty_lines(1)", "\nfirst\nsecond\n", "\nfirst\n"),
    ("top_non_empty_lines(9)", " \n", ""),
    ("osc_title", "wrong screen", "actual title"),
    ("osc_progress", "wrong screen", "actual progress"),
];
use super::*;

#[test]
fn validation_accepts_exact_complexity_boundaries() {
    let rule = "[[rules]]\nid = 'test'\ncontains = ['x']\n";
    assert!(parse_manifest(&format!("id = 'codex'\n{}", rule.repeat(128))).is_ok());
    let mut gate = "{ contains = ['x'] }".to_owned();
    for _ in 0..7 {
        gate = format!("{{ all = [{gate}] }}");
    }
    let at_depth = format!("id = 'codex'\n[[rules]]\nid = 'depth'\nall = [{gate}]\n");
    assert!(parse_manifest(&at_depth).is_ok());
    assert!(
        parse_manifest(&at_depth.replace(
            &format!("all = [{gate}]"),
            &format!("all = [{{ all = [{gate}] }}]")
        ))
        .unwrap_err()
        .contains("max gate depth 8")
    );
    for (count, valid) in [(511, true), (512, false)] {
        let children = vec!["{contains = ['x']}"; count].join(",");
        let document = format!("id = 'codex'\n[[rules]]\nid = 'gates'\nall = [{children}]\n");
        assert_eq!(parse_manifest(&document).is_ok(), valid);
    }
    for (count, valid) in [(32, true), (33, false)] {
        let needles = vec!["'x'"; count].join(",");
        assert_eq!(
            parse_manifest(&format!(
                "id = 'codex'\n[[rules]]\nid = 'matchers'\ncontains = [{needles}]\n"
            ))
            .is_ok(),
            valid
        );
    }
    let needles = vec!["'x'"; 32].join(",");
    let document = format!(
        "id = 'codex'\n{}",
        format!("[[rules]]\nid = 'many'\ncontains = [{needles}]\n").repeat(32)
    );
    assert!(parse_manifest(&document).is_ok());
    assert!(
        parse_manifest(&(document + rule))
            .unwrap_err()
            .contains("max matcher count 1024")
    );
    for (count, valid) in [(512, true), (513, false)] {
        assert_eq!(
            parse_manifest(&format!(
                "id = 'codex'\n[[rules]]\nid = 'unicode'\ncontains = ['{}']\n",
                "界".repeat(count)
            ))
            .is_ok(),
            valid
        );
    }
}

#[test]
fn region_evidence_keeps_exact_source_slices() {
    let mut fixture = Fixture::new();

    for &(selector, screen, expected) in REGION_CASES {
        fixture.write_local(&format!(
            "id = 'codex'\n[[rules]]\nid = 'slice'\nregion = {selector:?}\ncontains = ['']\n"
        ));
        let result = fixture.cache.explain_with_input(
            Agent::Codex,
            DetectionInput {
                screen,
                osc_title: "actual title",
                osc_progress: "actual progress",
            },
        );
        assert_eq!(
            result.evaluated_rules[0].evidence.region_preview, expected,
            "selector {selector}"
        );
        assert_eq!(
            result.evaluated_rules[0].evidence.region_bytes,
            expected.len(),
            "selector {selector}"
        );
    }
}

#[test]
fn explain_json_preserves_absence_labels_and_preview_boundaries() {
    let fixture = Fixture::new();
    let unknown = fixture
        .cache
        .explain_for_label("not-an-agent", "ordinary screen");
    assert_eq!(
        explain_to_json_value(&unknown),
        serde_json::json!({
            "agent": "not-an-agent", "state": "unknown", "manifest_source": null,
            "manifest_version": null, "cached_remote_version": null, "local_override_shadowing_remote": false,
            "remote_update_status": null, "remote_update_error": null, "matched_rule": null,
            "visible_idle": false, "visible_blocker": false, "visible_working": false,
            "screen_detection_skipped": false, "skip_state_update": false, "skipped_update_reason": null,
            "fallback_reason": "unknown_agent", "warning": null, "evaluated_rules": []
        })
    );
    for (state, label) in [
        (AgentState::Idle, "idle"),
        (AgentState::Working, "working"),
        (AgentState::Blocked, "blocked"),
        (AgentState::Unknown, "unknown"),
    ] {
        assert_eq!(agent_state_label(state), label);
    }
    let unsupported = fixture.cache.explain(Agent::Omp, "ordinary screen");
    assert_eq!(unsupported.state, AgentState::Idle);
    assert_eq!(unsupported.source, None);
    assert_eq!(
        unsupported.fallback_reason.as_deref(),
        Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK)
    );
    let mut fixture = fixture;
    fixture.write_local("id = 'codex'\n[[rules]]\nid = 'preview'\ncontains = ['界']\n");
    for count in [239, 240, 241] {
        let result = fixture.cache.explain(Agent::Codex, &"界".repeat(count));
        let evidence = &result.evaluated_rules[0].evidence;
        assert_eq!(evidence.region_bytes, count * 3);
        assert_eq!(
            evidence.region_preview,
            if count > 240 {
                format!("{}...", "界".repeat(240))
            } else {
                "界".repeat(count)
            }
        );
        assert_eq!(evidence.contains, ["界"]);
        assert_eq!(
            (evidence.all_count, evidence.any_count, evidence.not_count),
            (0, 0, 0)
        );
    }
}

#[test]
fn independent_caches_reload_only_their_own_roots() {
    let mut first = Fixture::new();
    let mut second = Fixture::new();
    first.write_remote(&remote_manifest("9999.1", "working", "ready"));
    second.write_remote(&remote_manifest("9999.2", "blocked", "ready"));
    first.write_remote_without_reload(&remote_manifest("9999.3", "idle", "ready"));
    second.cache.reload_manifests().unwrap();
    assert_eq!(
        first.cache.detect(Agent::Codex, "ready").state,
        AgentState::Working
    );
    assert_eq!(
        second.cache.detect(Agent::Codex, "ready").state,
        AgentState::Blocked
    );
    first.cache.reload_manifests().unwrap();
    assert_eq!(
        first.cache.detect(Agent::Codex, "ready").state,
        AgentState::Idle
    );
    assert_eq!(
        second.cache.detect(Agent::Codex, "ready").state,
        AgentState::Blocked
    );
}

#[test]
fn sources_preserve_equal_versions_aliases_and_invalid_neighbors() {
    let mut fixture = Fixture::new();
    let remote = remote_manifest("9999.1", "working", "ready");
    for (document, expected_error) in [
        (
            remote.replace("id = \"codex\"", "id = \"gemini\""),
            "does not match codex",
        ),
        (
            remote.replace("version = \"9999.1\"", ""),
            "must include version",
        ),
        (
            remote.replace("min_engine_version = 1", ""),
            "must include min_engine_version",
        ),
        (
            remote.replace("min_engine_version = 1", "min_engine_version = 4"),
            "requires engine 4",
        ),
    ] {
        fixture.write_remote(&document);
        let explanation = fixture.cache.explain(Agent::Codex, "ready");
        assert!(matches!(explanation.source, Some(ManifestSource::Bundled)));
        assert!(
            explanation
                .warning
                .as_deref()
                .unwrap()
                .contains(expected_error)
        );
    }
    fixture.write_remote(&remote.replace("id = \"codex\"", "id = \"other\"\naliases = ['codex']"));
    assert_eq!(
        fixture.cache.detect(Agent::Codex, "ready").state,
        AgentState::Working
    );
    fixture.write_local("id = 'gemini'\n[[rules]]\nid = 'wrong'\ncontains = ['ready']");
    let result = fixture.cache.explain(Agent::Codex, "ready");
    assert_eq!(result.state, AgentState::Working);
    assert!(
        result
            .warning
            .as_deref()
            .unwrap()
            .contains("manifest id gemini does not match codex")
    );
    std::fs::remove_file(fixture.overrides.join("codex.toml")).unwrap();
    std::fs::create_dir(fixture.overrides.join("codex.toml")).unwrap();
    fixture.cache.reload_manifests().unwrap();
    assert!(
        fixture
            .cache
            .explain(Agent::Codex, "ready")
            .warning
            .as_deref()
            .unwrap()
            .contains("could not be loaded")
    );
}

#[test]
fn equal_priority_keeps_first_rule_and_independent_visibility() {
    let mut fixture = Fixture::new();
    fixture.write_local("id = 'codex'\n[[rules]]\nid = 'first'\nstate = 'working'\npriority = 7\ncontains = ['Ä']\nvisible_idle = true\nvisible_blocker = true\nvisible_working = true\n[[rules]]\nid = 'second'\nstate = 'blocked'\npriority = 7\ncontains = ['ä']");
    let result = fixture.cache.explain(Agent::Codex, "ä");
    assert_eq!(result.matched_rule.as_ref().unwrap().id, "first");
    assert_eq!(result.state, AgentState::Working);
    assert_eq!(
        result.visible,
        crate::agents::AgentVisibility {
            visible_idle: false,
            visible_blocker: false,
            visible_working: true
        }
    );
    assert_eq!(result.source.as_ref().unwrap().kind(), "local override");
    assert_eq!(
        result.source.as_ref().unwrap().label(),
        fixture.overrides.join("codex.toml").display().to_string()
    );
    assert_eq!(ManifestSource::Bundled.kind(), "bundled");
    let remote = ManifestSource::Remote {
        path: "cache.toml".into(),
        version: "1".to_owned(),
    };
    assert_eq!(remote.kind(), "remote");
    assert_eq!(remote.label(), "remote:cache.toml");
}

#[test]
fn declared_and_local_top_region_engine_neighbors_validate() {
    let engine = "id = 'codex'\nmin_engine_version = 3\n[[rules]]\nid = 'top'\nregion = 'top_non_empty_lines(1)'\ncontains = ['ready']";
    assert!(parse_manifest(engine).is_ok());
    assert!(parse_manifest(&engine.replace("min_engine_version = 3\n", "")).is_ok());
}

#[test]
fn equal_download_and_bundle_versions_select_download() {
    let mut fixture = Fixture::new();
    fixture.write_remote(&remote_manifest("2026.08.09.1", "blocked", "ready"));
    let equal = fixture.cache.explain(Agent::Codex, "ready");
    assert!(matches!(equal.source, Some(ManifestSource::Remote { .. })));
    assert_eq!(equal.state, AgentState::Blocked);
}
