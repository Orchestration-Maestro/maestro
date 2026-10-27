use super::*;

fn absolute_test_path(name: &str) -> String {
    std::env::current_dir()
        .unwrap()
        .join(name)
        .display()
        .to_string()
}

#[test]
fn pane_native_state_reservation_excludes_full_lifecycle_sources() {
    assert!(is_reserved_native_state_source("maestro:claude", "claude"));
    assert!(is_reserved_native_state_source("maestro:codex", "codex"));
    assert!(is_reserved_native_state_source("maestro:devin", "devin"));
    assert!(!is_reserved_native_state_source("maestro:kimi", "kimi"));
    assert!(!is_reserved_native_state_source(
        "maestro:opencode",
        "opencode"
    ));
}

#[test]
fn pane_planner_allows_supported_agents() {
    let path = absolute_test_path("session.jsonl");
    for &(source, agent, value, expected) in RESUME_CASES {
        assert_eq!(
            plan(source, agent, &AgentSessionRef::id(value).unwrap())
                .unwrap()
                .argv,
            expected
        );
    }
    assert_eq!(
        plan("maestro:pi", "pi", &AgentSessionRef::path(&path).unwrap())
            .unwrap()
            .argv,
        ["pi", "--session", &path]
    );
    assert_eq!(
        plan("maestro:omp", "omp", &AgentSessionRef::path(&path).unwrap())
            .unwrap()
            .argv,
        ["omp", &format!("--resume={path}")]
    );
}

#[test]
fn pane_planner_rejects_custom_and_unsupported_path_refs() {
    let claude_session = absolute_test_path("claude-session");
    assert!(
        plan(
            "custom:claude",
            "claude",
            &AgentSessionRef::id("session").unwrap()
        )
        .is_none()
    );
    assert!(
        plan(
            "maestro:claude",
            "claude",
            &AgentSessionRef::path(&claude_session).unwrap()
        )
        .is_none()
    );
}

#[test]
fn pane_report_ref_prefers_path_agent_and_alternate_path_agent_paths_and_validates_values() {
    report_paths_and_fallback();
    report_identifier_only_agents();
}

#[test]
fn pane_normalize_session_start_source_allows_known_values() {
    assert_eq!(
        normalize_session_start_source(Some("startup".into())),
        Some("startup".into())
    );
    assert_eq!(
        normalize_session_start_source(Some("resume".into())),
        Some("resume".into())
    );
    assert_eq!(
        normalize_session_start_source(Some("clear".into())),
        Some("clear".into())
    );
    assert_eq!(
        normalize_session_start_source(Some("compact".into())),
        Some("compact".into())
    );
    assert_eq!(
        normalize_session_start_source(Some("branch".into())),
        Some("branch".into())
    );
    assert_eq!(
        normalize_session_start_source(Some("new".into())),
        Some("new".into())
    );
    assert_eq!(
        normalize_session_start_source(Some("fork".into())),
        Some("fork".into())
    );
    assert_eq!(
        normalize_session_start_source(Some("select".into())),
        Some("select".into())
    );
    assert_eq!(
        normalize_session_start_source(Some(" resume ".into())),
        Some("resume".into())
    );
    assert_eq!(normalize_session_start_source(Some("other".into())), None);
    assert_eq!(normalize_session_start_source(None), None);
}

#[test]
fn pane_ids_are_data_not_shell_text() {
    let id = "abc; rm -rf /";
    let codex_plan = plan("maestro:codex", "codex", &AgentSessionRef::id(id).unwrap()).unwrap();
    assert_eq!(codex_plan.argv, vec!["codex", "resume", id]);

    let copilot_plan = plan(
        "maestro:copilot",
        "copilot",
        &AgentSessionRef::id(id).unwrap(),
    )
    .unwrap();
    assert_eq!(copilot_plan.argv, vec!["copilot", "--resume=abc; rm -rf /"]);

    let devin_plan = plan("maestro:devin", "devin", &AgentSessionRef::id(id).unwrap()).unwrap();
    assert_eq!(devin_plan.argv, vec!["devin", "--resume", id]);
}

