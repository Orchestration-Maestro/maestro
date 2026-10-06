mod support;
use maestro_settings::{SettingsManager, SettingsScope};
use serde_json::{Value, json};
use std::sync::Arc;
use support::*;

#[test]
fn fresh_enabled_models_survive_thinking_save() {
    let (m, s, q) = seeded(json!({"enabledModels":["old"]}), json!({}));
    put(
        s.as_ref(),
        SettingsScope::Global,
        r#"{"enabledModels":["fresh"]}"#,
    );
    m.set_default_thinking_level("high".into());
    q.drive();
    block_on(m.flush());
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global),
        json!({"enabledModels":["fresh"],"defaultThinkingLevel":"high"})
    );
    assert_eq!(m.get_default_thinking_level().as_deref(), Some("high"));
}

#[test]
fn fresh_custom_keys_survive_theme_save() {
    let (m, s, q) = empty();
    put(
        s.as_ref(),
        SettingsScope::Global,
        r#"{"custom":{"new":true},"theme":"old"}"#,
    );
    m.set_theme("dark".into());
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global),
        json!({"custom":{"new":true},"theme":"dark"})
    );
    assert_eq!(m.get_global_settings(), json!({"theme":"dark"}));
}

#[test]
fn thinking_save_wins_same_key_edit() {
    let (m, s, q) = empty();
    put(
        s.as_ref(),
        SettingsScope::Global,
        r#"{"defaultThinkingLevel":"low"}"#,
    );
    m.set_default_thinking_level("high".into());
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global)["defaultThinkingLevel"],
        "high"
    );
}

#[test]
fn global_reload_observes_external_values() {
    let (m, s, _) = empty();
    put(s.as_ref(), SettingsScope::Global, r#"{"theme":"external"}"#);
    block_on(m.reload());
    assert_eq!(m.get_theme().as_deref(), Some("external"));
    let (m, s, _) = seeded(
        json!({"theme":"dark","extensions":["/before.ts"]}),
        json!({}),
    );
    put(
        s.as_ref(),
        SettingsScope::Global,
        r#"{"theme":"light","extensions":["/after.ts"],"defaultModel":"claude-sonnet"}"#,
    );
    block_on(m.reload());
    assert_eq!(m.get_theme().as_deref(), Some("light"));
    assert_eq!(m.get_extension_paths(), vec!["/after.ts"]);
    assert_eq!(m.get_default_model().as_deref(), Some("claude-sonnet"));
}

#[test]
fn invalid_reload_retains_accepted_global() {
    let (m, s, _) = seeded(json!({"theme":"accepted"}), json!({}));
    put(s.as_ref(), SettingsScope::Global, "{");
    block_on(m.reload());
    assert_eq!(m.get_theme().as_deref(), Some("accepted"));
}

#[test]
fn load_errors_drain_in_scope_order() {
    let storage = Arc::new(maestro_settings::InMemorySettingsStorage::new());
    put(storage.as_ref(), SettingsScope::Global, "{");
    put(storage.as_ref(), SettingsScope::Project, " ");
    let m = SettingsManager::from_storage(storage, Scheduler::default().spawn());
    let errors = m.drain_errors();
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].scope, SettingsScope::Global);
    assert_eq!(errors[1].scope, SettingsScope::Project);
    assert!(m.drain_errors().is_empty());
}

#[test]
fn global_project_and_override_spreads_are_one_level() {
    let (m, s, q) = seeded(
        json!({"retry":{"enabled":true,"provider":{"timeoutMs":1,"maxRetries":2}},"arr":[1],"other":true}),
        json!({"retry":{"provider":{"maxRetries":7}},"arr":[2]}),
    );
    assert_eq!(m.get_global_settings()["retry"]["provider"]["timeoutMs"], 1);
    m.apply_overrides(json!({"retry":{"provider":{"timeoutMs":9}},"arr":null,"theme":"override"}));
    assert_eq!(m.get_theme().as_deref(), Some("override"));
    m.set_theme("stored".into());
    q.drive();
    assert_eq!(m.get_theme().as_deref(), Some("stored"));
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global)["retry"]["provider"],
        json!({"timeoutMs":1,"maxRetries":2})
    );
    assert_eq!(m.get_provider_retry_settings().timeout_ms, None);
    assert_eq!(m.get_provider_retry_settings().max_retries, Some(7.0));
}

#[test]
fn queue_mode_conversion_checks_property_presence() {
    let (m, _) = memory(json!({"queueMode":"all"}));
    assert_eq!(m.get_global_settings(), json!({"steeringMode":"all"}));
    let (m, _) = memory(json!({"queueMode":"all","steeringMode":null}));
    assert_eq!(
        m.get_global_settings(),
        json!({"queueMode":"all","steeringMode":null})
    );
}

#[test]
fn websocket_conversion_requires_boolean_and_absent_transport() {
    for (old, new) in [(true, "websocket"), (false, "sse")] {
        let (m, _) = memory(json!({"websockets":old}));
        assert_eq!(m.get_global_settings(), json!({"transport":new}));
    }
    for input in [
        json!({"websockets":true,"transport":null}),
        json!({"websockets":"true"}),
    ] {
        let (m, _) = memory(input.clone());
        assert_eq!(m.get_global_settings(), input);
    }
}

#[test]
fn skills_conversion_preserves_target_and_array_rules() {
    let (m, _) =
        memory(json!({"skills":{"enableSkillCommands":false,"customDirectories":["x",3]}}));
    assert_eq!(
        m.get_global_settings(),
        json!({"enableSkillCommands":false,"skills":["x",3]})
    );
    let (m, _) = memory(
        json!({"enableSkillCommands":null,"skills":{"enableSkillCommands":true,"customDirectories":[]}}),
    );
    assert_eq!(m.get_global_settings(), json!({"enableSkillCommands":null}));
    for skills in [Value::Null, json!([]), json!(true), json!("x")] {
        let input = json!({"skills":skills});
        let (m, _) = memory(input.clone());
        assert_eq!(m.get_global_settings(), input);
    }
}

#[test]
fn retry_conversion_handles_null_targets_and_provider_siblings() {
    for provider in [
        json!({"maxRetryDelayMs":null,"timeoutMs":2}),
        json!(["sibling"]),
        json!(false),
    ] {
        let (m, _) = memory(json!({"retry":{"maxDelayMs":7,"provider":provider}}));
        let raw = m.get_global_settings();
        assert_eq!(raw["retry"]["provider"]["maxRetryDelayMs"], 7);
        assert!(raw["retry"].get("maxDelayMs").is_none());
        if provider.is_array() {
            assert_eq!(raw["retry"]["provider"]["0"], "sibling");
        }
        if provider.is_object() {
            assert_eq!(raw["retry"]["provider"]["timeoutMs"], 2);
        }
    }
    let (m, _) = memory(json!({"retry":{"maxDelayMs":"bad","provider":{"maxRetryDelayMs":5}}}));
    assert_eq!(
        m.get_global_settings(),
        json!({"retry":{"provider":{"maxRetryDelayMs":5}}})
    );
}

