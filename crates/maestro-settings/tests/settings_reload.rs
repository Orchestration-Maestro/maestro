//! Reload, external edits, error draining, ordered persistence and recovery.

#[cfg(test)]
mod support;

#[cfg(test)]
mod tests {
    use super::support;
    use super::support::{Backend, ControlledStorage, block_on, settings};
    use maestro_settings::{SettingsListEntry, SettingsManager, SettingsScope, ThinkingLevel};
    use serde_json::{Value, json};

    const GLOBAL: SettingsScope = SettingsScope::Global;
    const PROJECT: SettingsScope = SettingsScope::Project;

    fn entries(items: &[&str]) -> Vec<SettingsListEntry> {
        items
            .iter()
            .map(|item| SettingsListEntry::String((*item).into()))
            .collect()
    }

    /// Seeds a backend and loads a manager over it.
    fn loaded(
        backend: &Backend,
        global: Option<&Value>,
        project: Option<&Value>,
    ) -> SettingsManager {
        for (scope, document) in [(GLOBAL, global), (PROJECT, project)] {
            if let Some(document) = document {
                backend.write(scope, &document.to_string());
            }
        }
        SettingsManager::from_storage(backend.handle())
    }

    #[test]
    fn external_models_survive_thinking_save() {
        for backend in Backend::all() {
            let mut manager = loaded(&backend, Some(&json!({"theme": "dark"})), None);
            backend.write(GLOBAL, r#"{"theme":"dark","enabledModels":["a","b"]}"#);
            manager.set_default_thinking_level(ThinkingLevel::High);
            block_on(manager.flush());
            assert_eq!(
                backend.json(GLOBAL),
                json!({"theme": "dark", "enabledModels": ["a", "b"], "defaultThinkingLevel": "high"})
            );
            assert_eq!(
                manager.get_enabled_models(),
                None,
                "external data enters the cache only on reload"
            );
            block_on(manager.reload());
            assert_eq!(manager.get_enabled_models(), Some(entries(&["a", "b"])));
        }
    }

    #[test]
    fn external_custom_fields_survive_theme_save() {
        for backend in Backend::all() {
            let mut manager = loaded(&backend, Some(&json!({"theme": "dark"})), None);
            let external = json!({"theme": "dark", "shellPath": "/bin/zsh", "extensions": ["/e"], "mine": {"deep": [1, 2]}});
            backend.write(GLOBAL, &external.to_string());
            manager.set_theme("light".into());
            block_on(manager.flush());
            let mut expected = external;
            expected["theme"] = json!("light");
            assert_eq!(backend.json(GLOBAL), expected);
        }
    }

    #[test]
    fn local_global_edit_wins() {
        for backend in Backend::all() {
            let mut manager = loaded(&backend, Some(&json!({"theme": "dark"})), None);
            backend.write(GLOBAL, r#"{"theme":"external","other":1}"#);
            manager.set_theme("local".into());
            block_on(manager.flush());
            assert_eq!(backend.json(GLOBAL), json!({"theme": "local", "other": 1}));
        }
    }

    /// Stored shape of an external package list change.
    fn external_resources() -> Value {
        json!({"extensions": ["/new"], "prompts": ["/kept"]})
    }

    #[test]
    fn external_resource_changes_survive_unrelated_saves() {
        for backend in Backend::all() {
            let seed = json!({"packages": ["npm:old"], "extensions": ["/old"], "theme": "dark"});
            let mut manager = loaded(&backend, Some(&seed), None);
            let mut external = external_resources();
            external["theme"] = json!("dark");
            backend.write(GLOBAL, &external.to_string());
            manager.set_theme("light".into());
            manager.set_quiet_startup(true);
            block_on(manager.flush());
            let saved = backend.json(GLOBAL);
            assert_eq!(
                saved.get("packages"),
                None,
                "an externally removed array stays removed"
            );
            assert_eq!(saved["extensions"], json!(["/new"]));
            assert_eq!(saved["prompts"], json!(["/kept"]));
            assert_eq!(saved["theme"], "light");
            assert_eq!(
                manager.get_extension_paths(),
                entries(&["/old"]),
                "the cache is unchanged"
            );
        }
    }

    #[test]
    fn project_external_changes_survive() {
        for backend in Backend::all() {
            let mut manager = loaded(
                &backend,
                None,
                Some(&json!({"extensions": ["a"], "prompts": ["old"]})),
            );
            let external = json!({
                "extensions": ["a"], "prompts": ["new"],
                "packages": [{"source": "git:x", "extensions": ["*.ts"], "opaque": {"k": 1}}]
            });
            backend.write(PROJECT, &external.to_string());
            manager.set_project_extension_paths(entries(&["b"]));
            block_on(manager.flush());
            let mut expected = external;
            expected["extensions"] = json!(["b"]);
            assert_eq!(backend.json(PROJECT), expected);
        }
    }

    #[test]
    fn project_local_edit_wins() {
        for backend in Backend::all() {
            let mut manager = loaded(&backend, None, Some(&json!({"extensions": ["a"]})));
            backend.write(PROJECT, r#"{"extensions":["external"],"skills":["s"]}"#);
            manager.set_project_extension_paths(entries(&["local"]));
            block_on(manager.flush());
            assert_eq!(
                backend.json(PROJECT),
                json!({"extensions": ["local"], "skills": ["s"]})
            );
        }
    }

    #[test]
    fn reload_accepts_new_global_data() {
        let storage = ControlledStorage::new();
        let backend = Backend::Controlled(storage.clone());
        let seed = json!({"theme": "dark", "defaultModel": "m1", "extensions": ["a"], "packages": ["npm:p"]});
        let mut manager = loaded(&backend, Some(&seed), Some(&json!({"quietStartup": true})));
        backend.write(
            GLOBAL,
            r#"{"theme":"light","defaultModel":"m2","extensions":[]}"#,
        );
        assert_eq!(manager.get_theme().as_deref(), Some("dark"));
        block_on(manager.reload());
        assert_eq!(manager.get_theme().as_deref(), Some("light"));
        assert_eq!(manager.get_default_model().as_deref(), Some("m2"));
        assert_eq!(manager.get_extension_paths(), Vec::new());
        assert_eq!(
            manager.get_packages(),
            Vec::new(),
            "a deleted array is gone after reload"
        );
        assert!(manager.get_quiet_startup(), "the project scope reloads too");
        assert_eq!(
            storage.calls(),
            [GLOBAL, PROJECT, GLOBAL, PROJECT],
            "global loads before project"
        );
    }

    #[test]
    fn reload_keeps_failed_scope_and_accepts_healthy() {
        let storage = ControlledStorage::new();
        let backend = Backend::Controlled(storage.clone());
        let mut manager = loaded(
            &backend,
            Some(&json!({"theme": "dark"})),
            Some(&json!({"quietStartup": true})),
        );
        assert!(manager.drain_errors().is_empty());

        backend.write(GLOBAL, "{broken");
        backend.write(
            PROJECT,
            r#"{"quietStartup":false,"hideThinkingBlock":true}"#,
        );
        block_on(manager.reload());
        assert_eq!(
            manager.get_theme().as_deref(),
            Some("dark"),
            "the failed scope keeps its accepted data"
        );
        assert!(
            manager.get_hide_thinking_block(),
            "the healthy scope still updates"
        );
        assert!(!manager.get_quiet_startup());
        let errors = manager.drain_errors();
        assert_eq!(
            errors.iter().map(|error| error.scope).collect::<Vec<_>>(),
            [GLOBAL]
        );

        backend.write(GLOBAL, r#"{"theme":"light"}"#);
        backend.write(PROJECT, "[]");
        block_on(manager.reload());
        assert_eq!(manager.get_theme().as_deref(), Some("light"));
        assert!(
            manager.get_hide_thinking_block(),
            "a failed project scope keeps its accepted data"
        );
        let errors = manager.drain_errors();
        assert_eq!(
            errors.iter().map(|error| error.scope).collect::<Vec<_>>(),
            [PROJECT]
        );

        storage.fail_reads(true);
        block_on(manager.reload());
        let errors = manager.drain_errors();
        assert_eq!(
            errors.iter().map(|error| error.scope).collect::<Vec<_>>(),
            [GLOBAL, PROJECT]
        );
        assert!(
            errors
                .iter()
                .all(|error| error.error.to_string() == "controlled failure")
        );
    }

    #[test]
    fn errors_drain_once_without_unlocking_saves() {
        let storage = ControlledStorage::new();
        let backend = Backend::Controlled(storage.clone());
        backend.write(GLOBAL, "{broken");
        backend.write(PROJECT, "not json");
        let mut manager = SettingsManager::from_storage(backend.handle());
        let errors = manager.drain_errors();
        assert_eq!(
            errors.iter().map(|error| error.scope).collect::<Vec<_>>(),
            [GLOBAL, PROJECT]
        );
        assert!(manager.drain_errors().is_empty(), "errors drain once");

        manager.set_theme("memory only".into());
        manager.set_project_extension_paths(entries(&["p"]));
        block_on(manager.flush());
        assert_eq!(
            manager.get_theme().as_deref(),
            Some("memory only"),
            "edits are accepted in memory"
        );
        assert!(
            storage.writes().is_empty(),
            "a failed-load scope is never written"
        );
        assert_eq!(storage.text(GLOBAL).as_deref(), Some("{broken"));

        backend.write(PROJECT, "{}");
        block_on(manager.reload());
        manager.drain_errors();
        manager.set_project_extension_paths(entries(&["p"]));
        block_on(manager.flush());
        assert_eq!(
            backend.json(PROJECT),
            json!({"extensions": ["p"]}),
            "a healthy reload unblocks the scope"
        );
        assert_eq!(
            storage.text(GLOBAL).as_deref(),
            Some("{broken"),
            "the other scope stays blocked"
        );

        backend.write(GLOBAL, "{}");
        block_on(manager.reload());
        manager.set_theme("saved".into());
        block_on(manager.flush());
        assert_eq!(backend.json(GLOBAL), json!({"theme": "saved"}));
    }

    #[test]
    fn in_memory_seed_survives_reload() {
        let seed = settings(json!({
            "defaultThinkingLevel": "high",
            "images": {"autoResize": false},
            "compaction": {"enabled": false}
        }));
        let mut manager = SettingsManager::in_memory(seed.clone());
        assert_eq!(manager.get_global_settings(), seed);
        block_on(manager.reload());
        assert_eq!(manager.get_global_settings(), seed);
        assert_eq!(
            manager.get_default_thinking_level(),
            Some(ThinkingLevel::High)
        );
        assert!(!manager.get_image_auto_resize());
        assert!(!manager.get_compaction_enabled());
        assert!(manager.drain_errors().is_empty());
    }

    #[test]
    fn unrelated_save_keeps_seed() {
        let seed = settings(json!({
            "images": {"autoResize": false},
            "compaction": {"enabled": false},
            "queueMode": "all"
        }));
        let mut manager = SettingsManager::in_memory(seed);
        let converted = manager.get_global_settings();
        assert_eq!(
            converted.0.get("steeringMode"),
            Some(&json!("all")),
            "the seed is converted"
        );
        assert_eq!(converted.0.get("queueMode"), None);
        manager.set_theme("dark".into());
        block_on(manager.flush());
        block_on(manager.reload());
        assert_eq!(manager.get_theme().as_deref(), Some("dark"));
        assert!(!manager.get_image_auto_resize());
        assert!(!manager.get_compaction_enabled());
        assert_eq!(manager.get_global_settings().0["steeringMode"], "all");
    }

    #[test]
    fn fresh_write_applies_conversions() {
        for backend in Backend::all() {
            let mut manager = loaded(&backend, Some(&json!({})), None);
            let legacy = json!({
                "queueMode": "all", "websockets": true,
                "skills": {"enableSkillCommands": false, "customDirectories": ["a"]},
                "retry": {"maxDelayMs": 9, "enabled": false}, "untouched": 1
            });
            backend.write(GLOBAL, &legacy.to_string());
            manager.set_theme("dark".into());
            block_on(manager.flush());
            assert_eq!(
                backend.json(GLOBAL),
                json!({
                    "steeringMode": "all", "transport": "websocket",
                    "skills": ["a"], "enableSkillCommands": false,
                    "retry": {"provider": {"maxRetryDelayMs": 9}, "enabled": false},
                    "untouched": 1, "theme": "dark"
                })
            );
            assert_eq!(
                manager.get_transport().as_str(),
                "auto",
                "disk-only values do not enter the cache"
            );
            assert_eq!(manager.get_global_settings().0.get("untouched"), None);
        }
    }

    /// Stored texts that load, with the accepted document they produce.
    const LOADABLE: [(Option<&str>, &str); 3] = [
        (None, "{}"),
        (Some(""), "{}"),
        (
            Some(r#"{"x":{"y":[1,2]},"unknown":null}"#),
            r#"{"x":{"y":[1,2]},"unknown":null}"#,
        ),
    ];

    /// Stored texts that fail to load.
    const UNLOADABLE: [&str; 11] = [
        " ",
        "{bad",
        "[]",
        "[1]",
        "1",
        "null",
        "\"s\"",
        "true",
        "{\"n\":1e999}",
        "{\"s\":\"\\ud800\"}",
        "{\"a\":1,}",
    ];

    #[test]
    fn missing_empty_invalid_and_nonobject_loads() {
        for (text, expected) in LOADABLE {
            let storage = support::seeded(text, text);
            let mut manager = SettingsManager::from_storage(storage);
            assert_eq!(
                Value::Object(manager.get_global_settings().0),
                serde_json::from_str::<Value>(expected).unwrap()
            );
            assert!(manager.drain_errors().is_empty(), "{text:?} loads");
        }
        for text in UNLOADABLE {
            let storage = support::seeded(Some(text), None);
            let mut manager = SettingsManager::from_storage(storage.clone());
            let errors = manager.drain_errors();
            assert_eq!(
                errors.iter().map(|error| error.scope).collect::<Vec<_>>(),
                [GLOBAL],
                "{text:?}"
            );
            manager.set_theme("kept in memory".into());
            block_on(manager.flush());
            assert_eq!(
                support::raw(&*storage, GLOBAL).as_deref(),
                Some(text),
                "{text:?} is never overwritten"
            );
        }
        let storage = ControlledStorage::new();
        storage.fail_reads(true);
        let mut manager = SettingsManager::from_storage(storage);
        assert_eq!(
            manager.drain_errors().len(),
            2,
            "a read failure is a scoped load error"
        );
    }

    #[test]
    fn paired_model_selection_saves_once() {
        let storage = ControlledStorage::new();
        let mut manager = SettingsManager::from_storage(storage.clone());
        manager.set_default_model_and_provider("local".into(), "model-1".into());
        assert_eq!(manager.get_default_provider().as_deref(), Some("local"));
        assert_eq!(manager.get_default_model().as_deref(), Some("model-1"));
        block_on(manager.flush());
        let writes = storage.writes();
        assert_eq!(writes.len(), 1, "one storage update for the pair");
        assert_eq!(
            serde_json::from_str::<Value>(&writes[0].1).unwrap(),
            json!({"defaultProvider": "local", "defaultModel": "model-1"})
        );
    }

    #[test]
    fn writes_follow_one_cross_scope_queue() {
        let storage = ControlledStorage::new();
        let mut manager = SettingsManager::from_storage(storage.clone());
        storage.hold();
        manager.set_theme("a".into());
        storage.wait_blocked();
        manager.set_project_extension_paths(entries(&["p"]));
        manager.set_default_model("m".into());
        assert_eq!(
            manager.get_theme().as_deref(),
            Some("a"),
            "memory is published before any write ends"
        );
        assert_eq!(manager.get_default_model().as_deref(), Some("m"));
        assert_eq!(manager.get_extension_paths(), entries(&["p"]));
        assert!(storage.writes().is_empty());

        storage.release();
        storage.wait_for_writes(3);
        let writes: Vec<_> = storage
            .writes()
            .into_iter()
            .map(|(scope, text)| (scope, serde_json::from_str::<Value>(&text).unwrap()))
            .collect();
        assert_eq!(
            writes,
            [
                (GLOBAL, json!({"theme": "a"})),
                (PROJECT, json!({"extensions": ["p"]})),
                (GLOBAL, json!({"theme": "a", "defaultModel": "m"})),
            ],
            "first-in first-out across both scopes, each save from its own snapshot"
        );
    }

    /// Three edits: the first and second are queued together, the third after the second fails.
    struct Recovery {
        scope: SettingsScope,
        edits: [fn(&mut SettingsManager); 3],
        expected: Value,
    }

    fn run_recovery(recovery: &Recovery) {
        let storage = ControlledStorage::new();
        let mut manager = SettingsManager::from_storage(storage.clone());
        storage.script_writes(&[true, false, true]);
        storage.hold();
        (recovery.edits[0])(&mut manager);
        (recovery.edits[1])(&mut manager);
        storage.release();
        block_on(manager.flush());
        let errors = manager.drain_errors();
        assert_eq!(
            errors.iter().map(|error| error.scope).collect::<Vec<_>>(),
            [recovery.scope]
        );
        assert_eq!(errors[0].error.to_string(), "controlled failure");
        (recovery.edits[2])(&mut manager);
        block_on(manager.flush());
        let saved: Value = serde_json::from_str(&storage.text(recovery.scope).unwrap()).unwrap();
        assert_eq!(saved, recovery.expected);
    }

    #[test]
    fn successful_ack_keeps_later_failed_revisions() {
        let recoveries = [
            Recovery {
                scope: GLOBAL,
                edits: [
                    |m| m.set_theme("t".into()),
                    |m| m.set_default_model("m".into()),
                    |m| m.set_quiet_startup(true),
                ],
                expected: json!({"theme": "t", "defaultModel": "m", "quietStartup": true}),
            },
            Recovery {
                scope: PROJECT,
                edits: [
                    |m| m.set_project_extension_paths(entries(&["e"])),
                    |m| m.set_project_prompt_template_paths(entries(&["p"])),
                    |m| m.set_project_skill_paths(entries(&["s"])),
                ],
                expected: json!({"extensions": ["e"], "prompts": ["p"], "skills": ["s"]}),
            },
            Recovery {
                scope: GLOBAL,
                edits: [
                    |m| m.set_show_images(false),
                    |m| m.set_show_terminal_progress(true),
                    |m| m.set_quiet_startup(true),
                ],
                expected: json!({"terminal": {"showImages": false, "showTerminalProgress": true}, "quietStartup": true}),
            },
            Recovery {
                scope: GLOBAL,
                edits: [
                    |m| m.set_retry_enabled(false),
                    |m| m.set_retry_enabled(true),
                    |m| m.set_quiet_startup(true),
                ],
                expected: json!({"retry": {"enabled": true}, "quietStartup": true}),
            },
        ];
        for recovery in &recoveries {
            run_recovery(recovery);
        }
    }

    #[test]
    fn external_edit_to_acknowledged_field_survives() {
        for backend in Backend::all() {
            let mut manager = loaded(&backend, Some(&json!({})), None);
            manager.set_theme("a".into());
            block_on(manager.flush());
            backend.write(GLOBAL, r#"{"theme":"external"}"#);
            manager.set_quiet_startup(true);
            block_on(manager.flush());
            assert_eq!(
                backend.json(GLOBAL),
                json!({"theme": "external", "quietStartup": true})
            );
        }
    }

    #[test]
    fn malformed_fresh_disk_fails_without_stopping_queue() {
        let storage = ControlledStorage::new();
        let mut manager = SettingsManager::from_storage(storage.clone());
        storage.inject(GLOBAL, "{broken");
        manager.set_theme("a".into());
        block_on(manager.flush());
        let errors = manager.drain_errors();
        assert_eq!(
            errors.iter().map(|error| error.scope).collect::<Vec<_>>(),
            [GLOBAL]
        );
        assert_eq!(
            manager.get_theme().as_deref(),
            Some("a"),
            "accepted memory is kept"
        );
        assert_eq!(storage.text(GLOBAL).as_deref(), Some("{broken"));

        storage.fail_reads(true);
        manager.set_default_model("m".into());
        block_on(manager.flush());
        assert_eq!(
            manager.drain_errors().len(),
            1,
            "a storage read failure is a scoped error"
        );
        storage.fail_reads(false);
        storage.script_writes(&[false]);
        manager.set_project_extension_paths(entries(&["p"]));
        block_on(manager.flush());
        assert_eq!(
            manager.drain_errors()[0].scope,
            PROJECT,
            "a write failure is a scoped error"
        );

        storage.inject(GLOBAL, "{}");
        manager.set_quiet_startup(true);
        manager.set_project_skill_paths(entries(&["s"]));
        block_on(manager.flush());
        assert!(
            manager.drain_errors().is_empty(),
            "later work runs once storage recovers"
        );
        let global: Value = serde_json::from_str(&storage.text(GLOBAL).unwrap()).unwrap();
        assert_eq!(
            global,
            json!({"theme": "a", "defaultModel": "m", "quietStartup": true})
        );
        let project: Value = serde_json::from_str(&storage.text(PROJECT).unwrap()).unwrap();
        assert_eq!(project, json!({"extensions": ["p"], "skills": ["s"]}));
    }

    #[test]
    fn reload_waits_and_drops_overrides_and_dirty() {
        let storage = ControlledStorage::new();
        let mut manager = SettingsManager::from_storage(storage.clone());
        storage.hold();
        manager.set_theme("a".into());
        manager.apply_overrides(settings(json!({"quietStartup": true})));
        assert!(manager.get_quiet_startup());
        std::thread::scope(|scope| {
            scope.spawn(|| {
                storage.wait_blocked();
                storage.release();
            });
            block_on(manager.reload());
        });
        assert_eq!(
            manager.get_theme().as_deref(),
            Some("a"),
            "reload waited for the queued write"
        );
        assert!(
            !manager.get_quiet_startup(),
            "reload drops runtime overrides"
        );

        storage.script_writes(&[false]);
        manager.set_theme("lost".into());
        block_on(manager.flush());
        manager.drain_errors();
        storage.inject(GLOBAL, r#"{"theme":"external"}"#);
        storage.inject(PROJECT, "{broken");
        block_on(manager.reload());
        assert_eq!(manager.get_theme().as_deref(), Some("external"));
        assert_eq!(
            manager.drain_errors().len(),
            1,
            "the failed project scope is reported"
        );
        manager.set_quiet_startup(true);
        block_on(manager.flush());
        let saved: Value = serde_json::from_str(&storage.text(GLOBAL).unwrap()).unwrap();
        assert_eq!(
            saved,
            json!({"theme": "external", "quietStartup": true}),
            "discarded edits do not return"
        );
    }

    /// Child side: a manager that cannot start its write thread.
    #[cfg(unix)]
    fn run_without_threads() {
        if std::thread::Builder::new().spawn(|| ()).is_ok() {
            return support::report(&json!("limit not enforced"));
        }
        let mut manager = SettingsManager::in_memory(settings(json!({})));
        manager.set_theme("kept".into());
        let errors = manager.drain_errors();
        let scopes: Vec<_> = errors
            .iter()
            .map(|error| format!("{:?}", error.scope))
            .collect();
        support::report(&json!({"scopes": scopes, "theme": manager.get_theme()}));
    }

    #[test]
    fn flush_observes_without_cancelling_dropped_waiter() {
        #[cfg(unix)]
        if support::child_case().is_some() {
            return run_without_threads();
        }
        let storage = ControlledStorage::new();
        let mut manager = SettingsManager::from_storage(storage.clone());
        block_on(manager.flush());
        block_on(manager.flush());

        storage.hold();
        manager.set_theme("a".into());
        drop(manager.flush());
        storage.release();
        storage.wait_for_writes(1);

        storage.hold();
        manager.set_default_model("m".into());
        storage.wait_blocked();
        let barrier = manager.flush();
        manager.set_quiet_startup(true);
        storage.release();
        block_on(barrier);
        assert!(
            storage.writes().len() >= 2,
            "the flush waited for the work queued before it"
        );
        block_on(manager.flush());
        assert_eq!(storage.writes().len(), 3, "later work also completes");

        #[cfg(unix)]
        {
            let mut limited = support::shell_child_command(
                "flush_observes_without_cancelling_dropped_waiter",
                "limit",
                "ulimit -u 1 2>/dev/null",
            );
            let report = support::child_report(&limited.output().unwrap());
            if report != json!("limit not enforced") {
                assert_eq!(
                    report,
                    json!({"scopes": ["Global"], "theme": "kept"}),
                    "a worker start failure is a scoped error"
                );
            }
        }
    }
}
