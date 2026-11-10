use super::validation::validate_region_name;
use super::*;
use crate::tests::Fixture;
use std::fmt::Write as _;
fn remote_manifest(version: &str, state: &str, contains: &str) -> String {
    format!(
        r#"
id = "codex"
version = "{version}"
min_engine_version = 1
updated_at = "2026-06-10T12:00:00Z"

[[rules]]
id = "test"
state = "{state}"
contains = ["{contains}"]
"#
    )
}

fn local_manifest(state: &str, contains: &str) -> String {
    format!(
        r#"
id = "codex"

[[rules]]
id = "test"
state = "{state}"
contains = ["{contains}"]
"#
    )
}

fn rules_manifest(rules: &str) -> String {
    format!(
        r#"
id = "codex"

{rules}
"#
    )
}

#[test]
fn known_agent_fallback_is_idle_without_live_evidence() {
    let cache = ManifestCache::new(None, None).unwrap();
    let explanation = cache.explain(Agent::Codex, "ordinary prompt text");
    assert_eq!(explanation.state, AgentState::Idle);
    assert!(!explanation.visible.visible_idle);
    assert_eq!(
        explanation.fallback_reason.as_deref(),
        Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK)
    );
}
#[test]
fn matcher_gates_select_the_highest_priority_rule() {
    let mut fixture = Fixture::new();
    fixture.write_local(&rules_manifest(
        r#"
[[rules]]
id = "low_contains"
state = "idle"
priority = 1
contains = ["match"]

[[rules]]
id = "high_nested_gates"
state = "working"
priority = 10
contains = ["match"]
all = [
  { any = [{ regex = ["w[io]n"] }, { contains = ["fallback"] }] },
]
not = [
  { contains = ["blocked"] },
]

[[rules]]
id = "line_regex"
state = "blocked"
priority = 20
line_regex = ["^exact line$"]
"#,
    ));

    let high = fixture.cache.explain(Agent::Codex, "match win");
    assert_eq!(high.state, AgentState::Working);
    assert_eq!(
        high.matched_rule.as_ref().map(|rule| rule.id.as_str()),
        Some("high_nested_gates")
    );

    let not_gate = fixture.cache.explain(Agent::Codex, "match win blocked");
    assert_eq!(not_gate.state, AgentState::Idle);
    assert_eq!(
        not_gate.matched_rule.as_ref().map(|rule| rule.id.as_str()),
        Some("low_contains")
    );

    let line = fixture
        .cache
        .explain(Agent::Codex, "before\nexact line\nafter");
    assert_eq!(line.state, AgentState::Blocked);
    assert_eq!(
        line.matched_rule.as_ref().map(|rule| rule.id.as_str()),
        Some("line_regex")
    );
}
#[test]
fn downloaded_rules_precede_bundled_rules() {
    let mut fixture = Fixture::new();
    fixture.write_remote(&remote_manifest("9999.01.01.1", "blocked", "remote-ready"));

    let explain = fixture.cache.explain(Agent::Codex, "remote-ready");

    assert_eq!(explain.state, AgentState::Blocked);
    assert!(matches!(
        explain.source,
        Some(ManifestSource::Remote { .. })
    ));
    assert_eq!(explain.manifest_version.as_deref(), Some("9999.01.01.1"));
    assert_eq!(
        explain.cached_remote_version.as_deref(),
        Some("9999.01.01.1")
    );
}

#[test]
fn idle_fallback_retains_active_manifest_metadata() {
    let mut fixture = Fixture::new();
    fixture.write_remote(&remote_manifest("9999.01.01.1", "blocked", "remote-ready"));

    let explain = fixture.cache.explain(Agent::Codex, "ordinary prompt text");

    assert_eq!(explain.state, AgentState::Idle);
    assert_eq!(
        explain.fallback_reason.as_deref(),
        Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK)
    );
    assert_eq!(explain.manifest_version.as_deref(), Some("9999.01.01.1"));
    assert!(matches!(
        explain.source,
        Some(ManifestSource::Remote { .. })
    ));
}

#[test]
fn older_download_keeps_newer_bundle_active() {
    let mut fixture = Fixture::new();
    fixture.write_remote(&remote_manifest("2026.06.10.0", "blocked", "remote-ready"));

    let explain = fixture.cache.explain(Agent::Codex, "remote-ready");

    assert_eq!(explain.state, AgentState::Idle);
    assert!(matches!(explain.source, Some(ManifestSource::Bundled)));
    assert_eq!(
        explain.cached_remote_version.as_deref(),
        Some("2026.06.10.0")
    );
    assert!(
        explain
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("older than bundled"))
    );
}

