use super::*;

#[test]
fn known_process_labels_select_catalogue_agents() {
    for (label, expected) in [
        ("pi", Agent::Pi),
        ("claude", Agent::Claude),
        ("claude-code", Agent::Claude),
        ("codex", Agent::Codex),
        ("gemini", Agent::Gemini),
        ("cursor", Agent::Cursor),
        ("cursor-agent", Agent::Cursor),
        ("devin", Agent::Devin),
        ("devin-cli", Agent::Devin),
        ("agy", Agent::Antigravity),
        ("antigravity-cli", Agent::Antigravity),
        ("cline", Agent::Cline),
        ("omp", Agent::Omp),
        ("mastracode", Agent::Mastracode),
        ("mastra-code", Agent::Mastracode),
        ("opencode", Agent::OpenCode),
        ("opencode.exe", Agent::OpenCode),
        ("opencode2", Agent::OpenCode),
        ("opencode2.exe", Agent::OpenCode),
        ("kimi", Agent::Kimi),
        ("Kimi Code", Agent::Kimi),
        ("kiro", Agent::Kiro),
        ("kiro-cli", Agent::Kiro),
        ("copilot", Agent::GithubCopilot),
        ("ghcs", Agent::GithubCopilot),
        ("grok", Agent::Grok),
        ("grok-build", Agent::Grok),
        ("hermes", Agent::Hermes),
        ("hermes-agent", Agent::Hermes),
        ("kilo", Agent::Kilo),
        ("kilo-code", Agent::Kilo),
        ("qwen", Agent::Qwen),
        ("Qwen Code", Agent::Qwen),
        ("maki", Agent::Maki),
    ] {
        assert_eq!(identify_agent(label), Some(expected));
    }
}

#[test]
fn label_aliases_select_catalogue_agents() {
    for (label, expected) in [
        ("pi", Agent::Pi),
        ("claude", Agent::Claude),
        ("cursor-agent", Agent::Cursor),
        ("devin-cli", Agent::Devin),
        ("agy", Agent::Antigravity),
        ("antigravity", Agent::Antigravity),
        ("omp", Agent::Omp),
        ("mastracode", Agent::Mastracode),
        ("mastra code", Agent::Mastracode),
        ("opencode.exe", Agent::OpenCode),
        ("copilot", Agent::GithubCopilot),
        ("kimi-code", Agent::Kimi),
        ("github-copilot", Agent::GithubCopilot),
        ("amp-local", Agent::Amp),
        ("kiro-cli", Agent::Kiro),
        ("grok-build", Agent::Grok),
        ("hermes-agent", Agent::Hermes),
        ("qwen-code", Agent::Qwen),
        ("maki", Agent::Maki),
        ("kilo-code", Agent::Kilo),
    ] {
        assert_eq!(parse_agent_label(label), Some(expected));
    }
}

#[test]
fn catalogue_labels_round_trip_through_both_parsers() {
    for agent in Agent::ALL {
        let label = agent_label(agent);
        assert_eq!(parse_canonical_agent_label(label), Some(agent));
        assert_eq!(parse_agent_label(label), Some(agent));
    }
}

#[test]
fn catalogue_entries_select_interactive_executables() {
    let expected = [
        (Agent::Pi, "pi"),
        (Agent::Claude, "claude"),
        (Agent::Codex, "codex"),
        (Agent::Gemini, "gemini"),
        (
            Agent::Cursor,
            if cfg!(windows) {
                "cursor-agent.cmd"
            } else {
                "cursor-agent"
            },
        ),
        (Agent::Devin, "devin"),
        (Agent::Antigravity, "agy"),
        (Agent::Cline, "cline"),
        (Agent::Omp, "omp"),
        (Agent::Mastracode, "mastracode"),
        (Agent::OpenCode, "opencode"),
        (Agent::GithubCopilot, "copilot"),
        (Agent::Kimi, "kimi"),
        (Agent::Kiro, "kiro-cli"),
        (Agent::Droid, "droid"),
        (Agent::Amp, "amp"),
        (Agent::Grok, "grok"),
        (Agent::Hermes, "hermes"),
        (Agent::Kilo, "kilo"),
        (Agent::Qodercli, "qodercli"),
        (Agent::Qwen, "qwen"),
        (Agent::Maki, "maki"),
    ];
    assert_eq!(expected.len(), Agent::ALL.len());
    for (agent, executable) in expected {
        assert_eq!(interactive_agent_executable(agent), executable);
    }
}

