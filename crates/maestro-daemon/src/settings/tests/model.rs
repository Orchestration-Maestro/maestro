//! Typed workspace settings behavior.

use super::super::model::{ShellModeConfig, TerminalConfig};
use serde_json::json;

#[test]
fn pane_shell_options_default_and_decode() {
    let defaults: TerminalConfig = serde_json::from_value(json!({})).unwrap();
    assert!(defaults.default_shell.is_empty());
    assert_eq!(defaults.shell_mode, ShellModeConfig::Auto);
    let supplied: TerminalConfig = serde_json::from_value(json!({
        "paneShell": "nu", "paneShellMode": "non_login"
    }))
    .unwrap();
    assert_eq!(supplied.default_shell, "nu");
    assert_eq!(supplied.shell_mode, ShellModeConfig::NonLogin);
    let shell_only: TerminalConfig = serde_json::from_value(json!({"paneShell": "nu"})).unwrap();
    assert_eq!(shell_only.shell_mode, ShellModeConfig::Auto);
    let mode_only: TerminalConfig =
        serde_json::from_value(json!({"paneShellMode": "login"})).unwrap();
    assert!(mode_only.default_shell.is_empty());
    assert_eq!(mode_only.shell_mode, ShellModeConfig::Login);
}

#[test]
fn new_pane_directory_preserves_policy_and_path() {
    use super::super::model::NewTerminalCwdConfig;
    let defaults: TerminalConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(defaults.new_cwd, NewTerminalCwdConfig::Follow);
    for (text, expected) in [
        ("home", NewTerminalCwdConfig::Home),
        ("current", NewTerminalCwdConfig::Current),
        ("follow", NewTerminalCwdConfig::Follow),
        ("  ", NewTerminalCwdConfig::Follow),
        (" home ", NewTerminalCwdConfig::Home),
        (
            "~/Projects",
            NewTerminalCwdConfig::Path("~/Projects".into()),
        ),
        (" ./space ", NewTerminalCwdConfig::Path(" ./space ".into())),
    ] {
        let config: TerminalConfig = serde_json::from_value(json!({"newPaneCwd": text})).unwrap();
        assert_eq!(config.new_cwd, expected);
    }
}

#[test]
fn restore_agents_can_be_disabled() {
    use super::super::model::SessionConfig;
    let defaults: SessionConfig = serde_json::from_value(json!({})).unwrap();
    assert!(defaults.resume_agents_on_restore);
    let config: SessionConfig =
        serde_json::from_value(json!({"resumeAgentsOnRestore": false})).unwrap();
    assert!(!config.resume_agents_on_restore);
}

#[test]
fn update_options_default_and_decode() {
    use super::super::model::{UpdateChannelConfig, UpdateConfig};
    let defaults: UpdateConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(defaults.channel, UpdateChannelConfig::Stable);
    assert!(defaults.version_check);
    assert!(defaults.manifest_check);
    assert_eq!(defaults.channel.as_str(), "stable");
    let config: UpdateConfig = serde_json::from_value(
        json!({"channel": "preview", "versionCheck": false, "detectionCatalogCheck": false}),
    )
    .unwrap();
    assert_eq!(config.channel, UpdateChannelConfig::Preview);
    assert_eq!(config.channel.as_str(), "preview");
    assert!(!config.version_check);
    assert!(!config.manifest_check);
}

#[test]
fn update_channel_tracks_host_and_build() {
    use super::super::model::{UpdateChannelConfig, default_update_channel_for_build};
    assert_eq!(
        default_update_channel_for_build("windows", true),
        UpdateChannelConfig::Preview
    );
    assert_eq!(
        default_update_channel_for_build("windows", false),
        UpdateChannelConfig::Stable
    );
    assert_eq!(
        default_update_channel_for_build("linux", true),
        UpdateChannelConfig::Stable
    );
    assert_eq!(
        default_update_channel_for_build("macos", true),
        UpdateChannelConfig::Stable
    );
}