#[test]
fn override_keeps_precedence_over_download() {
    let mut fixture = Fixture::new();
    fixture.write_remote(&remote_manifest("9999.01.01.1", "blocked", "remote-ready"));
    fixture.write_local(&local_manifest("idle", "local-ready"));

    let explain = fixture.cache.explain(Agent::Codex, "local-ready");

    assert_eq!(explain.state, AgentState::Idle);
    assert!(matches!(explain.source, Some(ManifestSource::Override(_))));
    assert!(explain.local_override_shadowing_remote);
    assert_eq!(
        explain.cached_remote_version.as_deref(),
        Some("9999.01.01.1")
    );
}

#[test]
fn invalid_override_keeps_download_with_warning() {
    let mut fixture = Fixture::new();
    fixture.write_remote(&remote_manifest("9999.01.01.1", "blocked", "remote-ready"));
    fixture.write_local("id = ");

    let explain = fixture.cache.explain(Agent::Codex, "remote-ready");

    assert_eq!(explain.state, AgentState::Blocked);
    assert!(matches!(
        explain.source,
        Some(ManifestSource::Remote { .. })
    ));
    assert!(explain.warning.is_some());
}

#[test]
fn cached_rules_change_only_after_reload() {
    let mut fixture = Fixture::new();
    fixture.write_remote(&remote_manifest("9999.01.01.1", "blocked", "cached-ready"));

    let cached = fixture.cache.explain(Agent::Codex, "cached-ready");
    assert_eq!(cached.state, AgentState::Blocked);
    assert!(matches!(cached.source, Some(ManifestSource::Remote { .. })));
    assert_eq!(
        cached.matched_rule.as_ref().map(|rule| rule.id.as_str()),
        Some("test")
    );

    fixture.write_remote_without_reload(&remote_manifest("9999.01.01.2", "working", "new-ready"));

    let unchanged = fixture.cache.explain(Agent::Codex, "new-ready");
    assert_eq!(unchanged.state, AgentState::Idle);
    assert_eq!(
        unchanged.fallback_reason.as_deref(),
        Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK)
    );
    assert_eq!(
        unchanged.cached_remote_version.as_deref(),
        Some("9999.01.01.1")
    );

    fixture.cache.reload_manifests().unwrap();

    let reloaded = fixture.cache.explain(Agent::Codex, "new-ready");
    assert_eq!(reloaded.state, AgentState::Working);
    assert_eq!(
        reloaded.cached_remote_version.as_deref(),
        Some("9999.01.01.2")
    );
    assert_eq!(
        reloaded.matched_rule.as_ref().map(|rule| rule.id.as_str()),
        Some("test")
    );
}

#[test]
fn bundled_catalogue_entries_validate() {
    let fixture = Fixture::new();
    for agent in Agent::SCREEN_MANIFEST_AGENTS {
        assert!(
            fixture
                .cache
                .manifest_summaries()
                .iter()
                .any(|summary| summary.agent == agent),
            "missing bundled manifest for {}",
            agent_label(agent)
        );
    }
}