#[test]
fn canonical_lookup_rejects_aliases_and_normalized_spelling() {
    assert_eq!(parse_canonical_agent_label("claude-code"), None);
    assert_eq!(parse_canonical_agent_label("Pi"), None);
    assert_eq!(parse_canonical_agent_label(" pi "), None);
    assert_eq!(parse_canonical_agent_label("opencode.exe"), None);
}

#[test]
fn ordinary_programs_do_not_identify_as_agents() {
    assert_eq!(identify_agent("bash"), None);
    assert_eq!(identify_agent("zsh"), None);
    assert_eq!(identify_agent("vim"), None);
    assert_eq!(identify_agent("node"), None);
}

#[test]
fn process_label_lookup_ignores_case() {
    assert_eq!(identify_agent("Pi"), Some(Agent::Pi));
    assert_eq!(identify_agent("CLAUDE"), Some(Agent::Claude));
    assert_eq!(identify_agent("Codex"), Some(Agent::Codex));
    assert_eq!(identify_agent("Devin"), Some(Agent::Devin));
}

#[test]
fn hook_only_agent_has_no_screen_manifest() {
    assert!(full_lifecycle_hook_authority(
        "maestro:mastracode",
        "mastracode"
    ));
    assert!(!Agent::SCREEN_MANIFEST_AGENTS.contains(&Agent::Mastracode));
}

#[test]
fn identity_integrations_keep_screen_state_authority() {
    for (source, label, agent) in [
        ("maestro:hermes", "hermes", Agent::Hermes),
        ("maestro:qwen", "qwen", Agent::Qwen),
        ("maestro:antigravity_cli", "agy", Agent::Antigravity),
    ] {
        assert!(!full_lifecycle_hook_authority(source, label));
        assert!(session_identity_only_integration(source, label));
        assert!(Agent::SCREEN_MANIFEST_AGENTS.contains(&agent));
    }
}

#[test]
fn canonical_authority_pairs_retain_all_catalogue_classes() {
    for (suffix, label, lifecycle) in [
        ("pi", "pi", true),
        ("omp", "omp", true),
        ("mastracode", "mastracode", true),
        ("opencode", "opencode", true),
        ("kilo", "kilo", true),
        ("kimi", "kimi", true),
        ("hermes", "hermes", false),
        ("qwen", "qwen", false),
        ("antigravity_cli", "agy", false),
    ] {
        let canonical = format!("maestro:{suffix}");
        for source in [canonical.clone(), format!("herdr:{suffix}")] {
            let normalized = normalize_agent_source(source);
            assert_eq!(full_lifecycle_hook_authority(&normalized, label), lifecycle);
            assert_eq!(
                session_identity_only_integration(&normalized, label),
                !lifecycle
            );
        }
        for wrong_source in [format!("herdr:{suffix}"), format!("custom:{suffix}")] {
            assert!(!full_lifecycle_hook_authority(&wrong_source, label));
            assert!(!session_identity_only_integration(&wrong_source, label));
        }
        assert!(!full_lifecycle_hook_authority(&canonical, "unknown"));
        assert!(!session_identity_only_integration(&canonical, "unknown"));
    }
}

#[test]
fn agent_source_normalization_preserves_exact_domains() {
    for (source, expected) in [
        ("maestro:pi", "maestro:pi"),
        ("herdr:pi", "maestro:pi"),
        ("herdr:", "maestro:"),
        ("herdr:unknown", "maestro:unknown"),
        ("herdr:herdr:pi", "maestro:herdr:pi"),
        ("Herdr:pi", "Herdr:pi"),
        (" herdr:pi", " herdr:pi"),
        ("custom:pi", "custom:pi"),
        ("herdr", "herdr"),
        ("", ""),
    ] {
        assert_eq!(normalize_agent_source(source.to_owned()), expected);
    }
}