#[test]
fn model_and_delivery_accessors_keep_defaults() {
    let (m, q) = memory(json!({}));
    assert_eq!(m.get_last_changelog_version(), None);
    assert_eq!(m.get_default_provider(), None);
    assert_eq!(m.get_default_model(), None);
    assert_eq!(m.get_default_thinking_level(), None);
    assert_eq!(m.get_theme(), None);
    assert_eq!(m.get_steering_mode(), "one-at-a-time");
    assert_eq!(m.get_follow_up_mode(), "one-at-a-time");
    assert_eq!(m.get_transport(), "auto");
    m.set_last_changelog_version("v".into());
    m.set_default_provider("custom".into());
    m.set_default_model("one".into());
    m.set_default_model_and_provider("other".into(), "two".into());
    m.set_steering_mode("".into());
    m.set_follow_up_mode("future".into());
    m.set_transport("".into());
    assert_eq!(m.get_last_changelog_version().as_deref(), Some("v"));
    assert_eq!(m.get_default_provider().as_deref(), Some("other"));
    assert_eq!(m.get_default_model().as_deref(), Some("two"));
    assert_eq!(m.get_steering_mode(), "one-at-a-time");
    assert_eq!(m.get_follow_up_mode(), "future");
    assert_eq!(m.get_transport(), "");
    q.drive();
}

#[test]
fn context_and_retry_accessors_keep_individual_defaults() {
    let (m, s, q) = empty();
    assert!(m.get_compaction_enabled());
    assert_eq!(m.get_compaction_reserve_tokens(), 16384.0);
    assert_eq!(m.get_compaction_keep_recent_tokens(), 20000.0);
    let c = m.get_compaction_settings();
    assert_eq!(c.enabled, Some(true));
    assert_eq!(c.reserve_tokens, Some(16384.0));
    assert_eq!(c.keep_recent_tokens, Some(20000.0));
    let b = m.get_branch_summary_settings();
    assert_eq!(b.reserve_tokens, Some(16384.0));
    assert_eq!(b.skip_prompt, Some(false));
    assert!(!m.get_branch_summary_skip_prompt());
    let r = m.get_retry_settings();
    assert_eq!(r.enabled, Some(true));
    assert_eq!(r.max_retries, Some(3.0));
    assert_eq!(r.base_delay_ms, Some(2000.0));
    assert!(r.provider.is_none());
    assert!(m.get_retry_enabled());
    assert_eq!(
        m.get_provider_retry_settings().max_retry_delay_ms,
        Some(60000.0)
    );
    assert_eq!(m.get_global_settings(), json!({}));
    put(
        s.as_ref(),
        SettingsScope::Global,
        r#"{"compaction":{"reserveTokens":11},"retry":{"maxRetries":9}}"#,
    );
    m.set_compaction_enabled(false).unwrap();
    m.set_retry_enabled(false).unwrap();
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global),
        json!({"compaction":{"enabled":false,"reserveTokens":11},"retry":{"enabled":false,"maxRetries":9}})
    );
}

#[test]
fn command_and_presentation_accessors_keep_defaults() {
    let (m, q) = memory(json!({}));
    assert_eq!(m.get_shell_path(), None);
    assert_eq!(m.get_shell_command_prefix(), None);
    assert_eq!(m.get_npm_command(), None);
    assert!(!m.get_quiet_startup());
    assert!(!m.get_hide_thinking_block());
    assert!(!m.get_collapse_changelog());
    assert!(m.get_enable_install_telemetry());
    assert_eq!(m.get_double_escape_action(), "tree");
    assert_eq!(m.get_code_block_indent(), "  ");
    assert_eq!(m.get_global_settings(), json!({}));
    m.set_shell_path(Some("".into()));
    m.set_shell_command_prefix(Some(" x ".into()));
    m.set_npm_command(Some(vec!["mise".into(), "npm".into()]));
    m.set_quiet_startup(true);
    m.set_hide_thinking_block(true);
    m.set_collapse_changelog(true);
    m.set_enable_install_telemetry(false);
    m.set_double_escape_action("".into());
    assert_eq!(m.get_shell_path(), Some("".into()));
    assert_eq!(m.get_shell_command_prefix(), Some(" x ".into()));
    assert_eq!(m.get_npm_command(), Some(vec!["mise".into(), "npm".into()]));
    assert!(m.get_quiet_startup());
    assert!(m.get_hide_thinking_block());
    assert!(m.get_collapse_changelog());
    assert!(!m.get_enable_install_telemetry());
    assert_eq!(m.get_double_escape_action(), "");
    q.drive();
}

#[test]
fn resource_lists_replace_in_both_scopes() {
    let (m, s, q) = empty();
    assert!(m.get_packages().is_empty());
    assert!(m.get_extension_paths().is_empty());
    assert!(m.get_skill_paths().is_empty());
    assert!(m.get_prompt_template_paths().is_empty());
    assert!(m.get_theme_paths().is_empty());
    assert!(m.get_enable_skill_commands());
    m.set_packages(vec![maestro_settings::PackageSource::String(
        "npm:a".into(),
    )]);
    m.set_project_packages(vec![maestro_settings::PackageSource::String(
        "git:b".into(),
    )]);
    m.set_extension_paths(vec!["g1".into(), "g2".into()]);
    m.set_project_extension_paths(vec!["p2".into(), "p1".into()]);
    m.set_skill_paths(vec!["gs".into()]);
    m.set_project_skill_paths(vec!["ps".into()]);
    m.set_prompt_template_paths(vec!["gp".into()]);
    m.set_project_prompt_template_paths(vec!["pp".into()]);
    m.set_theme_paths(vec!["gt".into()]);
    m.set_project_theme_paths(vec!["pt".into()]);
    m.set_enable_skill_commands(false);
    q.drive();
    assert_eq!(m.get_extension_paths(), vec!["p2", "p1"]);
    assert_eq!(m.get_skill_paths(), vec!["ps"]);
    assert_eq!(m.get_prompt_template_paths(), vec!["pp"]);
    assert_eq!(m.get_theme_paths(), vec!["pt"]);
    assert!(!m.get_enable_skill_commands());
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global)["packages"],
        json!(["npm:a"])
    );
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Project)["packages"],
        json!(["git:b"])
    );
    m.set_project_extension_paths(vec![]);
    q.drive();
    assert!(m.get_extension_paths().is_empty());
}

#[test]
fn terminal_and_image_accessors_keep_defaults() {
    let (m, s, q) = empty();
    assert!(m.get_show_images());
    assert!(!m.get_show_terminal_progress());
    assert!(m.get_image_auto_resize());
    assert!(!m.get_block_images());
    assert_eq!(m.get_global_settings(), json!({}));
    put(
        s.as_ref(),
        SettingsScope::Global,
        r#"{"terminal":{"other":1},"images":{"other":2}}"#,
    );
    m.set_show_images(false).unwrap();
    m.set_show_terminal_progress(true).unwrap();
    m.set_image_auto_resize(false).unwrap();
    m.set_block_images(true).unwrap();
    q.drive();
    assert!(!m.get_show_images());
    assert!(m.get_show_terminal_progress());
    assert!(!m.get_image_auto_resize());
    assert!(m.get_block_images());
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global),
        json!({"terminal":{"showImages":false,"showTerminalProgress":true,"other":1},"images":{"autoResize":false,"blockImages":true,"other":2}})
    );
}

