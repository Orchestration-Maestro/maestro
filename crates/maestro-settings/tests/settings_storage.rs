//! Raw storage contract shared by the memory, native-file and controlled adapters, and
//! the native file locking.

#[cfg(test)]
mod tests {
    use crate::support;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    use serde_json::json;

    use crate::support::{
        ControlledStorage, Holder, Scratch, block_on, file_storage, location, probe_lock, raw,
    };
    use maestro_settings::{
        FileSettingsStorage, InMemorySettingsStorage, PackageSource, SettingsManager,
        SettingsScope, SettingsStorage, SettingsStorageError,
    };

    const BOTH: [SettingsScope; 2] = [SettingsScope::Global, SettingsScope::Project];

    #[test]
    fn checked_child_rejects_missing_identity() {
        assert!(
            support::child_command("settings_storage::tests::missing_child_identity", "missing")
                .output()
                .is_err(),
            "stale selection passed with zero tests"
        );
    }

    /// Handles the selected environment control and intentional failure bodies.
    fn report_checked_environment_child() -> bool {
        match support::child_case().as_deref() {
            Some("control") => {
                let cwd = std::env::current_dir().unwrap();
                let expected =
                    std::path::PathBuf::from(std::env::var_os("MAESTRO_EXPECT_CWD").unwrap());
                assert_eq!(
                    cwd.canonicalize().unwrap(),
                    expected.canonicalize().unwrap()
                );
                assert_eq!(
                    std::env::var_os("LLVM_PROFILE_FILE").unwrap(),
                    expected.join("literal-%p-%m.profraw")
                );
                assert_eq!(std::env::var("MAESTRO_ORDER").unwrap(), "replacement");
                assert!(std::env::var_os("MAESTRO_REMOVE").is_none());
                support::report(&json!({"selected": true, "value": "replacement"}));
                true
            }
            Some("failure" | "probe") => {
                support::report(&json!("free"));
                panic!("report does not establish successful completion");
            }
            Some("seed") | None => false,
            Some(_) => panic!("unexpected child case"),
        }
    }