#[test]
fn missing_channel_keeps_build_default() {
    use super::super::model::UpdateConfig;
    let defaults = UpdateConfig::default();
    let empty: UpdateConfig = serde_json::from_value(json!({})).unwrap();
    let config: UpdateConfig = serde_json::from_value(json!({"versionCheck": false})).unwrap();
    assert_eq!(empty.channel, defaults.channel);
    assert_eq!(config.channel, defaults.channel);
    assert!(!config.version_check);
}

#[test]
fn headless_dimensions_validate_as_a_pair() {
    use super::super::model::ServerConfig;
    let defaults: ServerConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(defaults.headless_size(), (120, 40));
    assert_eq!(defaults.invalid_headless_size_diagnostic(), None);
    let config: ServerConfig =
        serde_json::from_value(json!({"headlessCols":160,"headlessRows":50})).unwrap();
    assert_eq!(config.headless_size(), (160, 50));
    for (cols, rows) in [(0, 50), (160, 0), (0, 0)] {
        let config: ServerConfig =
            serde_json::from_value(json!({"headlessCols":cols,"headlessRows":rows})).unwrap();
        assert_eq!(config.headless_size(), (120, 40));
        assert_eq!(
            config.invalid_headless_size_diagnostic(),
            Some(format!(
                "daemon.headlessCols and daemon.headlessRows must be greater than zero (got {cols}x{rows})"
            ))
        );
    }
}

#[test]
fn worktree_directory_keeps_authored_spelling() {
    use super::super::model::WorktreesConfig;
    let defaults: WorktreesConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(defaults.directory, "~/.maestro/worktrees");
    let config: WorktreesConfig =
        serde_json::from_value(json!({"worktreesDirectory":" ./trees/../linked "})).unwrap();
    assert_eq!(config.directory, " ./trees/../linked ");
}

#[test]
fn scrollback_defaults_to_ten_million_bytes() {
    use super::super::model::AdvancedConfig;
    let defaults: AdvancedConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(defaults.scrollback_limit_bytes, 10_000_000);
}

#[test]
fn scrollback_byte_limit_decodes() {
    use super::super::model::AdvancedConfig;
    let config: AdvancedConfig =
        serde_json::from_value(json!({"scrollbackLimitBytes": 1234})).unwrap();
    assert_eq!(config.scrollback_limit_bytes, 1234);
    let legacy: AdvancedConfig = serde_json::from_value(json!({"scrollback_lines": 7})).unwrap();
    assert_eq!(legacy.scrollback_limit_bytes, 10_000_000);
}

#[test]
fn ime_cursor_shape_maps_all_six_codes() {
    use super::super::model::ImeCursorShape;
    assert_eq!(ImeCursorShape::default(), ImeCursorShape::SteadyBlock);
    for (name, expected) in [
        ("block", 1),
        ("steady_block", 2),
        ("underline", 3),
        ("steady_underline", 4),
        ("bar", 5),
        ("steady_bar", 6),
    ] {
        let shape: ImeCursorShape = serde_json::from_value(json!(name)).unwrap();
        assert_eq!(shape.to_decscusr(), expected);
    }
}

#[test]
fn experimental_options_decode_together() {
    use super::super::model::{ExperimentalConfig, ImeCursorShape};
    let defaults: ExperimentalConfig = serde_json::from_value(json!({})).unwrap();
    assert!(!defaults.allow_nested.0);
    assert!(!defaults.kitty_graphics.0);
    assert!(!defaults.pane_history.0);
    assert!(!defaults.reveal_hidden_cursor_for_cjk_ime.0);
    assert!(!defaults.switch_ascii_input_source_in_prefix.0);
    assert!(defaults.cjk_ime_agents.is_empty());
    assert_eq!(defaults.cjk_ime_cursor_shape, ImeCursorShape::SteadyBlock);
    let config: ExperimentalConfig = serde_json::from_value(json!({
        "allowNested":true,"kittyGraphics":true,"paneHistory":true,
        "revealHiddenCursorForCjkIme":true,"switchAsciiInputSourceInPrefix":true,
        "cjkImeAgents":["maestro","codex"],"cjkImeCursorShape":"bar"
    }))
    .unwrap();
    assert!(config.allow_nested.0);
    assert!(config.kitty_graphics.0);
    assert!(config.pane_history.0);
    assert!(config.reveal_hidden_cursor_for_cjk_ime.0);
    assert!(config.switch_ascii_input_source_in_prefix.0);
    assert_eq!(config.cjk_ime_agents, ["maestro", "codex"]);
    assert_eq!(config.cjk_ime_cursor_shape, ImeCursorShape::Bar);
}

