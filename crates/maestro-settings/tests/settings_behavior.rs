//! Accessor, default, conversion and merge behavior of the settings manager.

#[cfg(test)]
mod support;

#[cfg(test)]
mod tests {
    use super::support;
    use super::support::{block_on, manager, manager_with, manager_with_both, raw_json, settings};
    use maestro_settings::{
        BranchSummarySettings, CompactionSettings, DoubleEscapeAction, ImageSettings,
        MarkdownSettings, MessageDeliveryMode, PackageSource, ProviderRetrySettings, RetrySettings,
        SettingsListEntry, SettingsManager, SettingsScope, TerminalSettings,
        ThinkingBudgetsSettings, ThinkingLevel, TransportSetting, TreeFilterMode, WarningSettings,
    };
    use serde_json::{Value, json};

    #[test]
    fn model_preferences_round_trip() {
        let (mut settings, storage) = manager();
        assert_eq!(settings.get_last_changelog_version(), None);
        assert_eq!(settings.get_default_provider(), None);
        assert_eq!(settings.get_default_model(), None);
        assert_eq!(settings.get_default_thinking_level(), None);
        assert_eq!(settings.get_theme(), None);

        settings.set_default_provider("local".into());
        settings.set_default_model("any/model:name".into());
        settings.set_theme("light".into());
        settings.set_last_changelog_version("1.2.3".into());
        settings.set_default_thinking_level(ThinkingLevel::High);
        assert_eq!(settings.get_default_provider().as_deref(), Some("local"));
        assert_eq!(
            settings.get_default_thinking_level(),
            Some(ThinkingLevel::High)
        );

        block_on(settings.flush());
        let saved = json!({
            "defaultProvider": "local",
            "defaultModel": "any/model:name",
            "theme": "light",
            "lastChangelogVersion": "1.2.3",
            "defaultThinkingLevel": "high"
        });
        assert_eq!(raw_json(&*storage, SettingsScope::Global), saved);

        block_on(settings.reload());
        assert_eq!(settings.get_theme().as_deref(), Some("light"));
        assert_eq!(
            settings.get_global_settings().0,
            saved.as_object().unwrap().clone()
        );
    }

    /// The steering mode, follow-up mode and transport of a manager.
    type Delivery = (MessageDeliveryMode, MessageDeliveryMode, TransportSetting);

    fn delivery(settings: &SettingsManager) -> Delivery {
        (
            settings.get_steering_mode(),
            settings.get_follow_up_mode(),
            settings.get_transport(),
        )
    }

    /// Stored delivery preferences and what they read as.
    fn stored_delivery_cases() -> Vec<(
        Value,
        (MessageDeliveryMode, MessageDeliveryMode, TransportSetting),
    )> {
        use MessageDeliveryMode::{OneAtATime, Unknown};
        vec![
            (
                json!({"steeringMode": "custom", "followUpMode": "", "transport": ""}),
                (
                    Unknown("custom".into()),
                    OneAtATime,
                    TransportSetting::Unknown(String::new()),
                ),
            ),
            (
                json!({"steeringMode": true, "followUpMode": 3, "transport": null}),
                (OneAtATime, OneAtATime, TransportSetting::Auto),
            ),
        ]
    }

    #[test]
    fn delivery_preferences_round_trip() {
        use MessageDeliveryMode::{All, OneAtATime};
        let (mut settings, storage) = manager();
        assert_eq!(
            delivery(&settings),
            (OneAtATime, OneAtATime, TransportSetting::Auto)
        );

        settings.set_steering_mode(All);
        settings.set_follow_up_mode(All);
        settings.set_transport(TransportSetting::Websocket);
        assert_eq!(delivery(&settings), (All, All, TransportSetting::Websocket));
        block_on(settings.flush());
        assert_eq!(
            raw_json(&*storage, SettingsScope::Global),
            json!({"steeringMode": "all", "followUpMode": "all", "transport": "websocket"})
        );
        settings.set_transport(TransportSetting::Sse);
        settings.set_steering_mode(OneAtATime);
        assert_eq!(
            delivery(&settings),
            (OneAtATime, All, TransportSetting::Sse)
        );

        for (stored, expected) in stored_delivery_cases() {
            let (settings, _) = manager_with(stored);
            assert_eq!(delivery(&settings), expected);
        }
    }

    /// Every resolved context and retry record of a manager, as JSON.
    fn resolved(settings: &SettingsManager) -> Value {
        let compaction = settings.get_compaction_settings();
        let summary = settings.get_branch_summary_settings();
        let retry = settings.get_retry_settings();
        let provider = settings.get_provider_retry_settings();
        json!({
            "compaction": [compaction.enabled, compaction.reserve_tokens, compaction.keep_recent_tokens],
            "summary": [summary.reserve_tokens, summary.skip_prompt],
            "retry": [retry.enabled, retry.max_retries, retry.base_delay_ms],
            "provider": [provider.timeout_ms, provider.max_retries, provider.max_retry_delay_ms],
        })
    }

    #[test]
    fn context_and_retry_defaults_resolve() {
        let (mut settings, storage) = manager();
        assert_eq!(
            resolved(&settings),
            json!({
                "compaction": [true, 16384.0, 20000.0], "summary": [16384.0, false],
                "retry": [true, 3.0, 2000.0], "provider": [null, null, 60000.0],
            })
        );
        assert!(settings.get_compaction_enabled() && settings.get_retry_enabled());
        assert!(!settings.get_branch_summary_skip_prompt());

        settings.set_compaction_enabled(false);
        settings.set_retry_enabled(false);
        assert!(!settings.get_compaction_enabled() && !settings.get_retry_enabled());
        block_on(settings.flush());
        assert_eq!(
            raw_json(&*storage, SettingsScope::Global),
            json!({"compaction": {"enabled": false}, "retry": {"enabled": false}})
        );

        let (settings, _) = manager_with(json!({
            "compaction": {"enabled": "yes", "reserveTokens": 0, "keepRecentTokens": -5},
            "branchSummary": {"reserveTokens": 12.5, "skipPrompt": true},
            "retry": {"maxRetries": "many", "baseDelayMs": 0.5,
                "provider": {"timeoutMs": 90, "maxRetries": 0, "maxRetryDelayMs": -1}}
        }));
        assert_eq!(
            resolved(&settings),
            json!({
                "compaction": [true, 0.0, -5.0], "summary": [12.5, true],
                "retry": [true, 3.0, 0.5], "provider": [90.0, 0.0, -1.0],
            })
        );
        assert!(settings.get_branch_summary_skip_prompt());

        let (settings, _) = manager_with_both(
            json!({"retry": {"provider": {"timeoutMs": 5}}}),
            json!({"retry": {"provider": {"maxRetries": 2}}}),
        );
        assert_eq!(resolved(&settings)["provider"], json!([null, 2.0, 60000.0]));
    }