#[test]
fn numeric_getters_and_setters_keep_distinct_rules() {
    let (m, q) = memory(
        json!({"terminal":{"imageWidthCells":0.2},"editorPaddingX":1.7,"autocompleteMaxVisible":2.2}),
    );
    assert_eq!(m.get_image_width_cells(), 1.0);
    assert_eq!(m.get_editor_padding_x(), 1.7);
    assert_eq!(m.get_autocomplete_max_visible(), 2.2);
    m.set_image_width_cells(62.9).unwrap();
    m.set_editor_padding_x(9.0);
    m.set_autocomplete_max_visible(-3.0);
    assert_eq!(m.get_image_width_cells(), 62.0);
    assert_eq!(m.get_editor_padding_x(), 3.0);
    assert_eq!(m.get_autocomplete_max_visible(), 3.0);
    q.drive();
    for v in [Value::Null, json!("3"), json!(false)] {
        let (m, _) = memory(json!({"terminal":{"imageWidthCells":v}}));
        assert_eq!(m.get_image_width_cells(), 60.0);
    }
}

/// typed numeric reads keep nonfinite values; standard JSON snapshots use null.
#[test]
fn nonfinite_setters_keep_getters_and_json_outcomes() {
    let (m, _) = memory(json!({}));
    m.set_editor_padding_x(f64::NAN);
    m.apply_overrides(json!({"editorPaddingX":2}));
    assert_eq!(m.get_editor_padding_x(), 2.0);
    m.set_theme("reset".into());
    assert!(m.get_editor_padding_x().is_nan());
    for input in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.0] {
        let (m, s, q) = empty();
        m.set_editor_padding_x(input);
        m.set_autocomplete_max_visible(input);
        m.set_image_width_cells(input).unwrap();
        q.drive();
        if input == 0.0 {
            assert!(m.get_editor_padding_x().is_sign_positive());
        }
        if input.is_nan() {
            assert!(m.get_editor_padding_x().is_nan());
            assert!(m.get_autocomplete_max_visible().is_nan());
            assert_eq!(m.get_global_settings()["editorPaddingX"], Value::Null);
            assert_eq!(
                disk(s.as_ref(), SettingsScope::Global)["editorPaddingX"],
                Value::Null
            );
        } else {
            assert_eq!(
                m.get_editor_padding_x(),
                if input > 0.0 { 3.0 } else { 0.0 }
            );
            assert_eq!(
                m.get_autocomplete_max_visible(),
                if input > 0.0 { 20.0 } else { 3.0 }
            );
        }
        assert_eq!(
            m.get_image_width_cells(),
            if input.is_nan() || input == f64::INFINITY {
                60.0
            } else {
                1.0
            }
        );
        if input.is_nan() || input == f64::INFINITY {
            assert_eq!(
                m.get_global_settings()["terminal"]["imageWidthCells"],
                Value::Null
            );
            assert_eq!(
                disk(s.as_ref(), SettingsScope::Global)["terminal"]["imageWidthCells"],
                Value::Null
            );
        }
    }
}

#[test]
fn tree_filter_checks_only_its_known_values() {
    let (m, _) = memory(json!({}));
    for value in ["default", "no-tools", "user-only", "labeled-only", "all"] {
        m.set_tree_filter_mode(value.into());
        assert_eq!(m.get_tree_filter_mode(), value);
    }
    for value in [json!("unknown"), json!(""), Value::Null, json!(2)] {
        let (m, _) = memory(json!({"treeFilterMode":value}));
        assert_eq!(m.get_tree_filter_mode(), "default");
    }
    m.set_theme("open-future-name".into());
    assert_eq!(m.get_theme().as_deref(), Some("open-future-name"));
}

/// wrong types use the getter fallback; present clearOnShrink null is false.
#[test]
fn terminal_environment_fallbacks_distinguish_null() {
    if std::env::var_os("MAESTRO_SETTINGS_ENV_CHILD").is_none() {
        for env in [None, Some("0"), Some("true"), Some("1")] {
            let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
            cmd.args(["--exact", "terminal_environment_fallbacks_distinguish_null"])
                .env("MAESTRO_SETTINGS_ENV_CHILD", "1")
                .env_remove("MAESTRO_CLEAR_ON_SHRINK")
                .env_remove("MAESTRO_HARDWARE_CURSOR");
            if let Some(env) = env {
                cmd.env("MAESTRO_CLEAR_ON_SHRINK", env)
                    .env("MAESTRO_HARDWARE_CURSOR", env);
            }
            assert!(cmd.output().unwrap().status.success());
        }
        return;
    }
    let fallback = std::env::var("MAESTRO_CLEAR_ON_SHRINK").as_deref() == Ok("1");
    for value in [
        None,
        Some(Value::Null),
        Some(json!(false)),
        Some(json!(true)),
        Some(json!("wrong")),
    ] {
        let seed = if let Some(v) = value.clone() {
            json!({"terminal":{"clearOnShrink":v},"showHardwareCursor":v})
        } else {
            json!({})
        };
        let (m, q) = memory(seed);
        assert_eq!(
            m.get_clear_on_shrink(),
            match value.as_ref() {
                Some(Value::Null) => false,
                Some(Value::Bool(b)) => *b,
                _ => fallback,
            }
        );
        assert_eq!(
            m.get_show_hardware_cursor(),
            value.as_ref().and_then(Value::as_bool).unwrap_or(fallback)
        );
        m.set_clear_on_shrink(false).unwrap();
        m.set_show_hardware_cursor(false);
        assert!(!m.get_clear_on_shrink());
        assert!(!m.get_show_hardware_cursor());
        q.drive();
    }
}

#[test]
fn session_directory_keeps_literal_and_join_rules() {
    let (m, _) = memory(json!({}));
    assert_eq!(m.get_session_dir(), None);
    for text in [
        "",
        "~user",
        "relative/a/../b",
        " space ",
        "\u{feff}",
        "\u{85}",
    ] {
        let (m, _) = memory(json!({"sessionDir":text}));
        assert_eq!(m.get_session_dir().as_deref(), Some(text));
    }
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
    for (text, expected) in [
        ("~", home.clone()),
        ("~/a/../b", format!("{home}/b")),
        ("~/a/", format!("{home}/a/")),
        ("~//b", format!("{home}/b")),
    ] {
        let (m, _) = memory(json!({"sessionDir":text}));
        assert_eq!(m.get_session_dir(), Some(expected));
    }
}

/// raw and typed reads are detached owned copies.
#[test]
fn snapshots_and_owned_reads_are_detached() {
    let (m, q) = memory(
        json!({"npmCommand":["npm"],"enabledModels":["m"],"thinkingBudgets":{"low":3,"unknown":{"a":1}},"warnings":{"anthropicExtraUsage":false,"custom":[1]},"custom":{"nested":[1]}}),
    );
    let mut raw = m.get_global_settings();
    raw["custom"]["nested"][0] = json!(2);
    assert_eq!(m.get_global_settings()["custom"]["nested"], json!([1]));
    let mut npm = m.get_npm_command().unwrap();
    npm.push("extra".into());
    assert_eq!(m.get_npm_command(), Some(vec!["npm".into()]));
    let mut models = m.get_enabled_models().unwrap();
    models.clear();
    assert_eq!(m.get_enabled_models(), Some(vec!["m".into()]));
    m.set_enabled_models(Some(models));
    assert_eq!(m.get_enabled_models(), Some(vec![]));
    let mut budget = m.get_thinking_budgets().unwrap();
    budget.low = Some(99.0);
    assert_eq!(m.get_thinking_budgets().unwrap().low, Some(3.0));
    assert_eq!(budget.extra["unknown"], json!({"a":1}));
    let mut warnings = m.get_warnings();
    warnings.extra["custom"][0] = json!(9);
    assert_eq!(m.get_warnings().extra["custom"], json!([1]));
    m.set_warnings(warnings.clone());
    warnings.extra.clear();
    assert_eq!(m.get_warnings().extra["custom"], json!([9]));
    assert_eq!(m.get_warnings().anthropic_extra_usage, Some(false));
    q.drive();
}