#[test]
fn bundled_screen_rules_keep_all_status_and_prompt_variants() {
    let fixture = Fixture::new();
    let idle = fixture.cache.explain(
        Agent::Devin,
        "─────────────────────────────────────────────────────\n❭ Ask Devin to build features, fix bugs, or work on\n  your code\n─────────────────────────────────────────────────────\nSWE-1.6               Context: 16k / 200k tokens (7%)",
    );
    assert_eq!(idle.state, AgentState::Idle);
    assert!(idle.visible.visible_idle);

    let live_footer_idle = fixture.cache.explain(
        Agent::Devin,
        "Done.\n\n────────────────────────────────────────────────── (bypass permissions on) ─\n❭\n────────────────────────────────────────────────────────────────────────────\nClaude Opus 4.6 Thinking                                    Context: 38k / 200k tokens (18%)",
    );
    assert_eq!(live_footer_idle.state, AgentState::Idle);
    assert_eq!(
        live_footer_idle
            .matched_rule
            .as_ref()
            .map(|rule| rule.id.as_str()),
        Some("live_prompt_footer")
    );
    assert!(live_footer_idle.visible.visible_idle);

    let welcome_footer_idle = fixture.cache.explain(
        Agent::Devin,
        "⠀⠀⠀⠀⠀⣴⣾⣶⡄⠀⠀⠀⠀\n⠀⣴⣾⣶⡾⠛⠿⠟⠃⣴⣾⣶⡄  Devin CLI\n⠀⠛⠿⠟⠃⣴⣾⣶⡾⠛⠿⠟⠃  v2026.5.26-8\n⠀⣤⣶⣦⡄⠻⢿⠿⢷⣤⣶⣦⡄\n⠀⠻⢿⠿⢷⣤⣶⣦⡄⠻⢿⠿⠃  Hybrid\n⠀⠀⠀⠀⠀⠻⢿⠿⠃⠀⠀⠀⠀\n\n───────────────────────────\n❭ Ask Devin to build\n  features, fix bugs, or\n  work on your code\n───────────────────────────\nClaude Opus Looking for\n4.6 Thinkingplan mode? /\n            plan",
    );
    assert_eq!(welcome_footer_idle.state, AgentState::Idle);
    assert_eq!(
        welcome_footer_idle
            .matched_rule
            .as_ref()
            .map(|rule| rule.id.as_str()),
        Some("welcome_prompt_footer")
    );
    assert!(welcome_footer_idle.visible.visible_idle);

    let working = fixture.cache.explain(
        Agent::Devin,
        "◔ Reading shell 91b655\n  │ Timeout: 35s\n\n⠀⡆ Running tools · 27s (esc to interrupt)\n─────────────────────────────────────────────────────\n❭ Guide Devin while it works",
    );
    assert_eq!(working.state, AgentState::Working);
    assert!(working.visible.visible_working);

    let trust_prompt = fixture.cache.explain(
        Agent::Devin,
        "Do you trust the authors of this directory?\nFor security, devin should not be run in directories\nwith untrusted content.\n❭ 1 Yes, trust /private/tmp/devin-hook-probe\n· 2 No, exit",
    );
    assert_eq!(trust_prompt.state, AgentState::Blocked);
    assert!(trust_prompt.visible.visible_blocker);

    let permission_prompt = fixture.cache.explain(
        Agent::Devin,
        "⏺ Running command\n  └ $ sleep 30\n\n❭ 1 Yes  (Approve once)\n· 2 Yes, allow `sleep` commands\n· 3 Yes, always allow `sleep` commands\n· 4 No\n↑↓ select · ↵ confirm · esc cancel",
    );
    assert_eq!(permission_prompt.state, AgentState::Blocked);
    assert!(permission_prompt.visible.visible_blocker);
}

#[test]
fn malformed_rule_shapes_and_patterns_are_rejected() {
    assert!(
        parse_manifest(
            r#"
id = "codex"

[[rules]]
id = "typo"
state = "working"
contain = ["Working"]
"#
        )
        .is_err()
    );

    assert!(
        parse_manifest(
            r#"
id = "codex"

[[rules]]
id = "empty"
state = "working"
"#
        )
        .is_err()
    );

    assert!(
        parse_manifest(
            r#"
id = "codex"

[[rules]]
id = "bad_region"
state = "working"
region = "after_last_promt_marker"
contains = ["Working"]
"#
        )
        .is_err()
    );

    assert!(
        parse_manifest(
            r#"
id = "codex"

[[rules]]
id = "bad_regex"
state = "working"
regex = ["["]
"#
        )
        .is_err()
    );

    assert!(
        parse_manifest(
            r#"
id = "codex"

[[rules]]
id = "bad_nested_regex"
state = "working"
any = [{ line_regex = ["["] }]
"#
        )
        .is_err()
    );
}

#[test]
fn skip_rules_require_neutral_unknown_state() {
    assert!(
        parse_manifest(
            r#"
id = "codex"

[[rules]]
id = "bad_skip_state"
state = "idle"
skip_state_update = true
contains = ["menu"]
"#
        )
        .is_err()
    );

    assert!(
        parse_manifest(
            r#"
id = "codex"

[[rules]]
id = "bad_skip_visible"
state = "unknown"
skip_state_update = true
visible_blocker = true
contains = ["menu"]
"#
        )
        .is_err()
    );
}