#[test]
fn pane_planner_rejects_path_refs_for_id_only_agents() {
    for agent in ["hermes", "opencode", "kilo", "copilot", "devin", "agy"] {
        let source = if agent == "agy" {
            "maestro:antigravity_cli".into()
        } else {
            format!("maestro:{agent}")
        };
        assert!(
            plan(
                &source,
                agent,
                &AgentSessionRef::path(absolute_test_path("session")).unwrap()
            )
            .is_none()
        );
    }
    for agent in [
        "mastracode",
        "hermes",
        "opencode",
        "kilo",
        "copilot",
        "devin",
        "agy",
    ] {
        let source = if agent == "agy" {
            "maestro:antigravity_cli".into()
        } else {
            format!("maestro:{agent}")
        };
        assert!(
            session_ref_from_snapshot(&source, agent, AgentSessionRefKind::Id, "session").is_some()
        );
    }
}

#[test]
fn session_reference_limits_use_utf8_bytes_and_literal_argv() {
    for (value, valid) in [
        (String::new(), false),
        ("x".into(), true),
        ("x".repeat(512), true),
        ("x".repeat(513), false),
        ("é".repeat(256), true),
        (format!("{}x", "é".repeat(256)), false),
        ("修复🙂".into(), true),
        ("x\0y".into(), false),
        ("x\ny".into(), false),
        ("x\u{7f}y".into(), false),
    ] {
        assert_eq!(AgentSessionRef::id(value).is_some(), valid);
    }
    // An absolute path starts at a drive on Windows; every case keeps its byte length.
    let root = if cfg!(windows) { "C:/" } else { "/" };
    let limit = 4096 - root.len();
    for (value, valid) in [
        (String::new(), false),
        ("relative.jsonl".into(), false),
        (format!("{root}{}", "x".repeat(limit)), true),
        (format!("{root}{}", "x".repeat(limit + 1)), false),
        (format!("{root}{}x", "é".repeat((limit - 1) / 2)), true),
        (format!("{root}{}", "é".repeat(limit / 2 + 1)), false),
        (format!("{root}修复🙂"), true),
        (format!("{root}bad\npath"), false),
    ] {
        assert_eq!(AgentSessionRef::path(value).is_some(), valid);
    }
    snapshot_provenance_and_literal_resume();
    let session = format!("{root}session");
    for (source, agent, kind, value) in [
        ("maestro:claude", "codex", AgentSessionRefKind::Id, "id"),
        (
            "custom:pi",
            "pi",
            AgentSessionRefKind::Path,
            session.as_str(),
        ),
        (
            "maestro:claude",
            "claude",
            AgentSessionRefKind::Path,
            session.as_str(),
        ),
        ("maestro:pi", "pi", AgentSessionRefKind::Path, "relative"),
        ("maestro:pi", "pi", AgentSessionRefKind::Id, ""),
    ] {
        assert!(session_ref_from_snapshot(source, agent, kind, value).is_none());
    }
}

fn snapshot_provenance_and_literal_resume() {
    // An absolute path starts at a drive on Windows.
    let literal = if cfg!(windows) {
        "C:/修复🙂; literal.jsonl"
    } else {
        "/修复🙂; literal.jsonl"
    };
    let canonical =
        session_ref_from_snapshot("maestro:pi", "pi", AgentSessionRefKind::Path, literal).unwrap();
    let alias =
        session_ref_from_snapshot("herdr:pi", "pi", AgentSessionRefKind::Path, literal).unwrap();
    assert_eq!(alias, canonical);
    assert_eq!(canonical.source, "maestro:pi");
    assert_eq!(canonical.agent, "pi");
    assert_eq!(canonical.session_ref.kind, AgentSessionRefKind::Path);
    for snapshot in [canonical, alias] {
        let resume = plan(&snapshot.source, &snapshot.agent, &snapshot.session_ref).unwrap();
        assert_eq!(resume.agent, "pi");
        assert_eq!(resume.argv, ["pi", "--session", literal]);
        assert_eq!(
            resume.dedupe_key,
            format!("maestro:pi\0pi\0Path\0{literal}")
        );
    }
}