#[test]
fn local_extensions_remain_separate_from_packages() {
    let (m, _) = memory(json!({"extensions":["/local/ext.ts","./relative/ext.ts"]}));
    assert!(m.get_packages().is_empty());
    assert_eq!(
        m.get_extension_paths(),
        vec!["/local/ext.ts", "./relative/ext.ts"]
    );
    let (m, _) = memory(json!({"extensions":["./local.ts"],"packages":["npm:a"]}));
    assert_eq!(m.get_extension_paths(), vec!["./local.ts"]);
    assert_eq!(
        m.get_packages(),
        vec![maestro_settings::PackageSource::String("npm:a".into())]
    );
}

#[test]
fn package_source_filters_keep_shape_and_order() {
    let input = json!({"packages":["npm:a",{"source":"git:b","extensions":[],"skills":["!x","y"],"prompts":["z"],"themes":["theme"],"custom":9}]});
    let (m, s, q) = seeded(input.clone(), json!({}));
    let sources = m.get_packages();
    assert_eq!(sources.len(), 2);
    assert_eq!(
        sources[0],
        maestro_settings::PackageSource::String("npm:a".into())
    );
    assert_eq!(
        sources[1],
        maestro_settings::PackageSource::Object {
            source: "git:b".into(),
            extensions: Some(vec![]),
            skills: Some(vec!["!x".into(), "y".into()]),
            prompts: Some(vec!["z".into()]),
            themes: Some(vec!["theme".into()]),
            extra: serde_json::from_value(json!({"custom":9})).unwrap(),
            property_order: [
                "source",
                "extensions",
                "skills",
                "prompts",
                "themes",
                "custom"
            ]
            .map(str::to_owned)
            .to_vec(),
        }
    );
    let (reference, _) = memory(
        json!({"packages":["npm:simple-pkg",{"source":"npm:shitty-extensions","extensions":["extensions/oracle.ts"],"skills":[]}]}),
    );
    assert_eq!(
        reference.get_packages(),
        vec![
            maestro_settings::PackageSource::String("npm:simple-pkg".into()),
            maestro_settings::PackageSource::Object {
                source: "npm:shitty-extensions".into(),
                extensions: Some(vec!["extensions/oracle.ts".into()]),
                skills: Some(vec![]),
                prompts: None,
                themes: None,
                extra: Default::default(),
                property_order: ["source", "extensions", "skills"]
                    .map(str::to_owned)
                    .to_vec(),
            }
        ]
    );
    m.set_packages(sources);
    q.drive();
    assert_eq!(disk(s.as_ref(), SettingsScope::Global), input);
}

#[test]
fn shell_prefix_loads_verbatim() {
    let (m, _) = memory(json!({"shellCommandPrefix":" shopt -s expand_aliases; "}));
    assert_eq!(
        m.get_shell_command_prefix().as_deref(),
        Some(" shopt -s expand_aliases; ")
    );
}

#[test]
fn missing_shell_prefix_stays_absent() {
    let (m, _) = memory(json!({}));
    assert_eq!(m.get_shell_command_prefix(), None);
}

#[test]
fn unrelated_save_keeps_shell_prefix() {
    let (m, s, q) = seeded(json!({"shellCommandPrefix":"prefix"}), json!({}));
    m.set_theme("dark".into());
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global)["shellCommandPrefix"],
        "prefix"
    );
}

#[test]
fn missing_session_directory_stays_absent() {
    let (m, _) = memory(json!({}));
    assert_eq!(m.get_session_dir(), None);
}

#[test]
fn global_session_directory_is_returned() {
    let (m, _) = memory(json!({"sessionDir":"literal"}));
    assert_eq!(m.get_session_dir().as_deref(), Some("literal"));
}

#[test]
fn project_session_directory_wins() {
    let (m, _, _) = seeded(
        json!({"sessionDir":"global"}),
        json!({"sessionDir":"project"}),
    );
    assert_eq!(m.get_session_dir().as_deref(), Some("project"));
}

#[test]
fn session_directory_expands_home_prefix() {
    let (m, _) = memory(json!({"sessionDir":"~/sessions"}));
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
    assert_eq!(m.get_session_dir(), Some(format!("{home}/sessions")));
}

#[test]
fn external_package_removal_survives_theme_save() {
    let (m, s, q) = seeded(json!({"packages":["old","remove"]}), json!({}));
    put(s.as_ref(), SettingsScope::Global, r#"{"packages":["old"]}"#);
    m.set_theme("dark".into());
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global)["packages"],
        json!(["old"])
    );
    let (m, s, q) = seeded(
        json!({"theme":"dark","packages":["npm:example-adapter"]}),
        json!({}),
    );
    assert_eq!(
        m.get_packages(),
        vec![maestro_settings::PackageSource::String(
            "npm:example-adapter".into()
        )]
    );
    put(
        s.as_ref(),
        SettingsScope::Global,
        r#"{"theme":"dark","packages":[]}"#,
    );
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global)["packages"],
        json!([])
    );
    m.set_theme("light".into());
    q.drive();
    block_on(m.flush());
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global)["packages"],
        json!([])
    );
    assert_eq!(disk(s.as_ref(), SettingsScope::Global)["theme"], "light");
}

#[test]
fn external_extensions_survive_thinking_save() {
    let (m, s, q) = seeded(json!({"extensions":["old"]}), json!({}));
    put(
        s.as_ref(),
        SettingsScope::Global,
        r#"{"extensions":["new"]}"#,
    );
    m.set_default_thinking_level("high".into());
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global)["extensions"],
        json!(["new"])
    );
}

#[test]
fn project_save_preserves_fresh_other_resource() {
    let (m, s, q) = seeded(json!({}), json!({"extensions":["old"],"skills":["old"]}));
    put(
        s.as_ref(),
        SettingsScope::Project,
        r#"{"extensions":["fresh"],"skills":["old"]}"#,
    );
    m.set_project_skill_paths(vec!["new".into()]);
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Project),
        json!({"extensions":["fresh"],"skills":["new"]})
    );
}

#[test]
fn project_save_wins_same_resource_edit() {
    let (m, s, q) = empty();
    put(
        s.as_ref(),
        SettingsScope::Project,
        r#"{"extensions":["fresh"]}"#,
    );
    m.set_project_extension_paths(vec!["accepted".into()]);
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Project)["extensions"],
        json!(["accepted"])
    );
}