#[test]
fn pane_history_requires_opt_in() {
    use super::super::model::ExperimentalConfig;
    let defaults: ExperimentalConfig = serde_json::from_value(json!({})).unwrap();
    assert!(!defaults.pane_history.0);
    let config: ExperimentalConfig = serde_json::from_value(json!({"paneHistory":true})).unwrap();
    assert!(config.pane_history.0);
    assert!(!config.kitty_graphics.0);
}

#[test]
fn kitty_graphics_requires_opt_in() {
    use super::super::model::ExperimentalConfig;
    let defaults: ExperimentalConfig = serde_json::from_value(json!({})).unwrap();
    assert!(!defaults.kitty_graphics.0);
    let config: ExperimentalConfig = serde_json::from_value(json!({"kittyGraphics":true})).unwrap();
    assert!(config.kitty_graphics.0);
    assert!(!config.pane_history.0);
}

#[test]
fn ime_hidden_cursor_reveal_is_opt_in() {
    use super::super::model::ExperimentalConfig;
    let defaults: ExperimentalConfig = serde_json::from_value(json!({})).unwrap();
    assert!(!defaults.reveal_hidden_cursor_for_cjk_ime.0);
    let config: ExperimentalConfig =
        serde_json::from_value(json!({"revealHiddenCursorForCjkIme":true})).unwrap();
    assert!(config.reveal_hidden_cursor_for_cjk_ime.0);
    assert!(!config.switch_ascii_input_source_in_prefix.0);
}

#[test]
fn prefix_ascii_input_switch_is_opt_in() {
    use super::super::model::ExperimentalConfig;
    let defaults: ExperimentalConfig = serde_json::from_value(json!({})).unwrap();
    assert!(!defaults.switch_ascii_input_source_in_prefix.0);
    let config: ExperimentalConfig =
        serde_json::from_value(json!({"switchAsciiInputSourceInPrefix":true})).unwrap();
    assert!(config.switch_ascii_input_source_in_prefix.0);
    assert!(!config.reveal_hidden_cursor_for_cjk_ime.0);
}

#[test]
fn ime_agent_filter_keeps_supplied_names() {
    use super::super::model::ExperimentalConfig;
    let defaults: ExperimentalConfig = serde_json::from_value(json!({})).unwrap();
    assert!(defaults.cjk_ime_agents.is_empty());
    let config: ExperimentalConfig =
        serde_json::from_value(json!({"cjkImeAgents":["codex","custom-agent","MAESTRO"]})).unwrap();
    assert_eq!(config.cjk_ime_agents, ["codex", "custom-agent", "MAESTRO"]);
}

#[test]
fn toast_defaults_include_clipboard_policy() {
    use super::super::model::{
        ToastClipboardPosition, ToastConfig, ToastDelivery, ToastMaestroPosition,
    };
    let defaults: ToastConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(defaults.delivery, ToastDelivery::Off);
    assert_eq!(defaults.delay_seconds, 1);
    assert_eq!(
        defaults.workspace.position,
        ToastMaestroPosition::BottomRight
    );
    assert!(defaults.clipboard.enabled);
    assert_eq!(
        defaults.clipboard.position,
        ToastClipboardPosition::BottomCenter
    );
}