#[test]
fn rule_count_limit_rejects_the_next_rule() {
    let mut manifest = String::from(
        r#"
id = "codex"
"#,
    );
    for index in 0..129 {
        write!(
            manifest,
            r#"
[[rules]]
id = "rule_{index}"
state = "idle"
contains = ["ready"]
"#
        )
        .unwrap();
    }

    assert!(parse_manifest(&manifest).is_err());
}

#[test]
fn gate_depth_limit_rejects_the_next_level() {
    let manifest = r#"
id = "codex"

[[rules]]
id = "deep"
state = "idle"
contains = ["ready"]
all = [
  { contains = ["1"], all = [
    { contains = ["2"], all = [
      { contains = ["3"], all = [
        { contains = ["4"], all = [
          { contains = ["5"], all = [
            { contains = ["6"], all = [
              { contains = ["7"], all = [
                { contains = ["8"], all = [
                  { contains = ["9"] },
                ] },
              ] },
            ] },
          ] },
        ] },
      ] },
    ] },
  ] },
]
"#;

    assert!(parse_manifest(manifest).is_err());
}

#[test]
fn matcher_limits_apply_per_gate_and_per_manifest() {
    let matchers = (0..33)
        .map(|index| format!(r#""m{index}""#))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = format!(
        r#"
id = "codex"

[[rules]]
id = "many"
state = "idle"
contains = [{matchers}]
"#
    );

    assert!(parse_manifest(&manifest).is_err());
}

#[test]
fn repeated_text_keeps_the_bottom_line_occurrence() {
    let content = "marker\nold\n\nmiddle\nmarker\nnew\n";

    assert_eq!(
        region(
            DetectionInput {
                screen: content,
                osc_title: "",
                osc_progress: "",
            },
            "bottom_non_empty_lines(2)"
        ),
        "marker\nnew\n"
    );
}

#[test]
fn repeated_text_keeps_the_top_line_occurrence() {
    let content = "\nmarker\nold\n\nmiddle\nmarker\nnew\n";

    assert_eq!(
        region(
            DetectionInput {
                screen: content,
                osc_title: "",
                osc_progress: "",
            },
            "top_non_empty_lines(2)"
        ),
        "\nmarker\nold\n"
    );
}

#[test]
fn top_region_counts_are_positive_canonical_and_bounded() {
    let name = "top_non_empty_lines";
    assert!(validate_region_name(&format!("{name}(1)")).is_ok());
    assert!(validate_region_name(&format!("{name}({})", u16::MAX)).is_ok());
    for count in ["0", "01", "+1", "65536", "999999999999999999999999"] {
        assert!(
            validate_region_name(&format!("{name}({count})")).is_err(),
            "{name} accepted invalid count {count}"
        );
    }
}

#[test]
fn top_regions_require_the_declared_engine_level() {
    let manifest = r#"
id = "grok"
version = "1"
min_engine_version = 2

[[rules]]
id = "background"
state = "working"
region = " top_non_empty_lines(1) "
contains = ["active"]
"#;

    assert!(parse_manifest(manifest).is_err());
}

fn osc_explain(
    agent: Agent,
    screen: &str,
    osc_title: &str,
    osc_progress: &str,
) -> DetectionExplain {
    let fixture = Fixture::new();
    fixture.cache.explain_with_input(
        agent,
        DetectionInput {
            screen,
            osc_title,
            osc_progress,
        },
    )
}

#[test]
fn braille_title_supplies_live_working_evidence() {
    let result = osc_explain(Agent::Claude, "", "⠂ project", "");
    assert_eq!(result.state, AgentState::Working);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("osc_title_working")
    );
    assert!(result.visible.visible_working);
}

#[test]
fn every_half_circle_title_frame_supplies_working_evidence() {
    for frame in ['◐', '◓', '◑', '◒'] {
        let title = format!("{frame} Initial conversation with Claude");
        let result = osc_explain(Agent::Claude, "", &title, "");
        assert_eq!(result.state, AgentState::Working, "frame {frame}");
        assert_eq!(
            result.matched_rule.as_ref().map(|rule| rule.id.as_str()),
            Some("osc_title_working"),
            "frame {frame}"
        );
        assert!(result.visible.visible_working, "frame {frame}");
    }
}

#[test]
fn static_title_prefix_supplies_live_idle_evidence() {
    let result = osc_explain(Agent::Claude, "", "✳ Claude Code", "");
    assert_eq!(result.state, AgentState::Idle);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("osc_title_idle")
    );
    assert!(result.visible.visible_idle);
}