#[test]
fn missing_and_empty_scopes_load_independently() {
    let s = Arc::new(maestro_settings::InMemorySettingsStorage::new());
    let m = SettingsManager::from_storage(s.clone(), Scheduler::default().spawn());
    assert_eq!(m.get_global_settings(), json!({}));
    assert_eq!(m.get_project_settings(), json!({}));
    put(s.as_ref(), SettingsScope::Global, "");
    put(s.as_ref(), SettingsScope::Project, r#"{"theme":"healthy"}"#);
    block_on(m.reload());
    assert_eq!(m.get_theme().as_deref(), Some("healthy"));
    assert!(m.drain_errors().is_empty());
    put(s.as_ref(), SettingsScope::Global, " ");
    put(s.as_ref(), SettingsScope::Project, "\t");
    block_on(m.reload());
    let e = m.drain_errors();
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].scope, SettingsScope::Global);
    assert_eq!(e[1].scope, SettingsScope::Project);
}

#[test]
fn root_arrays_are_accepted_without_object_admission() {
    for seed in [json!([]), json!(["a", "b"])] {
        let (m, _) = memory(seed.clone());
        assert_eq!(m.get_global_settings(), seed);
    }
    for seed in [Value::Null, json!(true), json!("x"), json!(4)] {
        assert!(SettingsManager::in_memory(seed, Scheduler::default().spawn()).is_err());
    }
    let (m, s, q) = seeded(json!(["x", "y"]), json!({}));
    m.set_theme("dark".into());
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global),
        json!({"0":"x","1":"y","theme":"dark"})
    );
}

#[test]
fn cache_publishes_before_scheduled_write() {
    let (m, s, q) = empty();
    m.apply_overrides(json!({"theme":"override"}));
    m.set_theme("new".into());
    assert_eq!(m.get_theme().as_deref(), Some("new"));
    assert_eq!(disk(s.as_ref(), SettingsScope::Global), json!({}));
    assert_eq!(q.len(), 1);
    q.drive();
    assert_eq!(disk(s.as_ref(), SettingsScope::Global)["theme"], "new");
}

#[test]
fn queued_jobs_progress_without_flush() {
    let (m, s, q) = empty();
    m.set_theme("new".into());
    q.drive();
    assert_eq!(disk(s.as_ref(), SettingsScope::Global)["theme"], "new");
    block_on(m.flush());
}

#[test]
fn fresh_disk_merge_does_not_publish_unrelated_edits() {
    for (text, first, second) in [("ab", "a", "b"), ("😀", "�", "�")] {
        let (m, s, q) = empty();
        put(
            s.as_ref(),
            SettingsScope::Global,
            &json!({"terminal":text}).to_string(),
        );
        m.set_show_images(false).unwrap();
        q.drive();
        assert_eq!(
            disk(s.as_ref(), SettingsScope::Global),
            json!({"terminal":{"0":first,"1":second,"showImages":false}})
        );
        if text == "ab" {
            assert_eq!(
                raw(s.as_ref(), SettingsScope::Global).unwrap(),
                "{\n  \"terminal\": {\n    \"0\": \"a\",\n    \"1\": \"b\",\n    \"showImages\": false\n  }\n}"
            );
        }
    }

    let (m, s, q) = seeded(
        json!({"compaction":{"enabled":true,"keepRecentTokens":8}}),
        json!({}),
    );
    put(
        s.as_ref(),
        SettingsScope::Global,
        r#"{"compaction":{"enabled":true,"keepRecentTokens":12,"custom":1},"external":true}"#,
    );
    m.set_compaction_enabled(false).unwrap();
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global),
        json!({"compaction":{"enabled":false,"keepRecentTokens":12,"custom":1},"external":true})
    );
    assert_eq!(m.get_compaction_keep_recent_tokens(), 8.0);
    assert!(m.get_global_settings().get("external").is_none());
    block_on(m.reload());
    assert_eq!(m.get_compaction_keep_recent_tokens(), 12.0);
}

#[test]
fn partial_reload_updates_healthy_scope_and_clears_overrides() {
    let (m, s, q) = seeded(json!({"theme":"old"}), json!({"extensions":["accepted"]}));
    m.set_theme("queued".into());
    m.apply_overrides(json!({"theme":"override"}));
    q.drive();
    put(s.as_ref(), SettingsScope::Global, r#"{"theme":"healthy"}"#);
    put(s.as_ref(), SettingsScope::Project, "{");
    block_on(m.reload());
    assert_eq!(m.get_theme().as_deref(), Some("healthy"));
    assert_eq!(m.get_extension_paths(), vec!["accepted"]);
    assert_eq!(m.drain_errors()[0].scope, SettingsScope::Project);
}

#[test]
fn load_latches_clear_only_after_successful_reload() {
    let (m, s, q) = empty();
    put(s.as_ref(), SettingsScope::Global, "{");
    block_on(m.reload());
    m.drain_errors();
    m.set_theme("session-only".into());
    assert_eq!(q.len(), 0);
    assert_eq!(m.get_theme().as_deref(), Some("session-only"));
    put(s.as_ref(), SettingsScope::Global, r#"{"theme":"repaired"}"#);
    m.set_default_model("lost".into());
    assert_eq!(q.len(), 0);
    block_on(m.reload());
    assert_eq!(m.get_theme().as_deref(), Some("repaired"));
    assert_eq!(m.get_default_model(), None);
    m.set_theme("saved".into());
    assert_eq!(q.len(), 1);
    q.drive();
    assert_eq!(disk(s.as_ref(), SettingsScope::Global)["theme"], "saved");
}

#[test]
fn unset_fields_omit_keys_without_null_replacement() {
    let (m, s, q) = seeded(
        json!({"shellPath":"x","shellCommandPrefix":"x","npmCommand":["x"],"enabledModels":["x"],"null":null}),
        json!({}),
    );
    m.set_shell_path(None);
    m.set_shell_command_prefix(None);
    m.set_npm_command(None);
    m.set_enabled_models(None);
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global),
        json!({"null":null})
    );
}

#[test]
fn conversions_run_on_seed_load_and_fresh_write() {
    let legacy = json!({"queueMode":"all","websockets":true,"skills":{"enableSkillCommands":false,"customDirectories":["s"]},"retry":{"maxDelayMs":3}});
    let expected = json!({"steeringMode":"all","transport":"websocket","skills":["s"],"enableSkillCommands":false,"retry":{"provider":{"maxRetryDelayMs":3}}});
    let (m, _) = memory(legacy.clone());
    assert_eq!(m.get_global_settings(), expected);
    assert!(legacy.get("queueMode").is_some());
    let (m, s, q) = seeded(legacy.clone(), json!({}));
    assert_eq!(m.get_global_settings(), expected);
    put(s.as_ref(), SettingsScope::Global, &legacy.to_string());
    m.set_theme("t".into());
    q.drive();
    let mut expected = expected;
    expected["theme"] = json!("t");
    assert_eq!(disk(s.as_ref(), SettingsScope::Global), expected);
}

/// overflow and lone UTF-16 surrogate escapes are unreadable JSON.