fn report_paths_and_fallback() {
    let path = absolute_test_path("session.jsonl");
    for agent in ["pi", "omp"] {
        let source = format!("maestro:{agent}");
        let session =
            session_ref_from_report(&source, agent, Some("id".into()), Some(path.clone())).unwrap();
        assert_eq!(session.kind, AgentSessionRefKind::Path);
        assert_eq!(session.value, path);
        assert!(
            session_ref_from_report(&source, agent, None, Some("relative.jsonl".into())).is_none()
        );
    }
    assert!(session_ref_from_report("maestro:pi", "pi", Some("bad\nid".into()), None).is_none());
    assert!(session_ref_from_report("custom:pi", "pi", Some("pi-id".into()), None).is_none());
    for path in [None, Some("relative.jsonl".into())] {
        let session =
            session_ref_from_report("maestro:omp", "omp", Some("omp-id".into()), path).unwrap();
        assert_eq!(session.kind, AgentSessionRefKind::Id);
        assert_eq!(session.value, "omp-id");
    }
}

fn report_identifier_only_agents() {
    for (agent, id) in [
        ("copilot", "copilot-id"),
        ("devin", "devin-id"),
        ("droid", "droid-id"),
        ("kimi", "kimi-id"),
        ("mastracode", "mastracode-id"),
        ("kilo", "kilo-id"),
        ("qodercli", "qoder-id"),
        ("qwen", "qwen-id"),
        ("agy", "agy-id"),
    ] {
        let source = if agent == "agy" {
            "maestro:antigravity_cli".into()
        } else {
            format!("maestro:{agent}")
        };
        let session = session_ref_from_report(&source, agent, Some(id.into()), None).unwrap();
        assert_eq!(session.kind, AgentSessionRefKind::Id);
        assert_eq!(session.value, id);
    }
    for agent in ["claude", "copilot", "droid"] {
        assert!(
            session_ref_from_report(
                &format!("maestro:{agent}"),
                agent,
                None,
                Some(absolute_test_path("session"))
            )
            .is_none()
        );
    }
}

const RESUME_CASES: &[(&str, &str, &str, &[&str])] = &[
    (
        "maestro:claude",
        "claude",
        "claude-session",
        &["claude", "--resume", "claude-session"],
    ),
    (
        "maestro:codex",
        "codex",
        "codex-session",
        &["codex", "resume", "codex-session"],
    ),
    (
        "maestro:copilot",
        "copilot",
        "copilot-session",
        &["copilot", "--resume=copilot-session"],
    ),
    (
        "maestro:devin",
        "devin",
        "devin-session",
        &["devin", "--resume", "devin-session"],
    ),
    (
        "maestro:droid",
        "droid",
        "droid-session",
        &["droid", "--resume", "droid-session"],
    ),
    (
        "maestro:kimi",
        "kimi",
        "kimi-session",
        &["kimi", "--session", "kimi-session"],
    ),
    (
        "maestro:mastracode",
        "mastracode",
        "mastracode-session",
        &["mastracode", "--thread", "mastracode-session"],
    ),
    (
        "maestro:hermes",
        "hermes",
        "hermes-session",
        &["hermes", "--resume", "hermes-session"],
    ),
    (
        "maestro:opencode",
        "opencode",
        "opencode-session",
        &["opencode", "--session", "opencode-session"],
    ),
    (
        "maestro:qodercli",
        "qodercli",
        "qoder-session",
        &["qodercli", "--resume", "qoder-session"],
    ),
    (
        "maestro:qwen",
        "qwen",
        "qwen-session",
        &["qwen", "--resume", "qwen-session"],
    ),
    (
        "maestro:kilo",
        "kilo",
        "kilo-session",
        &["kilo", "--session", "kilo-session"],
    ),
    (
        "maestro:cursor",
        "cursor",
        "cursor-session",
        &[
            if cfg!(windows) {
                "cursor-agent.cmd"
            } else {
                "cursor-agent"
            },
            "--resume",
            "cursor-session",
        ],
    ),
    (
        "maestro:antigravity_cli",
        "agy",
        "agy-session",
        &["agy", "--conversation", "agy-session"],
    ),
    (
        "maestro:grok",
        "grok",
        "grok-session",
        &["grok", "--resume", "grok-session"],
    ),
];