#[test]
fn stale_progress_without_screen_evidence_stays_fallback_idle() {
    let result = osc_explain(Agent::Claude, "", "", "4;3;");
    assert_eq!(result.state, AgentState::Idle);
    assert_eq!(
        result.fallback_reason.as_deref(),
        Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK)
    );
    assert!(!result.visible.visible_working);
}

#[test]
fn permission_screen_beats_stale_progress() {
    let blocker_screen =
        "──────────\n  1. Yes\n  2. No\n\nEnter to select · ↑/↓ to navigate · Esc to cancel\n";
    let result = osc_explain(Agent::Claude, blocker_screen, "✳ Task title", "4;3;");
    assert_eq!(result.state, AgentState::Blocked);
    assert!(result.visible.visible_blocker);
}

#[test]
fn completed_progress_selects_the_idle_rule() {
    let result = osc_explain(Agent::Claude, "", "", "4;0;");
    assert_eq!(result.state, AgentState::Idle);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("osc_progress_idle")
    );
}

#[test]
fn permission_screen_beats_an_idle_title() {
    let blocker_screen = "do you want to proceed?\n\
        bash command: rm -rf /tmp/test\n\
        ❯ 1. Yes\n   2. No\n\n\
        Esc to cancel · Tab to amend · ctrl+e to explain\n";
    let result = osc_explain(Agent::Claude, blocker_screen, "✳ Claude Code", "");
    assert_eq!(result.state, AgentState::Blocked);
    assert!(result.visible.visible_blocker);
}

#[test]
fn empty_title_progress_and_screen_keep_idle_fallback() {
    let result = osc_explain(Agent::Claude, "", "", "");
    assert_eq!(result.state, AgentState::Idle);
    assert_eq!(
        result.fallback_reason.as_deref(),
        Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK)
    );
    assert!(!result.visible.visible_idle);
}

#[test]
fn title_spinner_selects_the_working_rule() {
    let result = osc_explain(Agent::Codex, "", "⠋ llm-proxy", "");
    assert_eq!(result.state, AgentState::Working);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("osc_title_working")
    );
    assert!(result.visible.visible_working);
}

#[test]
fn action_required_title_selects_the_blocked_rule() {
    let result = osc_explain(Agent::Codex, "", "[ . ] Action Required | llm-proxy", "");
    assert_eq!(result.state, AgentState::Blocked);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("osc_title_blocked")
    );
    assert!(result.visible.visible_blocker);
}

#[test]
fn plain_title_selects_live_idle_evidence() {
    let result = osc_explain(Agent::Codex, "", "llm-proxy", "");
    assert_eq!(result.state, AgentState::Idle);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("osc_title_idle")
    );
    assert!(result.visible.visible_idle);
}

#[test]
fn trust_prompt_requires_live_top_region() {
    let screen = "> You are in C:\\Users\\user\\project\n\n\
        Do you trust the contents of this\n\
        directory? Working with untrusted\n\
        contents comes with higher risk of\n\
        prompt injection. Trusting the\n\
        directory allows project-local config,\n\
        hooks, and exec policies to load.\n\n\
        › 1. Yes, continue\n\
          2. No, quit\n\n\
        Press enter to continue\n";
    let result = osc_explain(Agent::Codex, screen, "project", "");

    assert_eq!(result.state, AgentState::Blocked);
    assert_eq!(
        result.matched_rule.as_ref().map(|rule| rule.id.as_str()),
        Some("trust_directory")
    );
    assert!(result.visible.visible_blocker);

    let transcript = "› > You are in C:\\Users\\user\\project\n\n\
        Do you trust the contents of this\n\
        directory? Working with untrusted contents comes with higher risk.\n";
    let result = osc_explain(Agent::Codex, transcript, "project", "");

    assert_eq!(result.state, AgentState::Idle);
    assert_ne!(
        result.matched_rule.as_ref().map(|rule| rule.id.as_str()),
        Some("trust_directory")
    );
    assert!(!result.visible.visible_blocker);
}

#[test]
fn background_terminal_text_keeps_title_idle() {
    let screen = "background terminal running · /ps to view · /stop to close\n";
    let result = osc_explain(Agent::Codex, screen, "llm-proxy", "");
    assert_eq!(result.state, AgentState::Idle);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("osc_title_idle")
    );
    assert!(result.visible.visible_idle);
}