#[test]
fn json_boundaries_do_not_add_trim_or_unicode_repair() {
    for text in [" ", "\u{feff}{}", r#"{"n":1e400}"#, r#"{"s":"\uD800"}"#] {
        let (m, s, _) = empty();
        put(s.as_ref(), SettingsScope::Global, text);
        block_on(m.reload());
        assert_eq!(m.drain_errors().len(), 1);
    }
    let (m, s, _) = empty();
    put(
        s.as_ref(),
        SettingsScope::Global,
        " \n\t{\"shellCommandPrefix\":\"\u{feff}\u{85}\"}\r ",
    );
    block_on(m.reload());
    assert!(m.drain_errors().is_empty());
    assert_eq!(
        m.get_shell_command_prefix().as_deref(),
        Some("\u{feff}\u{85}")
    );
}

#[test]
fn settings_json_has_ecmascript_order_and_number_spelling() {
    let (m, s, q) = empty();
    m.set_default_model_and_provider("provider".into(), "model".into());
    q.drive();
    assert_eq!(
        raw(s.as_ref(), SettingsScope::Global).unwrap(),
        "{\n  \"defaultProvider\": \"provider\",\n  \"defaultModel\": \"model\"\n}"
    );
    m.set_show_terminal_progress(true).unwrap();
    m.set_show_images(false).unwrap();
    q.drive();
    assert!(
        raw(s.as_ref(), SettingsScope::Global)
            .unwrap()
            .contains("\"showTerminalProgress\": true,\n    \"showImages\": false")
    );

    let s = Arc::new(maestro_settings::InMemorySettingsStorage::new());
    put(
        s.as_ref(),
        SettingsScope::Global,
        r#"{"z":0,"10":10,"2":2,"0":0,"4294967294":4,"01":1,"-0":-0,"4294967295":5,"small":0.000001,"tiny":0.0000001,"large":100000000000000000000,"huge":1e21,"round":9007199254740993,"theme":"old","a":2}"#,
    );
    let q = Scheduler::default();
    let m = SettingsManager::from_storage(s.clone(), q.spawn());
    m.set_theme("new".into());
    q.drive();
    assert_eq!(
        raw(s.as_ref(), SettingsScope::Global).unwrap(),
        "{\n  \"0\": 0,\n  \"2\": 2,\n  \"10\": 10,\n  \"4294967294\": 4,\n  \"z\": 0,\n  \"01\": 1,\n  \"-0\": 0,\n  \"4294967295\": 5,\n  \"small\": 0.000001,\n  \"tiny\": 1e-7,\n  \"large\": 100000000000000000000,\n  \"huge\": 1e+21,\n  \"round\": 9007199254740992,\n  \"theme\": \"new\",\n  \"a\": 2\n}"
    );
}

/// named array properties are kept by typed reads but not JSON snapshots.
#[test]
fn nested_setters_preserve_container_branches() {
    type Setter = fn(&SettingsManager) -> Result<(), support::Error>;
    let setters: [(&str, &str, Setter); 8] = [
        ("compaction", "enabled", |m| m.set_compaction_enabled(false)),
        ("retry", "enabled", |m| m.set_retry_enabled(false)),
        ("terminal", "showImages", |m| m.set_show_images(false)),
        ("terminal", "imageWidthCells", |m| {
            m.set_image_width_cells(3.0)
        }),
        ("terminal", "clearOnShrink", |m| {
            m.set_clear_on_shrink(false)
        }),
        ("terminal", "showTerminalProgress", |m| {
            m.set_show_terminal_progress(false)
        }),
        ("images", "autoResize", |m| m.set_image_auto_resize(false)),
        ("images", "blockImages", |m| m.set_block_images(false)),
    ];
    for (field, child, setter) in setters {
        for value in [
            None,
            Some(Value::Null),
            Some(json!(false)),
            Some(json!(0)),
            Some(json!("")),
        ] {
            let mut seed = json!({});
            if let Some(v) = value {
                seed[field] = v;
            }
            let (m, q) = memory(seed);
            setter(&m).unwrap();
            assert!(m.get_global_settings()[field].get(child).is_some());
            assert_eq!(q.len(), 1);
        }
        for value in [json!(true), json!(1), json!("truthy")] {
            let seed = json!({field:value});
            let (m, q) = memory(seed.clone());
            let err = setter(&m).unwrap_err();
            assert!(err.to_string().contains(child));
            assert_eq!(m.get_global_settings(), seed);
            assert_eq!(q.len(), 0);
        }
        let (m, s, q) = seeded(json!({field:["a"]}), json!({}));
        setter(&m).unwrap();
        assert_eq!(m.get_global_settings()[field], json!(["a"]));
        q.drive();
        let out = disk(s.as_ref(), SettingsScope::Global);
        assert_eq!(out[field]["0"], "a");
        assert!(out[field].get(child).is_some());
    }
    let (m, _) = memory(json!({"compaction":[]}));
    m.set_compaction_enabled(false).unwrap();
    assert!(!m.get_compaction_enabled());
    let (m, q) = memory(json!([]));
    m.set_compaction_enabled(false).unwrap();
    assert!(!m.get_compaction_enabled());
    assert_eq!(m.get_global_settings(), json!([]));
    assert_eq!(q.len(), 1);
}

#[test]
fn write_snapshots_keep_cross_scope_enqueue_order() {
    let s = Arc::new(Controlled::default());
    let q = Scheduler::default();
    let m = SettingsManager::from_storage(s.clone(), q.spawn());
    m.set_theme("one".into());
    m.set_project_extension_paths(vec!["p".into()]);
    m.set_theme("two".into());
    m.set_default_model_and_provider("provider".into(), "model".into());
    assert_eq!(q.len(), 4);
    q.drive();
    let writes = s.writes.lock().unwrap();
    assert_eq!(
        writes.iter().map(|(scope, _)| *scope).collect::<Vec<_>>(),
        vec![
            SettingsScope::Global,
            SettingsScope::Project,
            SettingsScope::Global,
            SettingsScope::Global
        ]
    );
    let values = writes
        .iter()
        .map(|(_, text)| serde_json::from_str::<Value>(text).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values[0], json!({"theme":"one"}));
    assert_eq!(values[1], json!({"extensions":["p"]}));
    assert_eq!(values[2], json!({"theme":"two"}));
    assert_eq!(
        values[3],
        json!({"theme":"two","defaultProvider":"provider","defaultModel":"model"})
    );
}

#[test]
fn failed_writes_keep_memory_and_continue_queue() {
    let s = Arc::new(Controlled::default());
    let q = Scheduler::default();
    let m = SettingsManager::from_storage(s.clone(), q.spawn());
    *s.failures.lock().unwrap() = vec![SettingsScope::Global, SettingsScope::Project];
    m.set_theme("accepted".into());
    m.set_project_extension_paths(vec!["accepted".into()]);
    q.drive();
    assert_eq!(m.get_theme().as_deref(), Some("accepted"));
    assert_eq!(m.get_extension_paths(), vec!["accepted"]);
    let e = m.drain_errors();
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].scope, SettingsScope::Global);
    assert_eq!(e[1].scope, SettingsScope::Project);
    assert_eq!(e[0].error.downcast_ref::<Sentinel>().unwrap().0, 1);
    assert_eq!(e[1].error.to_string(), "sentinel-0");
    m.set_default_model("model".into());
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Global),
        json!({"theme":"accepted","defaultModel":"model"})
    );
    m.set_project_skill_paths(vec!["skill".into()]);
    q.drive();
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Project),
        json!({"extensions":["accepted"],"skills":["skill"]})
    );
    assert!(m.drain_errors().is_empty());
}