#[test]
fn toast_delivery_and_positions_decode() {
    use super::super::model::{
        ToastClipboardPosition, ToastConfig, ToastDelivery, ToastMaestroPosition,
    };
    let config: ToastConfig = serde_json::from_value(json!({
        "delivery":"terminal", "delaySeconds":2,
        "workspace":{"position":"top-left"}, "clipboard":{"enabled":false,"position":"top-center"}
    }))
    .unwrap();
    assert_eq!(config.delivery, ToastDelivery::Terminal);
    assert_eq!(config.delay_seconds, 2);
    assert_eq!(config.workspace.position, ToastMaestroPosition::TopLeft);
    assert!(!config.clipboard.enabled);
    assert_eq!(config.clipboard.position, ToastClipboardPosition::TopCenter);
    for name in [
        "top-left",
        "top-center",
        "top-right",
        "bottom-left",
        "bottom-center",
        "bottom-right",
    ] {
        let position: ToastClipboardPosition = serde_json::from_value(json!(name)).unwrap();
        assert_eq!(serde_json::to_value(position).unwrap(), json!(name));
    }
}

#[test]
fn toast_system_delivery_decodes() {
    use super::super::model::{ToastConfig, ToastDelivery};
    let config: ToastConfig = serde_json::from_value(json!({"delivery":"system"})).unwrap();
    assert_eq!(config.delivery, ToastDelivery::System);
    let config: ToastConfig = serde_json::from_value(json!({"delivery":"workspace"})).unwrap();
    assert_eq!(config.delivery, ToastDelivery::Workspace);
}

#[test]
fn toast_delay_checks_both_boundaries() {
    use super::super::model::ToastConfig;
    for seconds in [0, 3600] {
        let config: ToastConfig = serde_json::from_value(json!({"delaySeconds":seconds})).unwrap();
        assert_eq!(config.delay_seconds, seconds);
    }
    let error = serde_json::from_value::<ToastConfig>(json!({"delaySeconds":3601})).unwrap_err();
    assert_eq!(
        error.to_string(),
        "multiplexer.toast.delaySeconds must be between 0 and 3600"
    );
    let config: ToastConfig = serde_json::from_value(json!({"delaySeconds":null})).unwrap();
    assert_eq!(config.delay_seconds, 1);
    for wrong in [
        json!(-1),
        json!(1.5),
        json!("2"),
        json!(true),
        json!({}),
        json!([]),
    ] {
        assert!(serde_json::from_value::<ToastConfig>(json!({"delaySeconds":wrong})).is_err());
    }
}

#[test]
fn right_click_modifier_aliases_combine() {
    use super::super::model::RightClickPassthroughModifierConfig;
    use maestro_protocol::frames::KeyModifiers;
    for name in ["", " ", "off", "OFF", "none", "disabled"] {
        let config: RightClickPassthroughModifierConfig =
            serde_json::from_value(json!(name)).unwrap();
        assert_eq!(config.modifiers(), None);
    }
    for (name, bits) in [
        ("ctrl", KeyModifiers::CONTROL),
        ("control", KeyModifiers::CONTROL),
        ("alt", KeyModifiers::ALT),
        ("option", KeyModifiers::ALT),
        ("cmd", KeyModifiers::SUPER),
        ("command", KeyModifiers::SUPER),
        ("super", KeyModifiers::SUPER),
        ("meta", KeyModifiers::META),
        ("hyper", KeyModifiers::HYPER),
        (" Super + ALT ", KeyModifiers::SUPER | KeyModifiers::ALT),
    ] {
        let config: RightClickPassthroughModifierConfig =
            serde_json::from_value(json!(name)).unwrap();
        assert_eq!(config.modifiers(), Some(bits));
    }
}

#[test]
fn right_click_modifier_rejects_invalid_parts() {
    use super::super::model::RightClickPassthroughModifierConfig;
    for text in [
        "shift",
        "shift+ctrl",
        "ctrl+",
        "+ctrl",
        "ctrl++alt",
        "mystery",
    ] {
        let error =
            serde_json::from_value::<RightClickPassthroughModifierConfig>(json!(text)).unwrap_err();
        assert_eq!(
            error.to_string(),
            "multiplexer.rightClickPassthroughModifier must be empty, off, none, disabled, ctrl/control, alt/option, cmd/command/super, meta, hyper, or a + separated combination without shift"
        );
    }
}