    #[test]
    fn checked_child_preserves_environment_reports_and_flags() {
        const NAME: &str =
            "settings_storage::tests::checked_child_preserves_environment_reports_and_flags";
        if report_checked_environment_child() {
            return;
        }
        if support::child_case().is_none() {
            let before: std::collections::BTreeMap<_, _> = std::env::vars_os().collect();
            let output = support::child_command(NAME, "seed")
                .env("MAESTRO_REMOVE", "inherited")
                .env("MAESTRO_ORDER", "original")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                before == std::env::vars_os().collect(),
                "parent environment changed"
            );
            return;
        }
        assert_eq!(std::env::var("MAESTRO_REMOVE").unwrap(), "inherited");
        let before: std::collections::BTreeMap<_, _> = std::env::vars_os().collect();
        let root = Scratch::new().unwrap();
        let output = support::child_command(NAME, "control")
            .current_dir(root.path())
            .env("MAESTRO_EXPECT_CWD", root.path())
            .env(
                "LLVM_PROFILE_FILE",
                root.path().join("literal-%p-%m.profraw"),
            )
            .env_remove("MAESTRO_ORDER")
            .env("MAESTRO_ORDER", "replacement")
            .env_remove("MAESTRO_REMOVE")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            support::child_report(&output),
            json!({"selected": true, "value": "replacement"})
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.starts_with("\nrunning 1 test\n"));
        assert!(text.lines().any(|line| line == "."));
        assert!(!text.contains("test settings_storage::"));
        assert!(output.stderr.is_empty());
        let failed = support::child_command(NAME, "failure").output().unwrap();
        assert!(!failed.status.success());
        assert_eq!(support::child_report(&failed), json!("free"));
        assert!(
            std::panic::catch_unwind(|| support::probe_lock(NAME, &root.path().join("lock")))
                .is_err(),
            "failed child report was accepted"
        );
        assert!(
            before == std::env::vars_os().collect(),
            "parent environment changed"
        );
    }

    /// Runs `update` against one scope and returns the text it saw.
    fn transact(
        storage: &dyn SettingsStorage,
        scope: SettingsScope,
        replacement: Result<Option<&str>, &str>,
    ) -> (Option<String>, Result<(), SettingsStorageError>) {
        let mut seen = None;
        let outcome = storage.with_lock(scope, &mut |current| {
            seen = current.map(str::to_owned);
            match replacement {
                Ok(text) => Ok(text.map(str::to_owned)),
                Err(message) => Err(message.into()),
            }
        });
        (seen, outcome)
    }

    /// The contract every adapter honors: one callback, optional replacement, independent scopes.
    fn assert_raw_transaction_contract(storage: &dyn SettingsStorage) {
        for scope in BOTH {
            let (seen, outcome) = transact(storage, scope, Ok(None));
            assert_eq!(
                (seen, outcome.is_ok()),
                (None, true),
                "{scope:?} starts unwritten"
            );
        }
        transact(storage, SettingsScope::Global, Ok(Some("global\n")))
            .1
            .unwrap();
        assert_eq!(
            raw(storage, SettingsScope::Global).as_deref(),
            Some("global\n")
        );
        assert_eq!(
            raw(storage, SettingsScope::Project),
            None,
            "scopes are independent"
        );

        transact(storage, SettingsScope::Project, Ok(Some("")))
            .1
            .unwrap();
        assert_eq!(
            raw(storage, SettingsScope::Project).as_deref(),
            Some(""),
            "empty text is a write"
        );

        let (_, kept) = transact(storage, SettingsScope::Global, Ok(None));
        kept.unwrap();
        assert_eq!(
            raw(storage, SettingsScope::Global).as_deref(),
            Some("global\n"),
            "no replacement keeps bytes"
        );

        let (seen, failed) = transact(storage, SettingsScope::Global, Err("boom"));
        assert_eq!(seen.as_deref(), Some("global\n"));
        assert_eq!(failed.unwrap_err().to_string(), "boom");
        assert_eq!(
            raw(storage, SettingsScope::Global).as_deref(),
            Some("global\n"),
            "a failed callback writes nothing"
        );
    }

    #[test]
    fn adapters_share_raw_transaction_contract() {
        assert_raw_transaction_contract(&InMemorySettingsStorage::new());
        assert_raw_transaction_contract(&*ControlledStorage::new());
        let root = Scratch::new().unwrap();
        assert_raw_transaction_contract(&file_storage(root.path()));
        assert_eq!(
            fs::read_to_string(root.path().join("agent/settings.json")).unwrap(),
            "global\n"
        );
        assert_eq!(
            fs::read_to_string(root.path().join("work/.maestro/settings.json")).unwrap(),
            ""
        );
    }

    /// Writes `"inner"` to `scope` and returns the text the callback saw there.
    fn write_inner(storage: &InMemorySettingsStorage, scope: SettingsScope) -> Option<String> {
        let mut seen = None;
        storage
            .with_lock(scope, &mut |current| {
                seen = current.map(str::to_owned);
                Ok(Some("inner".to_owned()))
            })
            .unwrap();
        seen
    }

    /// Runs a callback on `outer` that re-enters `inner` through [`write_inner`], then
    /// returns `outer_text`; yields the text the inner callback saw and both stored scopes.
    fn reenter(
        outer: SettingsScope,
        inner: SettingsScope,
        outer_text: Option<&str>,
    ) -> (Option<String>, [Option<String>; 2]) {
        let storage = InMemorySettingsStorage::new();
        support::put(&storage, SettingsScope::Global, "global");
        let mut inner_seen = None;
        storage
            .with_lock(outer, &mut |_| {
                inner_seen = write_inner(&storage, inner);
                Ok(outer_text.map(str::to_owned))
            })
            .unwrap();
        (inner_seen, BOTH.map(|scope| raw(&storage, scope)))
    }

    #[test]
    fn memory_callbacks_may_reenter_the_same_scope() {
        let (seen, stored) = reenter(SettingsScope::Global, SettingsScope::Global, Some("outer"));
        assert_eq!(seen.as_deref(), Some("global"));
        assert_eq!(
            stored,
            [Some("outer".to_owned()), None],
            "the outer write is the last write"
        );

        let (_, stored) = reenter(SettingsScope::Global, SettingsScope::Global, None);
        assert_eq!(
            stored,
            [Some("inner".to_owned()), None],
            "an outer callback that writes nothing keeps the inner write"
        );
    }

    #[test]
    fn memory_callbacks_may_reenter_the_other_scope() {
        let (seen, stored) = reenter(SettingsScope::Global, SettingsScope::Project, Some("outer"));
        assert_eq!(seen, None);
        assert_eq!(
            stored,
            [Some("outer".to_owned()), Some("inner".to_owned())],
            "each scope keeps its own last write"
        );

        let (seen, stored) = reenter(SettingsScope::Project, SettingsScope::Global, None);
        assert_eq!(seen.as_deref(), Some("global"));
        assert_eq!(stored, [Some("inner".to_owned()), None]);
    }

    /// File storage whose supplied directories pass through a directory that does not exist.
    fn dotted_storage(root: &Path) -> FileSettingsStorage {
        FileSettingsStorage::new(
            &location(root, "missing/../work"),
            &location(root, "missing/../agent"),
            ".maestro",
        )
    }

    #[test]
    fn supplied_paths_fold_dot_segments_before_reading() {
        let root = Scratch::new().unwrap();
        let (agent, config) = (root.path().join("agent"), root.path().join("work/.maestro"));
        fs::create_dir_all(&agent).unwrap();
        fs::create_dir_all(&config).unwrap();
        fs::write(agent.join("settings.json"), "global").unwrap();
        fs::write(config.join("settings.json"), "project").unwrap();
        let storage = dotted_storage(root.path());
        for (scope, text) in [
            (SettingsScope::Global, "global"),
            (SettingsScope::Project, "project"),
        ] {
            assert_eq!(transact(&storage, scope, Ok(None)).0.as_deref(), Some(text));
        }
    }

    #[test]
    fn supplied_paths_fold_dot_segments_before_writing() {
        let root = Scratch::new().unwrap();
        let storage = dotted_storage(root.path());
        for scope in BOTH {
            transact(&storage, scope, Ok(Some("written"))).1.unwrap();
        }
        for file in ["agent/settings.json", "work/.maestro/settings.json"] {
            assert_eq!(
                fs::read_to_string(root.path().join(file)).unwrap(),
                "written"
            );
        }
        assert!(
            !root.path().join("missing").exists(),
            "a folded segment is never created"
        );
    }

    /// Child side: replaces both scopes through locations relative to the process
    /// directory and reports the text each scope held.
    fn replace_through_relative_locations() {
        let storage = FileSettingsStorage::new("../..", "../agent", ".maestro");
        let held = BOTH.map(|scope| {
            let (seen, outcome) = transact(&storage, scope, Ok(Some("replaced")));
            outcome.unwrap();
            seen
        });
        support::report(&json!(held));
    }

    #[test]
    fn relative_locations_keep_their_leading_parents() {
        if support::child_case().is_some() {
            return replace_through_relative_locations();
        }
        let root = Scratch::new().unwrap();
        let process_directory = root.path().join("a/b");
        let files = [
            root.path().join("a/agent/settings.json"),
            root.path().join(".maestro/settings.json"),
        ];
        fs::create_dir_all(&process_directory).unwrap();
        for (file, text) in files.iter().zip(["global seed", "project seed"]) {
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }
        let output = support::child_command(
            "settings_storage::tests::relative_locations_keep_their_leading_parents",
            "run",
        )
        .current_dir(&process_directory)
        .output()
        .unwrap();
        assert!(output.status.success());
        assert_eq!(
            support::child_report(&output),
            json!(["global seed", "project seed"]),
            "each scope reads the file its parents lead to"
        );
        for file in &files {
            assert_eq!(fs::read_to_string(file).unwrap(), "replaced");
        }
    }

    /// Creates a manager over the temporary tree's supplied directories.
    fn create_manager(root: &Path) -> SettingsManager {
        SettingsManager::create(
            &location(root, "work"),
            &location(root, "agent"),
            ".maestro",
        )
    }

    /// File adapter observing exclusion at the actual manager-write callback.
    struct ObservedStorage {
        inner: FileSettingsStorage,
        sidecar: std::path::PathBuf,
        writes: std::sync::atomic::AtomicUsize,
    }

    impl ObservedStorage {
        /// Probe the sidecar while the file adapter still owns its exclusion.
        fn observe_write(&self, scope: SettingsScope, replacement: bool) {
            if scope == SettingsScope::Global && replacement {
                assert_eq!(
                    probe_lock(
                        "settings_storage::tests::global_key_edit_merges_fresh_global_scope",
                        &self.sidecar
                    ),
                    "blocked"
                );
                self.writes
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    impl SettingsStorage for ObservedStorage {
        fn with_lock(
            &self,
            scope: SettingsScope,
            update: &mut dyn FnMut(Option<&str>) -> maestro_settings::SettingsUpdate,
        ) -> Result<(), SettingsStorageError> {
            self.inner.with_lock(scope, update)
        }

        fn with_write_lock(
            &self,
            scope: SettingsScope,
            update: &mut dyn FnMut(Option<&str>) -> maestro_settings::SettingsUpdate,
        ) -> Result<(), SettingsStorageError> {
            self.inner.with_write_lock(scope, &mut |current| {
                let next = update(current)?;
                self.observe_write(scope, next.is_some());
                Ok(next)
            })
        }
    }

    #[test]
    fn saves_reach_a_hard_linked_file_in_place() {
        let root = Scratch::new().unwrap();
        let file = support::settings_file(root.path(), SettingsScope::Global);
        let linked = file.with_file_name("linked.json");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "{\"theme\": \"old\"}").unwrap();
        fs::hard_link(&file, &linked).unwrap();

        file_storage(root.path())
            .with_write_lock(SettingsScope::Global, &mut |_| {
                Ok(Some("{\"theme\": \"new\"}".into()))
            })
            .unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "{\"theme\": \"new\"}");
        assert_eq!(fs::read_to_string(&linked).unwrap(), "{\"theme\": \"new\"}");
    }

    #[cfg(unix)]
    #[test]
    fn saves_refuse_a_read_only_file_and_leave_it_whole() {
        let root = Scratch::new().unwrap();
        let file = support::settings_file(root.path(), SettingsScope::Global);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "{\"theme\": \"old\"}").unwrap();
        let Some(_restored) = maestro_test_directory::deny(&file, 0o444).unwrap() else {
            return;
        };

        let result = file_storage(root.path()).with_write_lock(SettingsScope::Global, &mut |_| {
            Ok(Some("{\"theme\": \"new\"}".into()))
        });

        let error = result.unwrap_err();
        let io = error.downcast_ref::<std::io::Error>().unwrap();
        assert_eq!(io.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read_to_string(&file).unwrap(), "{\"theme\": \"old\"}");
    }

    #[test]
    fn global_key_edit_merges_fresh_global_scope() {
        if support::run_lock_child() {
            return;
        }
        let root = Scratch::new().unwrap();
        let agent = root.path().join("agent");
        fs::create_dir_all(&agent).unwrap();
        let file = agent.join("settings.json");
        let seed = json!({"multiplexer": {"onboarding": true}, "theme": "chat"});
        fs::write(&file, serde_json::to_string_pretty(&seed).unwrap()).unwrap();
        let project = root.path().join("work/.maestro/settings.json");
        fs::create_dir_all(project.parent().unwrap()).unwrap();
        let project_bytes = b"{ \"multiplexer\": { \"mouseCapture\": true } }\n";
        fs::write(&project, project_bytes).unwrap();
        let storage = Arc::new(ObservedStorage {
            inner: file_storage(root.path()),
            sidecar: agent.join("settings.json.lock"),
            writes: std::sync::atomic::AtomicUsize::new(0),
        });
        let mut first = SettingsManager::from_storage(storage.clone());
        let mut second = SettingsManager::from_storage(storage.clone());
        let fresh = json!({
            "multiplexer": {"onboarding": true, "mouseCapture": false},
            "theme": "chat",
            "external": ["kept"]
        });
        fs::write(&file, serde_json::to_string_pretty(&fresh).unwrap()).unwrap();

        first.set_global_key("multiplexer", "onboarding", false);
        block_on(first.flush());
        second.set_global_key("multiplexer", "copyOnSelect", false);
        block_on(second.flush());

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(
            saved,
            json!({
                "multiplexer": {
                    "onboarding": false,
                    "mouseCapture": false,
                    "copyOnSelect": false
                },
                "theme": "chat",
                "external": ["kept"]
            })
        );
        assert!(first.drain_errors().is_empty());
        assert!(second.drain_errors().is_empty());
        assert_eq!(fs::read(project).unwrap(), project_bytes);
        assert_eq!(storage.writes.load(std::sync::atomic::Ordering::Relaxed), 2);
    }

    #[test]
    fn missing_read_creates_nothing() {
        let root = Scratch::new().unwrap();
        fs::create_dir_all(root.path().join("work")).unwrap();
        let mut settings = create_manager(root.path());
        assert!(settings.drain_errors().is_empty());
        assert!(
            !root.path().join("agent").exists(),
            "no global directory or lock"
        );
        assert!(
            !root.path().join("work/.maestro").exists(),
            "no project directory or lock"
        );

        fs::create_dir_all(root.path().join("agent")).unwrap();
        fs::write(
            root.path().join("agent/settings.json"),
            r#"{"theme":"kept"}"#,
        )
        .unwrap();
        let mut settings = create_manager(root.path());
        assert_eq!(
            settings.get_theme().as_deref(),
            Some("kept"),
            "the healthy scope still loads"
        );
        assert!(settings.drain_errors().is_empty());
        assert!(
            !root.path().join("work/.maestro").exists(),
            "a missing project scope stays missing"
        );
    }

    #[test]
    fn missing_write_calls_before_acquiring() {
        let root = Scratch::new().unwrap();
        let storage = file_storage(root.path());
        let config = root.path().join("work/.maestro");
        let mut calls = 0;
        storage
            .with_lock(SettingsScope::Project, &mut |current| {
                calls += 1;
                assert_eq!(current, None);
                assert!(
                    !config.exists(),
                    "the callback runs before the directory exists"
                );
                Ok(Some("{}".to_owned()))
            })
            .unwrap();
        assert_eq!(calls, 1, "one callback and no second read");
        assert_eq!(
            fs::read_to_string(config.join("settings.json")).unwrap(),
            "{}"
        );
        assert!(
            !config.join("settings.json.lock").exists(),
            "the write removes its lock file"
        );

        let mut settings = create_manager(root.path());
        settings.set_project_extension_paths(vec![maestro_settings::SettingsListEntry::String(
            "p".into(),
        )]);
        block_on(settings.flush());
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(config.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(saved, serde_json::json!({"extensions": ["p"]}));
    }

    #[test]
    fn first_project_write_creates_the_configuration_directory() {
        let root = Scratch::new().unwrap();
        fs::create_dir_all(root.path().join("agent")).unwrap();
        fs::write(
            root.path().join("agent/settings.json"),
            r#"{"theme":"dark"}"#,
        )
        .unwrap();
        let config = root.path().join("work/.maestro");
        let mut settings = create_manager(root.path());
        assert!(!config.exists(), "reading alone creates nothing");

        settings.set_project_packages(vec![PackageSource::Source("npm:test-pkg".into())]);
        block_on(settings.flush());
        assert!(config.is_dir(), "the write creates the directory");
        assert!(config.join("settings.json").is_file());
        assert_eq!(
            fs::read_to_string(config.join("settings.json")).unwrap(),
            "{\n  \"packages\": [\n    \"npm:test-pkg\"\n  ]\n}"
        );
    }

    /// Probes the sidecar of `settings.json` in `directory` from another process.
    fn sidecar_state(test: &str, directory: &Path) -> String {
        probe_lock(test, &directory.join("settings.json.lock"))
    }

    #[test]
    fn existing_transaction_holds_exclusion() {
        const TEST: &str = "settings_storage::tests::existing_transaction_holds_exclusion";
        if support::run_lock_child() {
            return;
        }
        let root = Scratch::new().unwrap();
        let storage = file_storage(root.path());
        let (agent, config) = (root.path().join("agent"), root.path().join("work/.maestro"));
        fs::create_dir_all(&config).unwrap();
        fs::create_dir_all(&agent).unwrap();
        fs::write(config.join("settings.json"), "project").unwrap();
        fs::write(agent.join("settings.json"), "global").unwrap();

        let mut states = Vec::new();
        storage
            .with_lock(SettingsScope::Global, &mut |current| {
                assert_eq!(current, Some("global"));
                states.push(sidecar_state(TEST, &agent));
                states.push(sidecar_state(TEST, &config));
                Ok(Some("replaced".to_owned()))
            })
            .unwrap();
        assert_eq!(
            states,
            ["blocked", "free"],
            "only the transacting scope is held"
        );
        assert_eq!(
            sidecar_state(TEST, &agent),
            "free",
            "the lock is released afterwards"
        );
        assert_eq!(
            fs::read_to_string(agent.join("settings.json")).unwrap(),
            "replaced"
        );
    }

    #[test]
    fn held_sidecar_skips_update_until_released() {
        const TEST: &str = "settings_storage::tests::held_sidecar_skips_update_until_released";
        if support::run_lock_child() {
            return;
        }
        let root = Scratch::new().unwrap();
        let storage = file_storage(root.path());
        let agent = root.path().join("agent");
        fs::create_dir_all(&agent).unwrap();
        fs::write(agent.join("settings.json"), "global").unwrap();
        let holder = Holder::start(TEST, &agent.join("settings.json.lock"));

        let mut calls = 0;
        let outcome = storage.with_lock(SettingsScope::Global, &mut |_| {
            calls += 1;
            Ok(None)
        });
        assert!(outcome.is_err(), "a held lock exhausts the attempts");
        assert_eq!(calls, 0, "the callback never runs");

        holder.release();
        transact(&storage, SettingsScope::Global, Ok(None))
            .1
            .unwrap();
    }

    #[test]
    fn sidecar_open_and_read_failures_skip_update() {
        let root = Scratch::new().unwrap();
        let storage = file_storage(root.path());
        let agent = root.path().join("agent");
        fs::create_dir_all(agent.join("settings.json.lock")).unwrap();
        fs::write(agent.join("settings.json"), "global").unwrap();
        let (seen, outcome) = transact(&storage, SettingsScope::Global, Ok(None));
        assert!(outcome.is_err(), "the sidecar cannot be opened");
        assert_eq!(seen, None, "the callback never runs");

        fs::remove_dir(agent.join("settings.json.lock")).unwrap();
        fs::remove_file(agent.join("settings.json")).unwrap();
        fs::create_dir(agent.join("settings.json")).unwrap();
        let (seen, outcome) = transact(&storage, SettingsScope::Global, Ok(None));
        assert!(outcome.is_err(), "a directory cannot be read as settings");
        assert_eq!(seen, None);
    }

    #[test]
    fn callback_and_write_failure_release_lock() {
        const TEST: &str = "settings_storage::tests::callback_and_write_failure_release_lock";
        if support::run_lock_child() {
            return;
        }
        let root = Scratch::new().unwrap();
        let storage = file_storage(root.path());
        let agent = root.path().join("agent");
        fs::create_dir_all(&agent).unwrap();
        let file = agent.join("settings.json");
        fs::write(&file, "global").unwrap();

        transact(&storage, SettingsScope::Global, Err("callback failed"))
            .1
            .unwrap_err();
        assert_eq!(
            sidecar_state(TEST, &agent),
            "free",
            "released after a callback failure"
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), "global");

        let outcome = storage.with_lock(SettingsScope::Global, &mut |_| {
            fs::remove_file(&file).unwrap();
            fs::create_dir(&file).unwrap();
            Ok(Some("unwritable".to_owned()))
        });
        assert!(outcome.is_err(), "writing over a directory fails");
        assert_eq!(
            sidecar_state(TEST, &agent),
            "free",
            "released after a write failure"
        );
    }

    #[test]
    fn settings_writes_leave_only_the_settings_file() {
        let root = Scratch::new().unwrap();
        let storage = file_storage(root.path());
        let agent = root.path().join("agent");
        for text in ["one", "two"] {
            transact(&storage, SettingsScope::Global, Ok(Some(text)))
                .1
                .unwrap();
            assert_eq!(
                fs::read_to_string(agent.join("settings.json")).unwrap(),
                text
            );
            assert_eq!(fs::read_dir(&agent).unwrap().count(), 1);
        }
    }

    #[cfg(unix)]
    #[test]
    fn linked_settings_lock_beside_the_linked_file() {
        let root = Scratch::new().unwrap();
        let storage = file_storage(root.path());
        let agent = root.path().join("agent");
        fs::create_dir(&agent).unwrap();
        let target = root.path().join("actual.json");
        fs::write(&target, "before").unwrap();
        std::os::unix::fs::symlink(&target, agent.join("settings.json")).unwrap();
        storage
            .with_lock(SettingsScope::Global, &mut |current| {
                assert_eq!(current, Some("before"));
                assert!(root.path().join("actual.json.lock").is_file());
                assert!(!agent.join("settings.json.lock").exists());
                Ok(Some("after".into()))
            })
            .unwrap();
        assert_eq!(fs::read_to_string(target).unwrap(), "after");
        assert!(!root.path().join("actual.json.lock").exists());
    }

    #[test]
    fn settings_write_succeeds_without_a_home() {
        const TEST: &str = "settings_storage::tests::settings_write_succeeds_without_a_home";
        if support::child_case().is_some() {
            let root = std::env::current_dir().unwrap();
            let storage = file_storage(&root);
            transact(&storage, SettingsScope::Global, Ok(Some("saved")))
                .1
                .unwrap();
            return;
        }
        let root = Scratch::new().unwrap();
        let output = support::child_command(TEST, "no-home")
            .current_dir(root.path())
            .env("HOME", root.path().join("missing-home"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(!root.path().join("missing-home").exists());
        assert_eq!(
            fs::read_to_string(root.path().join("agent/settings.json")).unwrap(),
            "saved"
        );
    }

    /// Child side: successful saves, a reload and a failed load, with nothing printed.
    fn run_quiet_operations() {
        let root = Scratch::new().unwrap();
        fs::create_dir_all(root.path().join("agent")).unwrap();
        let mut settings = create_manager(root.path());
        settings.set_theme("quiet".into());
        block_on(settings.flush());
        fs::write(root.path().join("agent/settings.json"), "{not json").unwrap();
        block_on(settings.reload());
        assert_eq!(
            settings.drain_errors().len(),
            1,
            "failures surface only through the drain"
        );
    }

    #[test]
    fn settings_operations_are_silent() {
        if support::child_case().is_some() {
            return run_quiet_operations();
        }
        let output = support::child_command(
            "settings_storage::tests::settings_operations_are_silent",
            "quiet",
        )
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let unexpected: Vec<_> = stdout
            .lines()
            .filter(|line| {
                !line.is_empty()
                    && *line != "running 1 test"
                    && *line != "."
                    && !line.starts_with("test result:")
            })
            .collect();
        assert_eq!(
            unexpected,
            Vec::<&str>::new(),
            "only the test harness prints"
        );
        assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    }

    #[test]
    fn settings_manager_module_exposes_the_crate_root_items() {
        use maestro_settings::settings_manager::{
            FileSettingsStorage as ModuleFileStorage, InMemorySettingsStorage as ModuleMemory,
            SettingsManager as ModuleManager, SettingsScope as ModuleScope,
        };
        let _file: FileSettingsStorage = ModuleFileStorage::new("w", "a", ".c");
        let storage: Arc<InMemorySettingsStorage> = Arc::new(ModuleMemory::new());
        let mut manager: SettingsManager = ModuleManager::from_storage(storage.clone());
        manager.set_theme("light".into());
        block_on(manager.flush());
        let scope: SettingsScope = ModuleScope::Global;
        assert_eq!(
            raw(&*storage, scope).as_deref(),
            Some("{\n  \"theme\": \"light\"\n}")
        );
    }
}