/// typed member admission is read-only; raw values remain unchanged.
#[test]
fn typed_reads_keep_wrong_typed_raw_values() {
    type Read = fn(&SettingsManager) -> Value;
    let cases: Vec<(&str, Read, Value, Value, Value, Value)> = vec![
        (
            "lastChangelogVersion",
            |m| json!(m.get_last_changelog_version()),
            json!(null),
            json!("valid"),
            json!(false),
            json!("valid"),
        ),
        (
            "defaultProvider",
            |m| json!(m.get_default_provider()),
            json!(null),
            json!("valid"),
            json!(false),
            json!("valid"),
        ),
        (
            "defaultModel",
            |m| json!(m.get_default_model()),
            json!(null),
            json!("valid"),
            json!(false),
            json!("valid"),
        ),
        (
            "defaultThinkingLevel",
            |m| json!(m.get_default_thinking_level()),
            json!(null),
            json!("valid"),
            json!(false),
            json!("valid"),
        ),
        (
            "theme",
            |m| json!(m.get_theme()),
            json!(null),
            json!("valid"),
            json!(false),
            json!("valid"),
        ),
        (
            "shellPath",
            |m| json!(m.get_shell_path()),
            json!(null),
            json!("valid"),
            json!(false),
            json!("valid"),
        ),
        (
            "shellCommandPrefix",
            |m| json!(m.get_shell_command_prefix()),
            json!(null),
            json!("valid"),
            json!(false),
            json!("valid"),
        ),
        (
            "sessionDir",
            |m| json!(m.get_session_dir()),
            json!(null),
            json!("valid"),
            json!(false),
            json!("valid"),
        ),
        (
            "steeringMode",
            |m| json!(m.get_steering_mode()),
            json!("one-at-a-time"),
            json!("valid"),
            json!(0),
            json!("valid"),
        ),
        (
            "followUpMode",
            |m| json!(m.get_follow_up_mode()),
            json!("one-at-a-time"),
            json!("valid"),
            json!(0),
            json!("valid"),
        ),
        (
            "transport",
            |m| json!(m.get_transport()),
            json!("auto"),
            json!("valid"),
            json!(0),
            json!("valid"),
        ),
        (
            "doubleEscapeAction",
            |m| json!(m.get_double_escape_action()),
            json!("tree"),
            json!("valid"),
            json!(0),
            json!("valid"),
        ),
        (
            "treeFilterMode",
            |m| json!(m.get_tree_filter_mode()),
            json!("default"),
            json!("all"),
            json!(0),
            json!("all"),
        ),
        (
            "markdown/codeBlockIndent",
            |m| json!(m.get_code_block_indent()),
            json!("  "),
            json!("valid"),
            json!(0),
            json!("valid"),
        ),
        (
            "compaction/enabled",
            |m| json!(m.get_compaction_enabled()),
            json!(true),
            json!(false),
            json!("wrong"),
            json!(false),
        ),
        (
            "branchSummary/skipPrompt",
            |m| json!(m.get_branch_summary_skip_prompt()),
            json!(false),
            json!(true),
            json!("wrong"),
            json!(true),
        ),
        (
            "retry/enabled",
            |m| json!(m.get_retry_enabled()),
            json!(true),
            json!(false),
            json!("wrong"),
            json!(false),
        ),
        (
            "hideThinkingBlock",
            |m| json!(m.get_hide_thinking_block()),
            json!(false),
            json!(true),
            json!("wrong"),
            json!(true),
        ),
        (
            "quietStartup",
            |m| json!(m.get_quiet_startup()),
            json!(false),
            json!(true),
            json!("wrong"),
            json!(true),
        ),
        (
            "collapseChangelog",
            |m| json!(m.get_collapse_changelog()),
            json!(false),
            json!(true),
            json!("wrong"),
            json!(true),
        ),
        (
            "enableInstallTelemetry",
            |m| json!(m.get_enable_install_telemetry()),
            json!(true),
            json!(false),
            json!("wrong"),
            json!(false),
        ),
        (
            "enableSkillCommands",
            |m| json!(m.get_enable_skill_commands()),
            json!(true),
            json!(false),
            json!("wrong"),
            json!(false),
        ),
        (
            "terminal/showImages",
            |m| json!(m.get_show_images()),
            json!(true),
            json!(false),
            json!("wrong"),
            json!(false),
        ),
        (
            "terminal/showTerminalProgress",
            |m| json!(m.get_show_terminal_progress()),
            json!(false),
            json!(true),
            json!("wrong"),
            json!(true),
        ),
        (
            "images/autoResize",
            |m| json!(m.get_image_auto_resize()),
            json!(true),
            json!(false),
            json!("wrong"),
            json!(false),
        ),
        (
            "images/blockImages",
            |m| json!(m.get_block_images()),
            json!(false),
            json!(true),
            json!("wrong"),
            json!(true),
        ),
        (
            "compaction/reserveTokens",
            |m| json!(m.get_compaction_reserve_tokens()),
            json!(16384.0),
            json!(7.0),
            json!("wrong"),
            json!(7.0),
        ),
        (
            "compaction/keepRecentTokens",
            |m| json!(m.get_compaction_keep_recent_tokens()),
            json!(20000.0),
            json!(7.0),
            json!("wrong"),
            json!(7.0),
        ),
        (
            "terminal/imageWidthCells",
            |m| json!(m.get_image_width_cells()),
            json!(60.0),
            json!(7.0),
            json!("wrong"),
            json!(7.0),
        ),
        (
            "editorPaddingX",
            |m| json!(m.get_editor_padding_x()),
            json!(0.0),
            json!(7.0),
            json!("wrong"),
            json!(7.0),
        ),
        (
            "autocompleteMaxVisible",
            |m| json!(m.get_autocomplete_max_visible()),
            json!(5.0),
            json!(7.0),
            json!("wrong"),
            json!(7.0),
        ),
        (
            "extensions",
            |m| json!(m.get_extension_paths()),
            json!([]),
            json!(["valid"]),
            json!([1]),
            json!(["valid"]),
        ),
        (
            "skills",
            |m| json!(m.get_skill_paths()),
            json!([]),
            json!(["valid"]),
            json!([1]),
            json!(["valid"]),
        ),
        (
            "prompts",
            |m| json!(m.get_prompt_template_paths()),
            json!([]),
            json!(["valid"]),
            json!([1]),
            json!(["valid"]),
        ),
        (
            "themes",
            |m| json!(m.get_theme_paths()),
            json!([]),
            json!(["valid"]),
            json!([1]),
            json!(["valid"]),
        ),
        (
            "enabledModels",
            |m| json!(m.get_enabled_models()),
            json!(null),
            json!(["valid"]),
            json!([1]),
            json!(["valid"]),
        ),
        (
            "npmCommand",
            |m| json!(m.get_npm_command()),
            json!(null),
            json!(["valid"]),
            json!([1]),
            json!(["valid"]),
        ),
        (
            "branchSummary/reserveTokens",
            |m| json!(m.get_branch_summary_settings().reserve_tokens),
            json!(16384.0),
            json!(7.0),
            json!(false),
            json!(7.0),
        ),
        (
            "retry/maxRetries",
            |m| json!(m.get_retry_settings().max_retries),
            json!(3.0),
            json!(7.0),
            json!(false),
            json!(7.0),
        ),
        (
            "retry/baseDelayMs",
            |m| json!(m.get_retry_settings().base_delay_ms),
            json!(2000.0),
            json!(7.0),
            json!(false),
            json!(7.0),
        ),
        (
            "retry/provider/timeoutMs",
            |m| json!(m.get_provider_retry_settings().timeout_ms),
            json!(null),
            json!(7.0),
            json!(false),
            json!(7.0),
        ),
        (
            "retry/provider/maxRetries",
            |m| json!(m.get_provider_retry_settings().max_retries),
            json!(null),
            json!(7.0),
            json!(false),
            json!(7.0),
        ),
        (
            "retry/provider/maxRetryDelayMs",
            |m| json!(m.get_provider_retry_settings().max_retry_delay_ms),
            json!(60000.0),
            json!(7.0),
            json!(false),
            json!(7.0),
        ),
        (
            "warnings/anthropicExtraUsage",
            |m| json!(m.get_warnings().anthropic_extra_usage),
            json!(null),
            json!(true),
            json!(0),
            json!(true),
        ),
        (
            "packages",
            |m| json!(m.get_packages().len()),
            json!(0),
            json!(["npm:valid"]),
            json!([{"source":false}]),
            json!(1),
        ),
    ];
    for (path, read, default, valid, wrong, expected) in cases {
        for (value, expected) in [
            (None, default.clone()),
            (Some(Value::Null), default.clone()),
            (Some(wrong), default),
            (Some(valid), expected),
        ] {
            let mut seed = json!({});
            if let Some(value) = value {
                let keys = path.split('/').collect::<Vec<_>>();
                let mut target = &mut seed;
                for key in &keys[..keys.len() - 1] {
                    target = target
                        .as_object_mut()
                        .unwrap()
                        .entry(*key)
                        .or_insert_with(|| json!({}));
                }
                target[keys[keys.len() - 1]] = value;
            }
            let (m, _) = memory(seed);
            let before = m.get_global_settings();
            assert_eq!(read(&m), expected, "{path}");
            assert_eq!(m.get_global_settings(), before, "{path} raw");
        }
    }
    for key in [
        "thinkingBudgets",
        "warnings",
        "compaction",
        "retry",
        "branchSummary",
        "terminal",
        "images",
        "markdown",
    ] {
        let (m, _) = memory(json!({key:"wrong"}));
        assert_eq!(m.get_global_settings()[key], "wrong");
        assert!(m.get_thinking_budgets().is_none());
        assert!(m.get_warnings().extra.is_empty());
    }
    for level in ["minimal", "low", "medium", "high"] {
        let (m, _) = memory(json!({"thinkingBudgets":{level:"wrong","other":1}}));
        let b = m.get_thinking_budgets().unwrap();
        assert_eq!([b.minimal, b.low, b.medium, b.high], [None; 4]);
        assert_eq!(b.extra["other"], 1);
    }
}