    /// One boolean preference with its default, location and accessors.
    struct Toggle {
        path: &'static [&'static str],
        default: Option<bool>,
        get: fn(&SettingsManager) -> bool,
        set: fn(&mut SettingsManager, bool),
    }

    /// Nests `value` under the object path.
    fn nest(path: &[&str], value: Value) -> Value {
        path.iter()
            .rev()
            .fold(value, |inner, key| json!({ *key: inner }))
    }

    /// Reads a boolean preference.
    type Getter = fn(&SettingsManager) -> bool;
    /// Writes a boolean preference.
    type Setter = fn(&mut SettingsManager, bool);

    impl Toggle {
        fn new(
            path: &'static [&'static str],
            default: Option<bool>,
            get: Getter,
            set: Setter,
        ) -> Self {
            Self {
                path,
                default,
                get,
                set,
            }
        }
    }

    fn top_level_toggles() -> Vec<Toggle> {
        type M = SettingsManager;
        vec![
            Toggle::new(
                &["hideThinkingBlock"],
                Some(false),
                M::get_hide_thinking_block,
                M::set_hide_thinking_block,
            ),
            Toggle::new(
                &["quietStartup"],
                Some(false),
                M::get_quiet_startup,
                M::set_quiet_startup,
            ),
            Toggle::new(
                &["collapseChangelog"],
                Some(false),
                M::get_collapse_changelog,
                M::set_collapse_changelog,
            ),
            Toggle::new(
                &["enableInstallTelemetry"],
                Some(true),
                M::get_enable_install_telemetry,
                M::set_enable_install_telemetry,
            ),
            Toggle::new(
                &["enableSkillCommands"],
                Some(true),
                M::get_enable_skill_commands,
                M::set_enable_skill_commands,
            ),
            Toggle::new(
                &["showHardwareCursor"],
                None,
                M::get_show_hardware_cursor,
                M::set_show_hardware_cursor,
            ),
        ]
    }

    fn nested_toggles() -> Vec<Toggle> {
        type M = SettingsManager;
        vec![
            Toggle::new(
                &["terminal", "showImages"],
                Some(true),
                M::get_show_images,
                M::set_show_images,
            ),
            Toggle::new(
                &["terminal", "clearOnShrink"],
                None,
                M::get_clear_on_shrink,
                M::set_clear_on_shrink,
            ),
            Toggle::new(
                &["terminal", "showTerminalProgress"],
                Some(false),
                M::get_show_terminal_progress,
                M::set_show_terminal_progress,
            ),
            Toggle::new(
                &["images", "autoResize"],
                Some(true),
                M::get_image_auto_resize,
                M::set_image_auto_resize,
            ),
            Toggle::new(
                &["images", "blockImages"],
                Some(false),
                M::get_block_images,
                M::set_block_images,
            ),
        ]
    }

    fn toggles() -> Vec<Toggle> {
        let mut all = top_level_toggles();
        all.extend(nested_toggles());
        all
    }

    #[test]
    fn toggles_publish_before_persistence() {
        for toggle in toggles() {
            let flipped = !toggle.default.unwrap_or(false);
            let (mut settings, storage) = manager();
            if let Some(default) = toggle.default {
                assert_eq!((toggle.get)(&settings), default, "{:?}", toggle.path);
            }
            (toggle.set)(&mut settings, flipped);
            assert_eq!((toggle.get)(&settings), flipped, "{:?}", toggle.path);
            block_on(settings.flush());
            assert_eq!(
                raw_json(&*storage, SettingsScope::Global),
                nest(toggle.path, json!(flipped)),
                "{:?}",
                toggle.path
            );

            let (mut settings, storage) =
                manager_with_both(json!({}), nest(toggle.path, json!(true)));
            (toggle.set)(&mut settings, false);
            assert!(
                (toggle.get)(&settings),
                "project wins for {:?}",
                toggle.path
            );
            block_on(settings.flush());
            assert_eq!(
                raw_json(&*storage, SettingsScope::Global),
                nest(toggle.path, json!(false))
            );
            assert_eq!(
                raw_json(&*storage, SettingsScope::Project),
                nest(toggle.path, json!(true))
            );

            if let [parent, key] = toggle.path {
                let (mut settings, storage) = manager_with(json!({ *parent: {"sibling": 1} }));
                (toggle.set)(&mut settings, flipped);
                block_on(settings.flush());
                assert_eq!(
                    raw_json(&*storage, SettingsScope::Global),
                    json!({ *parent: {"sibling": 1, *key: flipped} })
                );
            }
        }
    }

    fn text_entries(items: &[&str]) -> Vec<SettingsListEntry> {
        items
            .iter()
            .map(|item| SettingsListEntry::String((*item).into()))
            .collect()
    }

    /// Unset command preferences read as absent.
    fn assert_command_defaults() {
        let (settings, _) = manager();
        assert_eq!(settings.get_shell_path(), None);
        assert_eq!(settings.get_shell_command_prefix(), None);
        assert_eq!(settings.get_npm_command(), None);
        assert_eq!(settings.get_enabled_models(), None);
    }

    /// The prefix is readable before any setter and survives an unrelated save.
    fn assert_prefix_survives_unrelated_save() {
        let (mut settings, storage) =
            manager_with(json!({"shellCommandPrefix": "shopt -s expand_aliases"}));
        let prefix = settings.get_shell_command_prefix();
        assert_eq!(prefix.as_deref(), Some("shopt -s expand_aliases"));
        settings.set_theme("dark".into());
        block_on(settings.flush());
        assert_eq!(
            raw_json(&*storage, SettingsScope::Global),
            json!({"shellCommandPrefix": "shopt -s expand_aliases", "theme": "dark"})
        );
    }

    /// Setting stores values, unsetting removes the keys, empty values stay present.
    fn assert_set_and_unset() {
        let (mut settings, storage) = manager();
        settings.set_shell_path(Some("/bin/sh".into()));
        settings.set_npm_command(Some(text_entries(&["mise", "exec"])));
        settings.set_enabled_models(Some(text_entries(&["model*"])));
        block_on(settings.flush());
        assert_eq!(
            raw_json(&*storage, SettingsScope::Global),
            json!({"shellPath": "/bin/sh", "npmCommand": ["mise", "exec"], "enabledModels": ["model*"]})
        );
        settings.set_shell_path(None);
        settings.set_shell_command_prefix(None);
        settings.set_npm_command(None);
        settings.set_enabled_models(None);
        block_on(settings.flush());
        assert_eq!(raw_json(&*storage, SettingsScope::Global), json!({}));

        settings.set_shell_command_prefix(Some(String::new()));
        settings.set_npm_command(Some(Vec::new()));
        assert_eq!(settings.get_shell_command_prefix().as_deref(), Some(""));
        assert_eq!(settings.get_npm_command(), Some(Vec::new()));
        block_on(settings.flush());
        assert_eq!(
            raw_json(&*storage, SettingsScope::Global),
            json!({"shellCommandPrefix": "", "npmCommand": []})
        );
    }

    #[test]
    fn command_preferences_unset_without_null() {
        assert_command_defaults();
        assert_prefix_survives_unrelated_save();
        assert_set_and_unset();

        let (settings, _) =
            manager_with(json!({"shellPath": 5, "npmCommand": "npm", "enabledModels": ["a", 7]}));
        assert_eq!(settings.get_shell_path(), None);
        assert_eq!(settings.get_npm_command(), None);
        let models = vec![
            SettingsListEntry::String("a".into()),
            SettingsListEntry::Unknown(json!(7)),
        ];
        assert_eq!(settings.get_enabled_models(), Some(models));
    }

    type ListGet = fn(&SettingsManager) -> Vec<SettingsListEntry>;
    type ListSet = fn(&mut SettingsManager, Vec<SettingsListEntry>);

    #[test]
    fn resource_lists_keep_order_and_opaque_members() {
        let lists: [(&str, ListGet, ListSet, ListSet); 4] = [
            (
                "extensions",
                SettingsManager::get_extension_paths,
                SettingsManager::set_extension_paths,
                SettingsManager::set_project_extension_paths,
            ),
            (
                "skills",
                SettingsManager::get_skill_paths,
                SettingsManager::set_skill_paths,
                SettingsManager::set_project_skill_paths,
            ),
            (
                "prompts",
                SettingsManager::get_prompt_template_paths,
                SettingsManager::set_prompt_template_paths,
                SettingsManager::set_project_prompt_template_paths,
            ),
            (
                "themes",
                SettingsManager::get_theme_paths,
                SettingsManager::set_theme_paths,
                SettingsManager::set_project_theme_paths,
            ),
        ];
        for (key, get, set_global, set_project) in lists {
            let (mut settings, storage) = manager();
            assert_eq!(get(&settings), Vec::new(), "{key} defaults to empty");
            set_global(&mut settings, text_entries(&["b", "a", "c"]));
            assert_eq!(get(&settings), text_entries(&["b", "a", "c"]));
            set_project(
                &mut settings,
                vec![
                    SettingsListEntry::String("p".into()),
                    SettingsListEntry::Unknown(json!({"opaque": 1})),
                ],
            );
            assert_eq!(
                get(&settings),
                vec![
                    SettingsListEntry::String("p".into()),
                    SettingsListEntry::Unknown(json!({"opaque": 1}))
                ],
                "the project list replaces the global one"
            );
            block_on(settings.flush());
            assert_eq!(
                raw_json(&*storage, SettingsScope::Global),
                json!({ key: ["b", "a", "c"] })
            );
            assert_eq!(
                raw_json(&*storage, SettingsScope::Project),
                json!({ key: ["p", {"opaque": 1}] })
            );
        }
        let (settings, _) = manager_with(json!({"extensions": "not-a-list", "skills": {"a": 1}}));
        assert_eq!(settings.get_extension_paths(), Vec::new());
        assert_eq!(settings.get_skill_paths(), Vec::new());
    }

    /// Stored package entries covering strings, filtered objects and malformed shapes.
    fn stored_packages() -> Value {
        json!([
            "npm:custom",
            {"source": "git:custom", "extensions": [], "skills": ["a", 1], "opaque": {"x": 2}},
            {"source": "git:bare"},
            {"source": "git:bad", "skills": null},
            {"source": 4},
            12,
            null,
            {}
        ])
    }

    /// The first three stored entries are typed; the rest stay opaque.
    fn assert_typed_packages(packages: &[PackageSource]) {
        assert_eq!(packages.len(), 8);
        assert_eq!(packages[0], PackageSource::Source("npm:custom".into()));
        let PackageSource::Filtered {
            source,
            filters,
            extra,
        } = &packages[1]
        else {
            panic!("filtered entry expected, got {:?}", packages[1]);
        };
        assert_eq!(source, "git:custom");
        assert_eq!(filters.extensions, Some(Vec::new()));
        let skills = vec![
            SettingsListEntry::String("a".into()),
            SettingsListEntry::Unknown(json!(1)),
        ];
        assert_eq!(filters.skills, Some(skills));
        assert_eq!((&filters.prompts, &filters.themes), (&None, &None));
        assert_eq!(extra.get("opaque"), Some(&json!({"x": 2})));
        assert!(
            matches!(&packages[2], PackageSource::Filtered { filters, .. } if filters.skills.is_none())
        );
        assert!(
            packages[3..]
                .iter()
                .all(|entry| matches!(entry, PackageSource::Unknown(_)))
        );
    }

    #[test]
    fn packages_keep_filters_and_unknown_members() {
        let stored = stored_packages();
        let (mut settings, storage) = manager_with(json!({"packages": stored}));
        let packages = settings.get_packages();
        assert_typed_packages(&packages);

        settings.set_packages(packages);
        settings.set_project_packages(vec![PackageSource::Source("npm:project".into())]);
        assert_eq!(
            settings.get_packages(),
            vec![PackageSource::Source("npm:project".into())]
        );
        block_on(settings.flush());
        assert_eq!(
            raw_json(&*storage, SettingsScope::Global),
            json!({"packages": stored})
        );
        assert_eq!(
            raw_json(&*storage, SettingsScope::Project),
            json!({"packages": ["npm:project"]})
        );

        let (settings, _) = manager_with(json!({"packages": "npm:custom"}));
        assert_eq!(settings.get_packages(), Vec::new());
    }

    #[test]
    fn no_packages_inferred_from_extensions() {
        let (settings, _) = manager_with(json!({"extensions": ["/local/ext", "./relative/ext"]}));
        assert_eq!(settings.get_packages(), Vec::new());
        assert_eq!(
            settings.get_extension_paths(),
            text_entries(&["/local/ext", "./relative/ext"])
        );
    }

    #[test]
    fn owned_reads_do_not_mutate_manager() {
        let seed = settings(json!({
            "extensions": ["a"], "packages": ["npm:p"],
            "thinkingBudgets": {"low": 1}, "warnings": {"anthropicExtraUsage": true}
        }));
        let mut manager = SettingsManager::in_memory(seed.clone());
        let mut snapshot = manager.get_global_settings();
        snapshot.0.insert("theme".into(), json!("leaked"));
        let mut project = manager.get_project_settings();
        project.0.insert("theme".into(), json!("leaked"));
        let mut extensions = manager.get_extension_paths();
        extensions.push(SettingsListEntry::String("leaked".into()));
        let mut packages = manager.get_packages();
        packages.clear();
        let mut budgets = manager.get_thinking_budgets().unwrap();
        budgets.low = Some(99.0);
        let mut warnings: WarningSettings = manager.get_warnings();
        warnings.anthropic_extra_usage = Some(false);

        assert_eq!(manager.get_theme(), None);
        assert_eq!(manager.get_project_settings().0.len(), 0);
        assert_eq!(manager.get_global_settings(), seed);
        assert_eq!(manager.get_extension_paths(), text_entries(&["a"]));
        assert_eq!(manager.get_packages().len(), 1);
        assert_eq!(manager.get_thinking_budgets().unwrap().low, Some(1.0));
        assert_eq!(manager.get_warnings().anthropic_extra_usage, Some(true));
        manager.set_theme("kept".into());
        assert_eq!(manager.get_theme().as_deref(), Some("kept"));
    }

    /// Probes an effective value through typed getters.
    type Probe = fn(&SettingsManager) -> Value;

    /// Global and project documents, a probe of the effective value and its expectation.
    type MergeCase = (Value, Value, Probe, Value);

    fn probe_theme(settings: &SettingsManager) -> Value {
        json!(settings.get_theme())
    }

    fn probe_retry(settings: &SettingsManager) -> Value {
        let provider = settings.get_provider_retry_settings();
        json!([
            settings.get_retry_enabled(),
            provider.timeout_ms,
            provider.max_retries,
            provider.max_retry_delay_ms
        ])
    }

    fn probe_extension_count(settings: &SettingsManager) -> Value {
        json!(settings.get_extension_paths().len())
    }

    fn probe_show_images(settings: &SettingsManager) -> Value {
        json!(settings.get_show_images())
    }

    fn merge_cases() -> Vec<MergeCase> {
        let retry_global =
            json!({"retry": {"enabled": false, "provider": {"timeoutMs": 11, "maxRetries": 2}}});
        vec![
            (
                json!({"theme": "global"}),
                json!({"theme": "project"}),
                probe_theme,
                json!("project"),
            ),
            (
                json!({"theme": "global"}),
                json!({"theme": null}),
                probe_theme,
                json!(null),
            ),
            (
                json!({"theme": "global"}),
                json!({}),
                probe_theme,
                json!("global"),
            ),
            (
                retry_global,
                json!({"retry": {"provider": {"maxRetryDelayMs": 3}}}),
                probe_retry,
                json!([false, null, null, 3.0]),
            ),
            (
                json!({"extensions": ["a"]}),
                json!({"extensions": []}),
                probe_extension_count,
                json!(0),
            ),
            (
                json!({"terminal": {"showImages": false}}),
                json!({"terminal": "text"}),
                probe_show_images,
                json!(true),
            ),
            (
                json!({"terminal": 5}),
                json!({"terminal": {"showImages": false}}),
                probe_show_images,
                json!(false),
            ),
            (
                json!({"terminal": {"showImages": false}}),
                json!({"terminal": {"imageWidthCells": 3}}),
                probe_show_images,
                json!(false),
            ),
        ]
    }

    #[test]
    fn scopes_merge_one_object_level() {
        for (global, project, probe, expected) in merge_cases() {
            let (settings, _) = manager_with_both(global.clone(), project.clone());
            assert_eq!(
                probe(&settings),
                expected,
                "global {global} project {project}"
            );
            assert_eq!(
                settings.get_global_settings().0,
                global.as_object().unwrap().clone()
            );
            assert_eq!(
                settings.get_project_settings().0,
                project.as_object().unwrap().clone()
            );
        }
    }

    #[test]
    fn overrides_layer_without_persisting() {
        let seeds = (
            json!({"theme": "global", "retry": {"enabled": false}}),
            json!({"theme": "project"}),
        );
        let (mut settings, storage) = manager_with_both(seeds.0.clone(), seeds.1.clone());
        settings.apply_overrides(support::settings(
            json!({"theme": "runtime", "retry": {"maxRetries": 8}}),
        ));
        settings.apply_overrides(support::settings(json!({"retry": {"baseDelayMs": 5}})));
        assert_eq!(settings.get_theme().as_deref(), Some("runtime"));
        let retry = settings.get_retry_settings();
        assert_eq!(
            (retry.enabled, retry.max_retries, retry.base_delay_ms),
            (false, 8.0, 5.0)
        );
        assert_eq!(
            settings.get_global_settings().0,
            seeds.0.as_object().unwrap().clone()
        );
        assert_eq!(
            settings.get_project_settings().0,
            seeds.1.as_object().unwrap().clone()
        );
        block_on(settings.flush());
        assert_eq!(raw_json(&*storage, SettingsScope::Global), seeds.0);
        assert_eq!(raw_json(&*storage, SettingsScope::Project), seeds.1);

        settings.set_quiet_startup(true);
        assert_eq!(settings.get_theme().as_deref(), Some("project"));
        support::assert_number(settings.get_retry_settings().max_retries, 3.0);
    }

    /// Loads a global document and returns the accepted global snapshot.
    fn loaded(global: Value) -> Value {
        let (settings, _) = manager_with(global);
        Value::Object(settings.get_global_settings().0)
    }

    #[test]
    fn conversion_queue_target_presence() {
        for legacy in [json!("all"), json!(null), json!(false), json!(0), json!("")] {
            assert_eq!(
                loaded(json!({"queueMode": legacy})),
                json!({"steeringMode": legacy})
            );
        }
        for target in [json!(null), json!("all")] {
            let both = json!({"queueMode": "one-at-a-time", "steeringMode": target});
            assert_eq!(loaded(both.clone()), both);
        }
    }

    #[test]
    fn conversion_transport_boolean_only() {
        assert_eq!(
            loaded(json!({"websockets": true})),
            json!({"transport": "websocket"})
        );
        assert_eq!(
            loaded(json!({"websockets": false})),
            json!({"transport": "sse"})
        );
        for target in [json!(null), json!(""), json!("sse")] {
            let both = json!({"websockets": true, "transport": target});
            assert_eq!(loaded(both.clone()), both);
        }
        for legacy in [json!(null), json!(0), json!("true")] {
            let unchanged = json!({"websockets": legacy});
            assert_eq!(loaded(unchanged.clone()), unchanged);
        }
    }

    #[test]
    fn conversion_skills_object() {
        let cases = [
            (
                json!({"skills": {"enableSkillCommands": false, "customDirectories": ["a", 3]}}),
                json!({"skills": ["a", 3], "enableSkillCommands": false}),
            ),
            (json!({"skills": {}}), json!({})),
            (
                json!({"skills": {"enableSkillCommands": null, "customDirectories": []}}),
                json!({"enableSkillCommands": null}),
            ),
            (
                json!({"skills": {"enableSkillCommands": false}, "enableSkillCommands": true}),
                json!({"enableSkillCommands": true}),
            ),
            (
                json!({"skills": {"enableSkillCommands": false}, "enableSkillCommands": null}),
                json!({"enableSkillCommands": null}),
            ),
            (json!({"skills": {"customDirectories": "x"}}), json!({})),
        ];
        for (stored, expected) in cases {
            assert_eq!(loaded(stored.clone()), expected, "{stored}");
        }
        for unchanged in [
            json!({"skills": null}),
            json!({"skills": []}),
            json!({"skills": false}),
            json!({"skills": "text"}),
            json!({"skills": ["a"]}),
        ] {
            assert_eq!(loaded(unchanged.clone()), unchanged);
        }
    }

    #[test]
    fn conversion_retry_delay() {
        let cases = [
            (
                json!({"retry": {"maxDelayMs": 12.5}}),
                json!({"retry": {"provider": {"maxRetryDelayMs": 12.5}}}),
            ),
            (
                json!({"retry": {"maxDelayMs": 9, "provider": {"timeoutMs": 3, "maxRetryDelayMs": null}}}),
                json!({"retry": {"provider": {"timeoutMs": 3, "maxRetryDelayMs": 9}}}),
            ),
            (
                json!({"retry": {"maxDelayMs": 9, "provider": {"maxRetryDelayMs": 7, "custom": 1}}}),
                json!({"retry": {"provider": {"maxRetryDelayMs": 7, "custom": 1}}}),
            ),
            (
                json!({"retry": {"maxDelayMs": "1", "provider": {"timeoutMs": 3}}}),
                json!({"retry": {"provider": {"timeoutMs": 3}}}),
            ),
            (
                json!({"retry": {"maxDelayMs": null, "enabled": false}}),
                json!({"retry": {"enabled": false}}),
            ),
            (
                json!({"retry": {"maxDelayMs": 4, "provider": false}}),
                json!({"retry": {"provider": {"maxRetryDelayMs": 4}}}),
            ),
            (
                json!({"retry": {"maxDelayMs": 4, "provider": ["x"]}}),
                json!({"retry": {"provider": {"maxRetryDelayMs": 4}}}),
            ),
            (
                json!({"retry": {"maxDelayMs": "n", "provider": false}}),
                json!({"retry": {"provider": false}}),
            ),
        ];
        for (stored, expected) in cases {
            assert_eq!(loaded(stored.clone()), expected, "{stored}");
        }
        for unchanged in [
            json!({"retry": null}),
            json!({"retry": [1]}),
            json!({"retry": "x"}),
        ] {
            assert_eq!(loaded(unchanged.clone()), unchanged);
        }
    }

    /// Applies setters, flushes and returns the manager with its saved global document.
    fn numeric_after<F: FnOnce(&mut SettingsManager)>(set: F) -> (SettingsManager, Value) {
        let (mut settings, storage) = manager();
        set(&mut settings);
        block_on(settings.flush());
        let saved = raw_json(&*storage, SettingsScope::Global);
        (settings, saved)
    }

    /// Loaded numbers: width is floored with a minimum of one, the others stay unclamped.
    fn assert_loaded_numbers() {
        let (settings, _) = manager_with(json!({
            "terminal": {"imageWidthCells": -3.8}, "editorPaddingX": -3.8, "autocompleteMaxVisible": 21
        }));
        support::assert_number(settings.get_image_width_cells(), 1.0);
        support::assert_number(settings.get_editor_padding_x(), -3.8);
        support::assert_number(settings.get_autocomplete_max_visible(), 21.0);
        let widths = [
            (json!(0), 1.0),
            (json!(2.9), 2.0),
            (json!(60), 60.0),
            (json!(1e20), 1e20),
        ];
        let rejected = [json!("3"), json!(null), json!([])];
        for (stored, width) in widths
            .into_iter()
            .chain(rejected.map(|stored| (stored, 60.0)))
        {
            let (settings, _) = manager_with(json!({"terminal": {"imageWidthCells": stored}}));
            support::assert_number(settings.get_image_width_cells(), width);
        }
        let (settings, _) =
            manager_with(json!({"editorPaddingX": "3", "autocompleteMaxVisible": null}));
        support::assert_number(settings.get_editor_padding_x(), 0.0);
        support::assert_number(settings.get_autocomplete_max_visible(), 5.0);
    }

    /// Setter input with the width, padding and row count it stores.
    const SETTER_ROWS: [(f64, f64, f64, f64); 13] = [
        (f64::NEG_INFINITY, 1.0, 0.0, 3.0),
        (-3.8, 1.0, 0.0, 3.0),
        (-0.0, 1.0, 0.0, 3.0),
        (0.9, 1.0, 0.0, 3.0),
        (1.0, 1.0, 1.0, 3.0),
        (2.9, 2.0, 2.0, 3.0),
        (3.0, 3.0, 3.0, 3.0),
        (7.9, 7.0, 3.0, 7.0),
        (19.9, 19.0, 3.0, 19.0),
        (20.0, 20.0, 3.0, 20.0),
        (21.0, 21.0, 3.0, 20.0),
        (1e20, 1e20, 3.0, 20.0),
        (f64::INFINITY, 60.0, 3.0, 20.0),
    ];

    #[test]
    fn numeric_accessors_normalize_only_authored_fields() {
        assert_loaded_numbers();
        for (value, width, padding, rows) in SETTER_ROWS {
            let (settings, saved) = numeric_after(|m| {
                m.set_image_width_cells(value);
                m.set_editor_padding_x(value);
                m.set_autocomplete_max_visible(value);
            });
            support::assert_number(settings.get_image_width_cells(), width);
            support::assert_number(settings.get_editor_padding_x(), padding);
            support::assert_number(settings.get_autocomplete_max_visible(), rows);
            if value.is_finite() {
                assert_eq!(saved["editorPaddingX"].as_f64(), Some(padding));
                assert_eq!(saved["autocompleteMaxVisible"].as_f64(), Some(rows));
            }
        }
    }

    /// `NaN` is held as a typed result until reload, while snapshots and files hold `null`.
    fn assert_nan_until_reload() {
        let (mut settings, storage) = manager();
        settings.set_editor_padding_x(f64::NAN);
        settings.set_autocomplete_max_visible(f64::NAN);
        settings.set_image_width_cells(f64::NAN);
        assert!(settings.get_editor_padding_x().is_nan());
        assert!(settings.get_autocomplete_max_visible().is_nan());
        support::assert_number(settings.get_image_width_cells(), 60.0);
        settings.set_theme("unrelated".into());
        assert!(
            settings.get_editor_padding_x().is_nan(),
            "an unrelated setter keeps the result"
        );
        assert_eq!(
            settings.get_global_settings().0["editorPaddingX"],
            Value::Null
        );
        block_on(settings.flush());
        assert_eq!(
            raw_json(&*storage, SettingsScope::Global),
            json!({
                "editorPaddingX": null, "autocompleteMaxVisible": null,
                "terminal": {"imageWidthCells": null}, "theme": "unrelated"
            })
        );
        block_on(settings.reload());
        support::assert_number(settings.get_editor_padding_x(), 0.0);
        support::assert_number(settings.get_autocomplete_max_visible(), 5.0);
    }

    /// Anything that supplies its own value supersedes the typed `NaN` result.
    fn assert_nan_superseded() {
        let (mut settings, _) = manager();
        settings.set_editor_padding_x(f64::NAN);
        settings.set_editor_padding_x(2.0);
        support::assert_number(settings.get_editor_padding_x(), 2.0);

        let document = json!({"editorPaddingX": 2, "autocompleteMaxVisible": null});
        let (mut settings, _) = manager_with_both(json!({}), document);
        settings.set_editor_padding_x(f64::NAN);
        settings.set_autocomplete_max_visible(f64::NAN);
        support::assert_number(settings.get_editor_padding_x(), 2.0);
        support::assert_number(settings.get_autocomplete_max_visible(), 5.0);

        let (mut settings, _) = manager();
        settings.set_editor_padding_x(f64::NAN);
        settings.apply_overrides(support::settings(json!({"editorPaddingX": 4})));
        support::assert_number(settings.get_editor_padding_x(), 4.0);
    }

    #[test]
    fn numeric_setters_keep_nonfinite_until_reload() {
        assert_nan_until_reload();
        assert_nan_superseded();
    }

    /// Setters and defaults of the double-escape action and tree filter mode.
    fn assert_mode_setters() {
        let (mut settings, storage) = manager();
        assert_eq!(
            settings.get_double_escape_action(),
            DoubleEscapeAction::Tree
        );
        assert_eq!(settings.get_tree_filter_mode(), TreeFilterMode::Default);
        for action in [
            DoubleEscapeAction::Fork,
            DoubleEscapeAction::Tree,
            DoubleEscapeAction::None,
        ] {
            settings.set_double_escape_action(action.clone());
            assert_eq!(settings.get_double_escape_action(), action);
        }
        let modes = [
            TreeFilterMode::Default,
            TreeFilterMode::NoTools,
            TreeFilterMode::UserOnly,
            TreeFilterMode::LabeledOnly,
            TreeFilterMode::All,
        ];
        for mode in modes {
            settings.set_tree_filter_mode(mode);
            assert_eq!(settings.get_tree_filter_mode(), mode);
        }
        settings.set_double_escape_action(DoubleEscapeAction::Fork);
        settings.set_tree_filter_mode(TreeFilterMode::LabeledOnly);
        block_on(settings.flush());
        assert_eq!(
            raw_json(&*storage, SettingsScope::Global),
            json!({"doubleEscapeAction": "fork", "treeFilterMode": "labeled-only"})
        );
    }

    /// Unrecognized and wrong-typed stored modes.
    fn assert_stored_modes() {
        let cases = [
            (json!("x"), DoubleEscapeAction::Unknown("x".into())),
            (json!(""), DoubleEscapeAction::Unknown(String::new())),
            (json!(5), DoubleEscapeAction::Tree),
            (json!(null), DoubleEscapeAction::Tree),
        ];
        for (stored, escape) in cases {
            let (settings, _) =
                manager_with(json!({"doubleEscapeAction": stored, "treeFilterMode": stored}));
            assert_eq!(settings.get_double_escape_action(), escape, "{stored}");
            assert_eq!(
                settings.get_tree_filter_mode(),
                TreeFilterMode::Default,
                "{stored}"
            );
        }
    }

    #[test]
    fn presentation_modes_keep_defaults() {
        assert_mode_setters();
        assert_stored_modes();
        let (settings, _) = manager();
        assert_eq!(settings.get_code_block_indent(), "  ");
        for indent in ["", "\t", "\u{feff}\u{85}", "😀"] {
            let (settings, _) = manager_with(json!({"markdown": {"codeBlockIndent": indent}}));
            assert_eq!(settings.get_code_block_indent(), indent);
        }
        let (settings, _) = manager_with(json!({"markdown": {"codeBlockIndent": 4}}));
        assert_eq!(settings.get_code_block_indent(), "  ");
    }

    /// Property type of a stored member.
    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Text,
        Flag,
        Count,
        List,
        Record,
    }

    /// The keys of an object member with their natural types.
    type Members = &'static [(&'static str, Kind)];

    /// A top-level key, its natural type and the keys of its object members.
    type Property = (&'static str, Kind, Members);

    /// Every top-level key.
    const SCHEMA: &[Property] = &[
        ("lastChangelogVersion", Kind::Text, &[]),
        ("defaultProvider", Kind::Text, &[]),
        ("defaultModel", Kind::Text, &[]),
        ("defaultThinkingLevel", Kind::Text, &[]),
        ("transport", Kind::Text, &[]),
        ("steeringMode", Kind::Text, &[]),
        ("followUpMode", Kind::Text, &[]),
        ("theme", Kind::Text, &[]),
        (
            "compaction",
            Kind::Record,
            &[
                ("enabled", Kind::Flag),
                ("reserveTokens", Kind::Count),
                ("keepRecentTokens", Kind::Count),
            ],
        ),
        (
            "branchSummary",
            Kind::Record,
            &[("reserveTokens", Kind::Count), ("skipPrompt", Kind::Flag)],
        ),
        (
            "retry",
            Kind::Record,
            &[
                ("enabled", Kind::Flag),
                ("maxRetries", Kind::Count),
                ("baseDelayMs", Kind::Count),
                ("provider", Kind::Record),
            ],
        ),
        ("hideThinkingBlock", Kind::Flag, &[]),
        ("shellPath", Kind::Text, &[]),
        ("quietStartup", Kind::Flag, &[]),
        ("shellCommandPrefix", Kind::Text, &[]),
        ("npmCommand", Kind::List, &[]),
        ("collapseChangelog", Kind::Flag, &[]),
        ("enableInstallTelemetry", Kind::Flag, &[]),
        ("packages", Kind::List, &[]),
        ("extensions", Kind::List, &[]),
        ("skills", Kind::List, &[]),
        ("prompts", Kind::List, &[]),
        ("themes", Kind::List, &[]),
        ("enableSkillCommands", Kind::Flag, &[]),
        (
            "terminal",
            Kind::Record,
            &[
                ("showImages", Kind::Flag),
                ("imageWidthCells", Kind::Count),
                ("showTerminalProgress", Kind::Flag),
            ],
        ),
        (
            "images",
            Kind::Record,
            &[("autoResize", Kind::Flag), ("blockImages", Kind::Flag)],
        ),
        ("enabledModels", Kind::List, &[]),
        ("doubleEscapeAction", Kind::Text, &[]),
        ("treeFilterMode", Kind::Text, &[]),
        (
            "thinkingBudgets",
            Kind::Record,
            &[
                ("minimal", Kind::Count),
                ("low", Kind::Count),
                ("medium", Kind::Count),
                ("high", Kind::Count),
            ],
        ),
        ("editorPaddingX", Kind::Count, &[]),
        ("autocompleteMaxVisible", Kind::Count, &[]),
        ("markdown", Kind::Record, &[("codeBlockIndent", Kind::Text)]),
        (
            "warnings",
            Kind::Record,
            &[("anthropicExtraUsage", Kind::Flag)],
        ),
        ("sessionDir", Kind::Text, &[]),
    ];

    /// The JSON kind of a probe value.
    fn kind_of(value: &Value) -> Option<Kind> {
        match value {
            Value::String(_) => Some(Kind::Text),
            Value::Bool(_) => Some(Kind::Flag),
            Value::Number(_) => Some(Kind::Count),
            Value::Array(_) => Some(Kind::List),
            Value::Object(_) => Some(Kind::Record),
            Value::Null => None,
        }
    }

    /// The wrong-typed probe value for a member: `probe`, or a different kind when it fits.
    fn wrong_value(natural: Kind, probe: &Value) -> Value {
        if kind_of(probe) != Some(natural) {
            return probe.clone();
        }
        if natural == Kind::Text {
            json!(7)
        } else {
            json!("x")
        }
    }

    /// Every typed getter that does not depend on the environment.
    fn observe(settings: &SettingsManager) -> Value {
        let budgets = settings
            .get_thinking_budgets()
            .map_or([None; 4], |b| [b.minimal, b.low, b.medium, b.high]);
        json!({
            "text": [settings.get_last_changelog_version(), settings.get_default_provider(),
                settings.get_default_model(), settings.get_theme(), settings.get_shell_path(),
                settings.get_shell_command_prefix()],
            "enums": [format!("{:?}", settings.get_default_thinking_level()),
                format!("{:?}", settings.get_transport()), format!("{:?}", settings.get_steering_mode()),
                format!("{:?}", settings.get_follow_up_mode()),
                format!("{:?}", settings.get_double_escape_action()),
                format!("{:?}", settings.get_tree_filter_mode())],
            "flags": [settings.get_compaction_enabled(), settings.get_branch_summary_skip_prompt(),
                settings.get_retry_enabled(), settings.get_hide_thinking_block(),
                settings.get_quiet_startup(), settings.get_collapse_changelog(),
                settings.get_enable_install_telemetry(), settings.get_enable_skill_commands(),
                settings.get_show_images(), settings.get_show_terminal_progress(),
                settings.get_image_auto_resize(), settings.get_block_images()],
            "numbers": [settings.get_compaction_reserve_tokens(), settings.get_compaction_keep_recent_tokens(),
                settings.get_image_width_cells(), settings.get_editor_padding_x(),
                settings.get_autocomplete_max_visible()],
            "records": [format!("{:?}", settings.get_branch_summary_settings()),
                format!("{:?}", settings.get_retry_settings()),
                format!("{:?}", settings.get_provider_retry_settings())],
            "lists": [format!("{:?}", settings.get_npm_command()), format!("{:?}", settings.get_packages()),
                format!("{:?}", settings.get_extension_paths()), format!("{:?}", settings.get_skill_paths()),
                format!("{:?}", settings.get_prompt_template_paths()),
                format!("{:?}", settings.get_theme_paths()), format!("{:?}", settings.get_enabled_models())],
            "others": [settings.get_code_block_indent(), format!("{:?}", settings.get_session_dir()),
                format!("{budgets:?}")],
        })
    }

    /// An object member whose every key holds `probe`, or another wrong type where `probe` fits.
    fn wrong_members(members: Members, probe: &Value) -> Value {
        let wrong =
            |(member, kind): &(&str, Kind)| ((*member).to_owned(), wrong_value(*kind, probe));
        Value::Object(members.iter().map(wrong).collect())
    }

    /// A document whose every member holds `probe`, or another wrong type where `probe` fits.
    fn wrong_document(probe: &Value) -> serde_json::Map<String, Value> {
        let mut seed = serde_json::Map::new();
        for (key, kind, members) in SCHEMA {
            if *key == "skills" && probe.is_object() {
                continue; // the legacy skills object is converted, not kept
            }
            let wrong = if members.is_empty() {
                wrong_value(*kind, probe)
            } else {
                wrong_members(members, probe)
            };
            seed.insert((*key).into(), wrong);
        }
        seed
    }

    #[test]
    fn wrong_type_reads_preserve_raw_data() {
        let (empty, _) = manager();
        let expected = observe(&empty);
        for probe in [
            json!(null),
            json!(true),
            json!(7),
            json!("x"),
            json!([]),
            json!({}),
        ] {
            let seed = wrong_document(&probe);
            let (settings, _) = manager_with(Value::Object(seed.clone()));
            assert_eq!(observe(&settings), expected, "probe {probe}");
            assert_eq!(
                settings.get_global_settings().0,
                seed,
                "the raw document is untouched for {probe}"
            );
        }
        assert_nested_edits_repair_parents();
    }

    /// A nested edit replaces a malformed parent member with an object holding the edit.
    fn assert_nested_edits_repair_parents() {
        type Case = (&'static str, &'static str, fn(&mut SettingsManager), Value);
        let cases: [Case; 4] = [
            (
                "compaction",
                "enabled",
                |m| m.set_compaction_enabled(false),
                json!(false),
            ),
            (
                "retry",
                "enabled",
                |m| m.set_retry_enabled(false),
                json!(false),
            ),
            (
                "terminal",
                "showImages",
                |m| m.set_show_images(false),
                json!(false),
            ),
            (
                "images",
                "autoResize",
                |m| m.set_image_auto_resize(false),
                json!(false),
            ),
        ];
        for (field, key, edit, value) in cases {
            for malformed in [
                json!(null),
                json!(false),
                json!(0),
                json!("bad"),
                json!([]),
                json!(7),
            ] {
                let (mut settings, storage) =
                    manager_with(json!({ field: malformed, "custom": {"keep": 1} }));
                edit(&mut settings);
                block_on(settings.flush());
                let expected = json!({ field: { key: value }, "custom": {"keep": 1} });
                assert_eq!(
                    raw_json(&*storage, SettingsScope::Global),
                    expected,
                    "{field} from {malformed}"
                );
                assert_eq!(Value::Object(settings.get_global_settings().0), expected);
            }
        }
    }

    const CLEAR_ON_SHRINK_ENV: &str = "MAESTRO_CLEAR_ON_SHRINK";
    const HARDWARE_CURSOR_ENV: &str = "MAESTRO_HARDWARE_CURSOR";

    /// Stored forms of an optional boolean preference, by label.
    fn boolean_forms() -> [(&'static str, Option<Value>); 5] {
        [
            ("missing", None),
            ("null", Some(json!(null))),
            ("false", Some(json!(false))),
            ("true", Some(json!(true))),
            ("wrong", Some(json!("wrong"))),
        ]
    }

    /// Child side: reads both environment-dependent preferences for every stored form.
    fn report_environment_matrix() {
        let mut clear = serde_json::Map::new();
        let mut cursor = serde_json::Map::new();
        for (label, form) in boolean_forms() {
            let mut terminal = serde_json::Map::new();
            let mut document = serde_json::Map::new();
            if let Some(form) = form {
                terminal.insert("clearOnShrink".into(), form.clone());
                document.insert("showHardwareCursor".into(), form);
            }
            document.insert("terminal".into(), Value::Object(terminal));
            let (settings, _) = manager_with(Value::Object(document));
            clear.insert(label.into(), json!(settings.get_clear_on_shrink()));
            cursor.insert(label.into(), json!(settings.get_show_hardware_cursor()));
        }
        support::report(&json!({"clear": clear, "cursor": cursor}));
    }

    #[test]
    fn environment_fallback_distinguishes_null() {
        if support::child_case().is_some() {
            return report_environment_matrix();
        }
        for env in [None, Some(""), Some("0"), Some("1"), Some("true")] {
            let mut child =
                support::child_command("environment_fallback_distinguishes_null", "env");
            child
                .env_remove(CLEAR_ON_SHRINK_ENV)
                .env_remove(HARDWARE_CURSOR_ENV);
            if let Some(env) = env {
                child
                    .env(CLEAR_ON_SHRINK_ENV, env)
                    .env(HARDWARE_CURSOR_ENV, env);
            }
            let enabled = env == Some("1");
            let expected = json!({
                "clear": {"missing": enabled, "null": false, "false": false, "true": true, "wrong": enabled},
                "cursor": {"missing": enabled, "null": enabled, "false": false, "true": true, "wrong": enabled},
            });
            assert_eq!(
                support::child_report(&child.output().unwrap()),
                expected,
                "environment {env:?}"
            );
        }
    }

    /// Stored session directories and the paths expected when the home directory is `/home/maestro`.
    const SESSION_DIRS: [(Option<&str>, Option<&str>); 12] = [
        (None, None),
        (Some(""), Some("")),
        (Some("~"), Some("/home/maestro")),
        (Some("~/sessions"), Some("/home/maestro/sessions")),
        (Some("~/"), Some("/home/maestro")),
        (
            Some("~//nested/../sessions/"),
            Some("/home/maestro/sessions/"),
        ),
        (Some("~/../sibling"), Some("/home/sibling")),
        (Some("relative/../literal"), Some("relative/../literal")),
        (Some("~other"), Some("~other")),
        (Some("\\literal"), Some("\\literal")),
        (Some("\u{feff}folder\u{85}"), Some("\u{feff}folder\u{85}")),
        (Some("/abs//path/"), Some("/abs//path/")),
    ];

    /// Child side: resolves every stored session directory, then a project override.
    fn report_session_dirs() {
        let mut paths = Vec::new();
        for (stored, _) in SESSION_DIRS {
            let document = stored.map_or_else(|| json!({}), |stored| json!({"sessionDir": stored}));
            let (settings, _) = manager_with(document);
            paths.push(
                settings
                    .get_session_dir()
                    .map(|path| path.to_string_lossy().into_owned()),
            );
        }
        let (settings, _) = manager_with_both(
            json!({"sessionDir": "/global/sessions"}),
            json!({"sessionDir": "./sessions"}),
        );
        paths.push(
            settings
                .get_session_dir()
                .map(|path| path.to_string_lossy().into_owned()),
        );
        let (settings, _) = manager_with(json!({"sessionDir": null}));
        paths.push(
            settings
                .get_session_dir()
                .map(|path| path.to_string_lossy().into_owned()),
        );
        support::report(&json!(paths));
    }

    /// Child side: the expanded path of `~/x` as hexadecimal bytes.
    #[cfg(unix)]
    fn report_lossless_home() {
        use std::fmt::Write;
        use std::os::unix::ffi::OsStrExt;
        let (settings, _) = manager_with(json!({"sessionDir": "~/x"}));
        let path = settings.get_session_dir().unwrap();
        let hex = path
            .as_os_str()
            .as_bytes()
            .iter()
            .fold(String::new(), |mut hex, byte| {
                let _ = write!(hex, "{byte:02x}");
                hex
            });
        support::report(&json!(hex));
    }

    /// A home directory with an invalid UTF-8 byte must survive expansion byte for byte.
    #[cfg(unix)]
    fn assert_lossless_home() {
        use std::os::unix::ffi::OsStrExt;
        let home = std::ffi::OsStr::from_bytes(b"/h\xffme");
        let mut child = support::child_command("session_dir_expands_only_home_prefix", "lossless");
        child.env("HOME", home);
        assert_eq!(
            support::child_report(&child.output().unwrap()),
            json!("2f68ff6d652f78")
        );
    }

    #[test]
    fn session_dir_expands_only_home_prefix() {
        match support::child_case().as_deref() {
            Some("paths") => return report_session_dirs(),
            #[cfg(unix)]
            Some("lossless") => return report_lossless_home(),
            _ => {}
        }
        #[cfg(unix)]
        assert_lossless_home();
        let mut child = support::child_command("session_dir_expands_only_home_prefix", "paths");
        child.env("HOME", "/home/maestro");
        let mut expected: Vec<Option<&str>> =
            SESSION_DIRS.iter().map(|(_, expected)| *expected).collect();
        expected.extend([Some("./sessions"), None]);
        assert_eq!(
            support::child_report(&child.output().unwrap()),
            json!(expected)
        );
    }

    /// Runs one setter batch against storage seeded with `seed` text and returns the saved text.
    fn saved_text_after(seed: &str, edit: impl FnOnce(&mut SettingsManager)) -> String {
        let storage = support::seeded(Some(seed), None);
        let mut settings = SettingsManager::from_storage(storage.clone());
        edit(&mut settings);
        block_on(settings.flush());
        support::raw(&*storage, SettingsScope::Global).unwrap()
    }

    #[test]
    fn format_uses_two_spaces_without_newline() {
        let seed = r#"{"whole":1.0,"tiny":1e-7,"large":1e21,"mid":1e20,"negative":-0.0,"fraction":0.1,"empty":{},"list":[]}"#;
        let text = saved_text_after(seed, |settings| {
            settings.set_theme("a\"b\\c\n\u{1}é😀".into());
        });
        let expected = [
            "{",
            "  \"whole\": 1,",
            "  \"tiny\": 1e-7,",
            "  \"large\": 1e+21,",
            "  \"mid\": 100000000000000000000,",
            "  \"negative\": 0,",
            "  \"fraction\": 0.1,",
            "  \"empty\": {},",
            "  \"list\": [],",
            "  \"theme\": \"a\\\"b\\\\c\\n\\u0001é😀\"",
            "}",
        ]
        .join("\n");
        assert_eq!(text, expected);

        let text = saved_text_after("", |settings| {
            settings.set_image_width_cells(2.9);
            settings.set_shell_path(None);
        });
        assert_eq!(
            text,
            "{\n  \"terminal\": {\n    \"imageWidthCells\": 2\n  }\n}"
        );
    }

    #[test]
    fn opaque_integers_and_keys_survive_save() {
        let seed = concat!(
            r#"{"10":1,"2":2,"a":1,"huge":9007199254740993,"max":18446744073709551615,"#,
            r#""__proto__":{"theme":"fallback"},"emoji":"😀"}"#
        );
        let storage = support::seeded(Some(seed), None);
        let mut settings = SettingsManager::from_storage(storage.clone());
        assert_eq!(settings.get_theme(), None, "__proto__ is an ordinary key");
        settings.set_theme("light".into());
        block_on(settings.flush());
        let expected = [
            "{",
            "  \"10\": 1,",
            "  \"2\": 2,",
            "  \"a\": 1,",
            "  \"huge\": 9007199254740993,",
            "  \"max\": 18446744073709551615,",
            "  \"__proto__\": {",
            "    \"theme\": \"fallback\"",
            "  },",
            "  \"emoji\": \"😀\",",
            "  \"theme\": \"light\"",
            "}",
        ]
        .join("\n");
        assert_eq!(
            support::raw(&*storage, SettingsScope::Global).unwrap(),
            expected
        );
    }

    #[test]
    fn string_content_survives_without_trimming() {
        for text in [
            "",
            " ",
            "  padded  ",
            "\u{feff}\u{85}",
            "\tTab\n",
            "é😀\\\"",
            "\u{a0}\u{2028}",
        ] {
            let (mut settings, storage) = manager();
            settings.set_theme(text.into());
            settings.set_shell_path(Some(text.into()));
            settings.set_shell_command_prefix(Some(text.into()));
            assert_eq!(settings.get_theme().as_deref(), Some(text));
            block_on(settings.flush());
            block_on(settings.reload());
            assert_eq!(settings.get_theme().as_deref(), Some(text));
            assert_eq!(settings.get_shell_path().as_deref(), Some(text));
            assert_eq!(settings.get_shell_command_prefix().as_deref(), Some(text));
            assert_eq!(raw_json(&*storage, SettingsScope::Global)["theme"], text);
        }
    }

    #[test]
    fn warnings_and_budgets_keep_optional_fields() {
        let (settings, _) = manager();
        assert_eq!(settings.get_thinking_budgets(), None);
        assert_eq!(settings.get_warnings(), WarningSettings::default());

        let (settings, _) =
            manager_with(json!({"thinkingBudgets": {"low": 100, "high": "many", "custom": 1}}));
        let budgets = settings.get_thinking_budgets().unwrap();
        assert_eq!(
            (budgets.minimal, budgets.low, budgets.medium, budgets.high),
            (None, Some(100.0), None, None)
        );
        assert_eq!(budgets.extra.get("high"), Some(&json!("many")));
        assert_eq!(budgets.extra.get("custom"), Some(&json!(1)));
        let (settings, _) = manager_with(json!({"thinkingBudgets": "x"}));
        assert_eq!(settings.get_thinking_budgets(), None);

        let seed = json!({"warnings": {"anthropicExtraUsage": "bad", "extra": 7}});
        let (mut settings, storage) = manager_with(seed.clone());
        let mut warnings = settings.get_warnings();
        assert_eq!(warnings.anthropic_extra_usage, None);
        assert_eq!(
            warnings.extra.get("anthropicExtraUsage"),
            Some(&json!("bad"))
        );
        settings.set_warnings(warnings.clone());
        block_on(settings.flush());
        assert_eq!(raw_json(&*storage, SettingsScope::Global), seed);

        warnings.anthropic_extra_usage = Some(false);
        settings.set_warnings(warnings);
        block_on(settings.flush());
        assert_eq!(
            raw_json(&*storage, SettingsScope::Global),
            json!({"warnings": {"anthropicExtraUsage": false, "extra": 7}})
        );
        assert_eq!(settings.get_warnings().anthropic_extra_usage, Some(false));
    }

    /// Deserializes a sparse record and serializes it again.
    fn json_round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(stored: &Value) -> Value {
        let record: T = serde_json::from_value(stored.clone()).unwrap();
        serde_json::to_value(record).unwrap()
    }

    #[test]
    fn partial_records_do_not_materialize_defaults() {
        let (settings, storage) = manager();
        let _ = (
            settings.get_compaction_settings(),
            settings.get_retry_settings(),
            settings.get_warnings(),
        );
        block_on(settings.flush());
        assert_eq!(
            support::raw(&*storage, SettingsScope::Global),
            None,
            "reads write nothing"
        );

        let (mut settings, storage) = manager_with(json!({"compaction": {"enabled": false}}));
        support::assert_number(settings.get_compaction_settings().reserve_tokens, 16384.0);
        settings.set_theme("x".into());
        block_on(settings.flush());
        assert_eq!(
            raw_json(&*storage, SettingsScope::Global),
            json!({"compaction": {"enabled": false}, "theme": "x"})
        );

        let stored = [
            json!({"enabled": false, "keepRecentTokens": 1.5, "x": 1}),
            json!({"skipPrompt": true}),
            json!({"timeoutMs": 9, "maxRetries": "bad"}),
            json!({"provider": {"maxRetryDelayMs": 5}, "baseDelayMs": 2}),
            json!({"imageWidthCells": 7, "clearOnShrink": null}),
            json!({"autoResize": false}),
            json!({"minimal": 1, "high": 4}),
            json!({"codeBlockIndent": "\t"}),
            json!({"anthropicExtraUsage": true}),
        ];
        let sparse = [
            json_round_trip::<CompactionSettings>(&stored[0]),
            json_round_trip::<BranchSummarySettings>(&stored[1]),
            json_round_trip::<ProviderRetrySettings>(&stored[2]),
            json_round_trip::<RetrySettings>(&stored[3]),
            json_round_trip::<TerminalSettings>(&stored[4]),
            json_round_trip::<ImageSettings>(&stored[5]),
            json_round_trip::<ThinkingBudgetsSettings>(&stored[6]),
            json_round_trip::<MarkdownSettings>(&stored[7]),
            json_round_trip::<WarningSettings>(&stored[8]),
        ];
        assert_eq!(sparse, stored);
        assert_eq!(
            serde_json::to_value(CompactionSettings::default()).unwrap(),
            json!({})
        );
        let overlay = CompactionSettings {
            enabled: Some(true),
            extra: serde_json::from_value(json!({"enabled": "stale", "keep": 1})).unwrap(),
            ..CompactionSettings::default()
        };
        assert_eq!(
            serde_json::to_value(overlay).unwrap(),
            json!({"enabled": true, "keep": 1})
        );
    }
}
