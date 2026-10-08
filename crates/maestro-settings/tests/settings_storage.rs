//! Raw storage contract shared by the memory, native-file and controlled adapters, and
//! the native file locking.

#[cfg(test)]
mod support;

#[cfg(test)]
mod tests {
    use super::support;
    use std::ffi::OsStr;
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use super::support::{ControlledStorage, Holder, TempDir, block_on, probe_lock, raw};
    use maestro_settings::{
        FileSettingsStorage, InMemorySettingsStorage, SettingsManager, SettingsScope,
        SettingsStorage, SettingsStorageError,
    };

    const BOTH: [SettingsScope; 2] = [SettingsScope::Global, SettingsScope::Project];

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

    /// File storage below a temporary tree: working directory, agent directory and configuration name.
    fn file_storage(root: &Path) -> FileSettingsStorage {
        FileSettingsStorage::new(
            &root.join("work"),
            &root.join("agent"),
            OsStr::new(".maestro"),
        )
    }

    #[test]
    fn adapters_share_raw_transaction_contract() {
        assert_raw_transaction_contract(&InMemorySettingsStorage::new());
        assert_raw_transaction_contract(&*ControlledStorage::new());
        let root = TempDir::new();
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

    /// Creates a manager over the temporary tree's supplied directories.
    fn create_manager(root: &Path) -> SettingsManager {
        SettingsManager::create(
            &root.join("work"),
            &root.join("agent"),
            OsStr::new(".maestro"),
        )
    }

    #[test]
    fn missing_read_creates_nothing() {
        let root = TempDir::new();
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
        let root = TempDir::new();
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
            config.join("settings.json.lock").exists(),
            "the sidecar is created for the write"
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

    /// Probes the sidecar of `settings.json` in `directory` from another process.
    fn sidecar_state(test: &str, directory: &Path) -> String {
        probe_lock(test, &directory.join("settings.json.lock"))
    }

    #[test]
    fn existing_transaction_holds_exclusion() {
        const TEST: &str = "existing_transaction_holds_exclusion";
        if support::run_lock_child() {
            return;
        }
        let root = TempDir::new();
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
    fn contention_retries_ten_times() {
        const TEST: &str = "contention_retries_ten_times";
        if support::run_lock_child() {
            return;
        }
        let root = TempDir::new();
        let storage = file_storage(root.path());
        let agent = root.path().join("agent");
        fs::create_dir_all(&agent).unwrap();
        fs::write(agent.join("settings.json"), "global").unwrap();
        let holder = Holder::start(TEST, &agent.join("settings.json.lock"));

        let started = Instant::now();
        let mut calls = 0;
        let outcome = storage.with_lock(SettingsScope::Global, &mut |_| {
            calls += 1;
            Ok(None)
        });
        let waited = started.elapsed();
        assert!(outcome.is_err(), "a held lock exhausts the attempts");
        assert_eq!(calls, 0, "the callback never runs");
        assert!(
            waited >= Duration::from_millis(180),
            "nine waits of 20 ms, got {waited:?}"
        );

        holder.release();
        transact(&storage, SettingsScope::Global, Ok(None))
            .1
            .unwrap();
    }

    #[test]
    fn non_contention_errors_return_immediately() {
        let root = TempDir::new();
        let storage = file_storage(root.path());
        let agent = root.path().join("agent");
        fs::create_dir_all(agent.join("settings.json.lock")).unwrap();
        fs::write(agent.join("settings.json"), "global").unwrap();
        let mut fastest = Duration::MAX;
        for _ in 0..5 {
            let started = Instant::now();
            let (seen, outcome) = transact(&storage, SettingsScope::Global, Ok(None));
            assert!(outcome.is_err(), "the sidecar cannot be opened");
            assert_eq!(seen, None, "the callback never runs");
            fastest = fastest.min(started.elapsed());
        }
        assert!(
            fastest < Duration::from_millis(150),
            "retrying would take at least nine waits of 20 ms, got {fastest:?}"
        );

        fs::remove_dir(agent.join("settings.json.lock")).unwrap();
        fs::remove_file(agent.join("settings.json")).unwrap();
        fs::create_dir(agent.join("settings.json")).unwrap();
        let (seen, outcome) = transact(&storage, SettingsScope::Global, Ok(None));
        assert!(outcome.is_err(), "a directory cannot be read as settings");
        assert_eq!(seen, None);
    }

    #[test]
    fn callback_and_write_failure_release_lock() {
        const TEST: &str = "callback_and_write_failure_release_lock";
        if support::run_lock_child() {
            return;
        }
        let root = TempDir::new();
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
    fn sidecar_remains_stable_after_release() {
        const TEST: &str = "sidecar_remains_stable_after_release";
        if support::run_lock_child() {
            return;
        }
        let root = TempDir::new();
        let storage = file_storage(root.path());
        let agent = root.path().join("agent");
        transact(&storage, SettingsScope::Global, Ok(Some("one")))
            .1
            .unwrap();
        let sidecar = agent.join("settings.json.lock");
        let first = fs::metadata(&sidecar).unwrap();
        transact(&storage, SettingsScope::Global, Ok(Some("two")))
            .1
            .unwrap();
        let second = fs::metadata(&sidecar).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(first.ino(), second.ino(), "the sidecar is never replaced");
        }
        assert!(first.is_file() && second.is_file());

        let holder = Holder::start(TEST, &sidecar);
        assert_eq!(probe_lock(TEST, &sidecar), "blocked");
        holder.release();
        assert_eq!(probe_lock(TEST, &sidecar), "free");
        assert!(sidecar.exists(), "release never unlinks the sidecar");
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_preserved() {
        use std::os::unix::ffi::OsStrExt;
        let root = TempDir::new();
        let cwd = root.path().join(OsStr::from_bytes(b"w\xffk"));
        let agent = root.path().join(OsStr::from_bytes(b"a\xffg"));
        let config = OsStr::from_bytes(b".m\xffc");
        let mut settings = SettingsManager::create(&cwd, &agent, config);
        settings.set_theme("raw bytes".into());
        settings.set_project_extension_paths(vec![maestro_settings::SettingsListEntry::String(
            "p".into(),
        )]);
        block_on(settings.flush());
        assert!(agent.join("settings.json").is_file());
        assert!(cwd.join(config).join("settings.json").is_file());

        let reloaded = SettingsManager::create(&cwd, &agent, config);
        assert_eq!(reloaded.get_theme().as_deref(), Some("raw bytes"));
        assert_eq!(reloaded.get_extension_paths().len(), 1);
    }

    /// Child side: successful saves, a reload and a failed load, with nothing printed.
    fn run_quiet_operations() {
        let root = TempDir::new();
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
        let output = support::child_command("settings_operations_are_silent", "quiet")
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
    fn exports_available_for_target() {
        use maestro_settings::{
            BranchSummarySettings, CompactionSettings, ImageSettings, MarkdownSettings,
            PackageSource, ProviderRetrySettings, RetrySettings, Settings, SettingsError,
            SettingsStorageHandle, TerminalSettings, ThinkingBudgetsSettings, TransportSetting,
            WarningSettings,
        };
        let handle: SettingsStorageHandle = std::sync::Arc::new(InMemorySettingsStorage::new());
        let mut manager = SettingsManager::from_storage(handle);
        let _native = FileSettingsStorage::new(Path::new("w"), Path::new("a"), OsStr::new(".c"));
        manager.set_transport(TransportSetting::Sse);
        manager.set_packages(vec![PackageSource::Source("npm:p".into())]);
        let error = SettingsError {
            scope: SettingsScope::Project,
            error: "failed".into(),
        };
        assert_eq!(error.scope, SettingsScope::Project);
        let records = [
            CompactionSettings::default().extra,
            BranchSummarySettings::default().extra,
            RetrySettings::default().extra,
            ProviderRetrySettings::default().extra,
            TerminalSettings::default().extra,
            ImageSettings::default().extra,
            ThinkingBudgetsSettings::default().extra,
            MarkdownSettings::default().extra,
            WarningSettings::default().extra,
        ];
        assert!(records.iter().all(serde_json::Map::is_empty));
        assert_eq!(Settings::default().0.len(), 0);
        assert_eq!(manager.get_packages().len(), 1);
    }
}