#[test]
fn typed_round_trips_preserve_exact_property_positions() {
    let input = json!({"packages":[{"source":"x","custom":1,"skills":["a"],"extensions":[]}],"warnings":{"anthropicExtraUsage":false,"custom":1}});
    let expected = "{\n  \"packages\": [\n    {\n      \"source\": \"x\",\n      \"custom\": 1,\n      \"skills\": [\n        \"a\"\n      ],\n      \"extensions\": []\n    }\n  ],\n  \"warnings\": {\n    \"anthropicExtraUsage\": false,\n    \"custom\": 1\n  }\n}";
    let (m, s, q) = seeded(input, json!({}));
    m.set_packages(m.get_packages());
    m.set_warnings(m.get_warnings());
    q.drive();
    assert_eq!(
        raw(s.as_ref(), SettingsScope::Global).as_deref(),
        Some(expected)
    );
}

#[test]
fn load_reports_storage_failure_after_malformed_callback_text() {
    struct FailingAfterCallback;
    impl maestro_settings::SettingsStorage for FailingAfterCallback {
        fn with_lock(
            &self,
            _: SettingsScope,
            operation: &mut dyn FnMut(Option<&str>) -> Result<Option<String>, Error>,
        ) -> Result<(), Error> {
            operation(Some("{"))?;
            Err(Box::new(Sentinel(42)))
        }
    }
    let m =
        SettingsManager::from_storage(Arc::new(FailingAfterCallback), Scheduler::default().spawn());
    for errors in [m.drain_errors(), {
        block_on(m.reload());
        m.drain_errors()
    }] {
        assert_eq!(errors.len(), 2);
        for error in errors {
            assert_eq!(error.error.to_string(), "sentinel-42");
            assert_eq!(error.error.downcast_ref::<Sentinel>().unwrap().0, 42);
        }
    }
}

#[test]
fn primitive_conversion_preserves_property_presence_operator_errors() {
    for (input, expected) in [
        (
            json!(null),
            "Cannot use 'in' operator to search for 'queueMode' in null",
        ),
        (
            json!(true),
            "Cannot use 'in' operator to search for 'queueMode' in true",
        ),
        (
            json!(1),
            "Cannot use 'in' operator to search for 'queueMode' in 1",
        ),
        (
            json!("text"),
            "Cannot use 'in' operator to search for 'queueMode' in text",
        ),
    ] {
        let error = SettingsManager::in_memory(input.clone(), Scheduler::default().spawn())
            .err()
            .unwrap();
        assert_eq!(error.to_string(), expected);
        let (m, _, _) = seeded(input, json!({}));
        assert_eq!(m.drain_errors()[0].error.to_string(), expected);
    }
}

#[test]
fn nested_assignment_preserves_primitive_operator_errors() {
    for (input, expected) in [
        (
            json!(true),
            "Cannot create property 'autoResize' on boolean 'true'",
        ),
        (
            json!(1),
            "Cannot create property 'autoResize' on number '1'",
        ),
        (
            json!("text"),
            "Cannot create property 'autoResize' on string 'text'",
        ),
    ] {
        let (m, q) = memory(json!({"images":input}));
        let before = m.get_global_settings();
        assert_eq!(
            m.set_image_auto_resize(false).unwrap_err().to_string(),
            expected
        );
        assert_eq!(m.get_global_settings(), before);
        assert_eq!(q.len(), 0);
    }
}

#[test]
fn block_images_defaults_to_false() {
    let (m, _) = memory(json!({}));
    assert!(!m.get_block_images());
}

#[test]
fn block_images_accepts_explicit_true() {
    let (m, _) = memory(json!({"images":{"blockImages":true}}));
    assert!(m.get_block_images());
}

#[test]
fn block_images_setter_toggles_preference() {
    let (m, q) = memory(json!({}));
    assert!(!m.get_block_images());
    m.set_block_images(true).unwrap();
    assert!(m.get_block_images());
    m.set_block_images(false).unwrap();
    assert!(!m.get_block_images());
    q.drive();
}

#[test]
fn block_images_coexists_with_auto_resize() {
    let (m, _) = memory(json!({"images":{"autoResize":true,"blockImages":true}}));
    assert!(m.get_image_auto_resize());
    assert!(m.get_block_images());
}
