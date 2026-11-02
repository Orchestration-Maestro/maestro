//! Notification sound preference behavior.

use super::super::sound::{AgentSoundSetting, SoundConfig};
use maestro_detection::agents::Agent;
use serde_json::json;

#[test]
fn sound_options_and_agent_overrides_decode() {
    let defaults: SoundConfig = serde_json::from_value(json!({})).unwrap();
    assert!(defaults.enabled);
    assert_eq!(defaults.path, None);
    assert_eq!(defaults.done_path, None);
    assert_eq!(defaults.request_path, None);
    assert!(defaults.allows(None));
    for agent in Agent::ALL {
        assert_eq!(defaults.allows(Some(agent)), agent != Agent::Droid);
    }
    let config: SoundConfig = serde_json::from_value(json!({
        "enabled": false, "path":"all.mp3", "donePath":"done.mp3", "requestPath":"request.mp3"
    }))
    .unwrap();
    assert!(!config.enabled);
    assert_eq!(config.path.as_deref(), Some("all.mp3"));
    assert_eq!(config.done_path.as_deref(), Some("done.mp3"));
    assert_eq!(config.request_path.as_deref(), Some("request.mp3"));
    assert!(!config.allows(None));
    for agent in Agent::ALL {
        assert!(!config.allows(Some(agent)));
    }
    assert_agent_overrides();
}

/// Every configurable agent retains its own setting under both global states.
fn assert_agent_overrides() {
    for (key, agent) in [
        ("pi", Agent::Pi),
        ("claude", Agent::Claude),
        ("codex", Agent::Codex),
        ("gemini", Agent::Gemini),
        ("cursor", Agent::Cursor),
        ("devin", Agent::Devin),
        ("agy", Agent::Antigravity),
        ("cline", Agent::Cline),
        ("openCode", Agent::OpenCode),
        ("githubCopilot", Agent::GithubCopilot),
        ("kimi", Agent::Kimi),
        ("kiro", Agent::Kiro),
        ("droid", Agent::Droid),
        ("amp", Agent::Amp),
        ("grok", Agent::Grok),
        ("hermes", Agent::Hermes),
        ("kilo", Agent::Kilo),
        ("qodercli", Agent::Qodercli),
        ("qwen", Agent::Qwen),
        ("maki", Agent::Maki),
    ] {
        for (setting, expected) in [
            ("default", AgentSoundSetting::Default),
            ("on", AgentSoundSetting::On),
            ("off", AgentSoundSetting::Off),
        ] {
            let config: SoundConfig =
                serde_json::from_value(json!({"agents":{key:setting}})).unwrap();
            assert_eq!(
                config.agents.for_agent(Some(agent)),
                expected,
                "{key}/{setting}"
            );
            assert_eq!(config.allows(Some(agent)), setting != "off");
            let disabled: SoundConfig =
                serde_json::from_value(json!({"enabled":false,"agents":{key:setting}})).unwrap();
            assert!(!disabled.allows(Some(agent)));
        }
    }
}

#[test]
fn sound_done_path_uses_specific_then_shared() {
    use maestro_protocol::schema::NotificationShowSound;
    let root = std::path::Path::new("agent-root");
    let config: SoundConfig = serde_json::from_value(
        json!({"path":"all.mp3","donePath":"done.mp3","requestPath":"request.mp3"}),
    )
    .unwrap();
    assert_eq!(
        config.path_for(NotificationShowSound::Done, root),
        Some(root.join("done.mp3"))
    );
    let config: SoundConfig = serde_json::from_value(json!({"path":"all.mp3"})).unwrap();
    assert_eq!(
        config.path_for(NotificationShowSound::Done, root),
        Some(root.join("all.mp3"))
    );
    assert_eq!(
        SoundConfig::default().path_for(NotificationShowSound::Done, root),
        None
    );
    let absolute = std::env::current_dir().unwrap().join("absolute.mp3");
    let config: SoundConfig = serde_json::from_value(json!({"donePath":absolute})).unwrap();
    assert_eq!(
        config.path_for(NotificationShowSound::Done, root),
        Some(absolute)
    );
}

#[test]
fn sound_request_path_uses_specific_then_shared() {
    use maestro_protocol::schema::NotificationShowSound;
    let root = std::path::Path::new("agent-root");
    let config: SoundConfig = serde_json::from_value(
        json!({"path":"all.mp3","donePath":"done.mp3","requestPath":"request.mp3"}),
    )
    .unwrap();
    assert_eq!(
        config.path_for(NotificationShowSound::Request, root),
        Some(root.join("request.mp3"))
    );
    let config: SoundConfig =
        serde_json::from_value(json!({"path":"all.mp3","donePath":"done.mp3"})).unwrap();
    assert_eq!(
        config.path_for(NotificationShowSound::Request, root),
        Some(root.join("all.mp3"))
    );
    let config: SoundConfig = serde_json::from_value(json!({"donePath":"done.mp3"})).unwrap();
    assert_eq!(config.path_for(NotificationShowSound::Request, root), None);
    assert_eq!(
        SoundConfig::default().path_for(NotificationShowSound::Request, root),
        None
    );
}

#[test]
fn sound_none_never_selects_a_configured_or_fallback_path() {
    use maestro_protocol::schema::NotificationShowSound;
    let root = std::path::Path::new("agent-root");
    let config: SoundConfig = serde_json::from_value(
        json!({"path":"all.mp3","donePath":"done.mp3","requestPath":"request.mp3"}),
    )
    .unwrap();
    assert_eq!(config.path_for(NotificationShowSound::None, root), None);
    assert_eq!(
        SoundConfig::default().path_for(NotificationShowSound::None, root),
        None
    );
}

#[test]
fn sound_unsupported_extension_reports_fallback() {
    let config: SoundConfig = serde_json::from_value(json!({"path":"missing.wav"})).unwrap();
    let root = std::path::Path::new("agent-root");
    assert_eq!(
        config.diagnostics(root),
        [format!(
            "unsupported sound file format: multiplexer.sound.path = missing.wav resolves to {}; expected an mp3 file; using default sound",
            root.join("missing.wav").display()
        )]
    );
}

#[test]
fn sound_file_diagnostics_distinguish_missing_and_directory() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    std::fs::create_dir(root.join("directory.mp3")).unwrap();
    std::fs::write(root.join("valid.MP3"), b"controlled sound bytes").unwrap();
    let config: SoundConfig = serde_json::from_value(
        json!({"path":"missing.mp3","donePath":"directory.mp3","requestPath":"valid.MP3"}),
    )
    .unwrap();
    assert_eq!(
        config.diagnostics(root),
        [
            format!(
                "missing sound file: multiplexer.sound.path = missing.mp3 resolves to {}; using default sound",
                root.join("missing.mp3").display()
            ),
            format!(
                "invalid sound file: multiplexer.sound.donePath = directory.mp3 resolves to {}; using default sound",
                root.join("directory.mp3").display()
            ),
        ]
    );
}