#[test]
fn live_screen_working_overrides_a_static_title() {
    let screen = "• I’ll run it and wait for completion.\n\n\
        ◦ Working (1m 16s • esc to interrupt) · 1 background…\n\n\
        › Use /skills to list available skills\n\n\
        gpt-5.6-sol default · /work\n";
    let result = osc_explain(Agent::Codex, screen, "project", "");

    assert_eq!(result.state, AgentState::Working);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("screen_working_fallback")
    );
    assert!(result.visible.visible_working);
}

#[test]
fn title_working_rule_beats_screen_working_fallback() {
    let screen = "• Working (4s • esc to interrupt)\n\n\
        › Use /skills to list available skills\n\n\
        gpt-5.6-sol default · /work\n";
    let result = osc_explain(Agent::Codex, screen, "⠸ project", "");

    assert_eq!(result.state, AgentState::Working);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("osc_title_working")
    );
    assert!(result.visible.visible_working);
}

#[test]
fn strong_screen_blocker_beats_working_fallback() {
    let screen = "• Working (4s • esc to interrupt)\n\
        › 1. Yes, proceed\n\
        Press enter to confirm or esc to cancel\n";
    let result = osc_explain(Agent::Codex, screen, "project", "");

    assert_eq!(result.state, AgentState::Blocked);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("live_strong_blocker")
    );
    assert!(result.visible.visible_blocker);
    assert!(!result.visible.visible_working);
}

#[test]
fn weak_screen_blocker_beats_working_fallback() {
    let screen = "• Working (4s • esc to interrupt)\n\
        do you want to continue? [y/n]\n\
        › Use /skills to list available skills\n";
    let result = osc_explain(Agent::Codex, screen, "project", "");

    assert_eq!(result.state, AgentState::Blocked);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("weak_blocker")
    );
    assert!(!result.visible.visible_working);
}

#[test]
fn transcript_viewer_suppresses_working_updates() {
    let screen = "• Working (4s • esc to interrupt)\n\
        › transcript\n\
        ↑/↓ to scroll · pgup/pgdn to move · home/end to jump · q to quit · esc to edit prev\n";
    let result = osc_explain(Agent::Codex, screen, "project", "");

    assert_eq!(result.state, AgentState::Unknown);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("transcript_viewer")
    );
    assert!(result.skip_state_update);
    let fixture = Fixture::new();
    assert!(crate::should_skip_state_update(
        &fixture.cache,
        Some(Agent::Codex),
        screen
    ));
    assert!(!result.visible.visible_working);
}

#[test]
fn stale_quoted_and_indented_working_text_stays_idle() {
    let screens = [
        "◦ Working (1m 16s • esc to interrupt)\n\
         ■ Conversation interrupted\n\
         › Use /skills to list available skills\n\
         gpt-5.6-sol default · /work\n",
        "› Explain the text ◦ Working (1m 16s • esc to interrupt)\n\
         gpt-5.6-sol default · /work\n",
        "  ◦ Working (1m 16s • esc to interrupt)\n\
         › Use /skills to list available skills\n\
         gpt-5.6-sol default · /work\n",
    ];

    for screen in screens {
        let result = osc_explain(Agent::Codex, screen, "project", "");
        assert_eq!(result.state, AgentState::Idle);
        assert_eq!(
            result.matched_rule.as_ref().map(|r| r.id.as_str()),
            Some("osc_title_idle")
        );
        assert!(result.visible.visible_idle);
        assert!(!result.visible.visible_working);
    }
}

#[test]
fn interrupted_short_screen_stays_idle() {
    let screen = "◦ Working (1m 16s • esc to interrupt)\n\
        ■ Conversation interrupted\n\
        ›\n";
    let result = osc_explain(Agent::Codex, screen, "project", "");

    assert_eq!(result.state, AgentState::Idle);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("osc_title_idle")
    );
    assert!(result.visible.visible_idle);
    assert!(!result.visible.visible_working);
}

#[test]
fn active_title_spinner_beats_stale_weak_blocker() {
    let screen = "do you want to continue? [y/n]\n";
    let result = osc_explain(Agent::Codex, screen, "⠋ llm-proxy", "");
    assert_eq!(result.state, AgentState::Working);
    assert_eq!(
        result.matched_rule.as_ref().map(|r| r.id.as_str()),
        Some("osc_title_working")
    );
}

mod boundaries;
