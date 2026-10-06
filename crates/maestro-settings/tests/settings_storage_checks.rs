#![cfg(not(target_arch = "wasm32"))]
mod support;
use maestro_settings::{
    FileSettingsStorage, InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage,
};
use serde_json::json;
use std::sync::Arc;
use support::*;

#[test]
fn native_first_acquisition_sets_directory_timestamp_and_releases() {
    let scratch = Scratch::new();
    let storage = scratch.storage();
    storage
        .with_lock(SettingsScope::Global, &mut |text| {
            assert_eq!(text, None);
            Ok(Some("{}".into()))
        })
        .unwrap();
    assert!(scratch.root.join("user/settings.json").is_file());
    assert!(!scratch.root.join("user/settings.json.lock").exists());
}

#[test]
fn project_read_does_not_create_directory() {
    let scratch = Scratch::new();
    let m = SettingsManager::create(
        &scratch.root.join("cwd"),
        &scratch.root.join("user"),
        ".maestro",
        Scheduler::default().spawn(),
    );
    assert_eq!(m.get_project_settings(), json!({}));
    assert!(!scratch.root.join("cwd").exists());
    assert!(!scratch.root.join("user").exists());
}

#[test]
fn project_save_creates_directory() {
    let scratch = Scratch::new();
    let s = Arc::new(scratch.storage());
    let q = Scheduler::default();
    let m = SettingsManager::from_storage(s.clone(), q.spawn());
    m.set_project_extension_paths(vec!["local".into()]);
    assert!(!scratch.root.join("cwd").exists());
    q.drive();
    assert!(scratch.root.join("cwd/.maestro/settings.json").is_file());
    assert_eq!(
        disk(s.as_ref(), SettingsScope::Project),
        json!({"extensions":["local"]})
    );
}

#[test]
fn existing_file_is_locked_through_read_callback_and_write() {
    let scratch = Scratch::new();
    let s = scratch.storage();
    put(&s, SettingsScope::Global, "{}");
    s.with_lock(SettingsScope::Global, &mut |text| {
        assert_eq!(text, Some("{}"));
        assert!(scratch.root.join("user/settings.json.lock").is_dir());
        let contender = scratch.storage();
        let e = contender
            .with_lock(SettingsScope::Global, &mut |_| Ok(None))
            .unwrap_err();
        assert_eq!(e.to_string(), "Lock file is already being held");
        Ok(Some("{\"theme\":\"dark\"}".into()))
    })
    .unwrap();
    assert!(!scratch.root.join("user/settings.json.lock").exists());
    assert_eq!(disk(&s, SettingsScope::Global), json!({"theme":"dark"}));
}

#[test]
fn raw_storage_none_preserves_bytes() {
    let scratch = Scratch::new();
    let s = scratch.storage();
    put(&s, SettingsScope::Global, "{}");
    std::fs::write(
        scratch.root.join("user/settings.json"),
        b"{\"theme\":\"\xff\"}",
    )
    .unwrap();
    assert_eq!(
        raw(&s, SettingsScope::Global).as_deref(),
        Some("{\"theme\":\"\u{fffd}\"}")
    );

    let scratch = Scratch::new();
    let adapters: Vec<Arc<dyn SettingsStorage>> = vec![
        Arc::new(scratch.storage()),
        Arc::new(InMemorySettingsStorage::new()),
        Arc::new(Controlled::default()),
    ];
    for s in adapters {
        for scope in [SettingsScope::Global, SettingsScope::Project] {
            put(s.as_ref(), scope, " raw bytes ");
            s.with_lock(scope, &mut |text| {
                assert_eq!(text, Some(" raw bytes "));
                Ok(None)
            })
            .unwrap();
            assert_eq!(raw(s.as_ref(), scope).as_deref(), Some(" raw bytes "));
            let e = s
                .with_lock(scope, &mut |_| Err(Box::new(Sentinel(7))))
                .unwrap_err();
            assert_eq!(e.downcast_ref::<Sentinel>().unwrap().0, 7);
            assert_eq!(raw(s.as_ref(), scope).as_deref(), Some(" raw bytes "));
            put(s.as_ref(), scope, "");
            assert_eq!(raw(s.as_ref(), scope).as_deref(), Some(""));
        }
    }
}

#[test]
fn storage_swap_uses_identical_manager_calls() {
    let scratch = Scratch::new();
    let adapters: Vec<Arc<dyn SettingsStorage>> = vec![
        Arc::new(scratch.storage()),
        Arc::new(InMemorySettingsStorage::new()),
        Arc::new(Controlled::default()),
    ];
    for s in adapters {
        put(
            s.as_ref(),
            SettingsScope::Global,
            r#"{"theme":"seed","unknown":true}"#,
        );
        let q = Scheduler::default();
        let m = SettingsManager::from_storage(s.clone(), q.spawn());
        assert_eq!(m.get_theme().as_deref(), Some("seed"));
        m.set_theme("saved".into());
        q.drive();
        block_on(m.flush());
        assert_eq!(
            disk(s.as_ref(), SettingsScope::Global),
            json!({"theme":"saved","unknown":true})
        );
        put(s.as_ref(), SettingsScope::Global, r#"{"theme":"external"}"#);
        block_on(m.reload());
        assert_eq!(m.get_theme().as_deref(), Some("external"));
        put(s.as_ref(), SettingsScope::Global, "{");
        block_on(m.reload());
        assert_eq!(m.drain_errors().len(), 1);
        assert!(m.drain_errors().is_empty());
    }
}

#[test]
fn supplied_directories_keep_join_and_lexical_lock_identity() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let scratch = Scratch::new();
        let original = scratch.storage();
        put(&original, SettingsScope::Global, "{}");
        symlink(
            scratch.root.join("user"),
            scratch.root.join("directory-alias"),
        )
        .unwrap();
        std::fs::create_dir(scratch.root.join("file-alias")).unwrap();
        symlink(
            scratch.root.join("user/settings.json"),
            scratch.root.join("file-alias/settings.json"),
        )
        .unwrap();
        original
            .with_lock(SettingsScope::Global, &mut |_| {
                let directory = FileSettingsStorage::new(
                    &scratch.root,
                    &scratch.root.join("directory-alias"),
                    ".maestro",
                );
                let error = directory
                    .with_lock(SettingsScope::Global, &mut |_| Ok(None))
                    .unwrap_err();
                assert_eq!(error.to_string(), "Lock file is already being held");
                assert!(format!("{error:?}").contains("directory-alias"));
                let file = FileSettingsStorage::new(
                    &scratch.root,
                    &scratch.root.join("file-alias"),
                    ".maestro",
                );
                assert_eq!(raw(&file, SettingsScope::Global).as_deref(), Some("{}"));
                assert!(!scratch.root.join("file-alias/settings.json.lock").exists());
                Ok(None)
            })
            .unwrap();
    }

    let scratch = Scratch::new();
    for name in [".alternate", "/.alternate", "./.alternate//"] {
        let cwd = scratch.root.join("cwd//./nested/..");
        let agent = scratch.root.join("different//./nested/..");
        let s = FileSettingsStorage::new(&cwd, &agent, name);
        put(&s, SettingsScope::Global, "{}");
        put(&s, SettingsScope::Project, "{}");
        assert!(scratch.root.join("different/settings.json").is_file());
        assert!(scratch.root.join("cwd/.alternate/settings.json").is_file());
    }
    let m = SettingsManager::create(
        &scratch.root.join("cwd"),
        &scratch.root.join("different"),
        ".alternate",
        Scheduler::default().spawn(),
    );
    assert!(m.drain_errors().is_empty());
}

#[test]
fn missing_write_calls_back_before_directory_and_lock() {
    let scratch = Scratch::new();
    let s = scratch.storage();
    s.with_lock(SettingsScope::Project, &mut |text| {
        assert!(text.is_none());
        assert!(!scratch.root.join("cwd").exists());
        Ok(None)
    })
    .unwrap();
    s.with_lock(SettingsScope::Project, &mut |text| {
        assert!(text.is_none());
        assert!(!scratch.root.join("cwd").exists());
        let other = scratch.storage();
        put(&other, SettingsScope::Project, r#"{"interleaved":true}"#);
        Ok(Some(r#"{"winner":true}"#.into()))
    })
    .unwrap();
    assert_eq!(disk(&s, SettingsScope::Project), json!({"winner":true}));
    assert!(
        !scratch
            .root
            .join("cwd/.maestro/settings.json.lock")
            .exists()
    );
}

#[test]
fn storage_failures_always_release_acquired_lock() {
    let scratch = Scratch::new();
    let s = scratch.storage();
    put(&s, SettingsScope::Global, "{}");
    let lock = scratch.root.join("user/settings.json.lock");
    let e = s
        .with_lock(SettingsScope::Global, &mut |_| Err(Box::new(Sentinel(1))))
        .unwrap_err();
    assert_eq!(e.downcast_ref::<Sentinel>().unwrap().0, 1);
    assert!(!lock.exists());
    std::fs::remove_file(scratch.root.join("user/settings.json")).unwrap();
    std::fs::create_dir(scratch.root.join("user/settings.json")).unwrap();
    assert!(
        s.with_lock(SettingsScope::Global, &mut |_| panic!(
            "read should fail first"
        ))
        .is_err()
    );
    assert!(!lock.exists());
    std::fs::remove_dir(scratch.root.join("user/settings.json")).unwrap();
    put(&s, SettingsScope::Global, "{}");
    let e = s
        .with_lock(SettingsScope::Global, &mut |_| {
            std::fs::write(lock.join("prevent-removal"), "x").unwrap();
            Err(Box::new(Sentinel(2)))
        })
        .unwrap_err();
    assert!(e.downcast_ref::<Sentinel>().is_none());
    assert!(lock.is_dir());
    std::fs::remove_file(lock.join("prevent-removal")).unwrap();
    std::fs::remove_dir(lock).unwrap();
}

#[test]
fn lock_contention_has_ten_attempts_and_nine_gaps() {
    let scratch = Scratch::new();
    let s = scratch.storage();
    put(&s, SettingsScope::Global, "{}");
    std::fs::create_dir(scratch.root.join("user/settings.json.lock")).unwrap();
    let error = s
        .with_lock(SettingsScope::Global, &mut |_| {
            panic!("lock must fail before callback")
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "Lock file is already being held");
    assert!(format!("{error:?}").contains("ELOCKED"));
}

#[test]
fn lease_live_stale_precision_and_replacement_checks() {
    let scratch = Scratch::new();
    let s = scratch.storage();
    put(&s, SettingsScope::Global, "{}");
    let lock = scratch.root.join("user/settings.json.lock");
    for old in [
        std::time::SystemTime::now() - std::time::Duration::from_secs(11),
        std::time::UNIX_EPOCH - std::time::Duration::from_secs(1),
    ] {
        std::fs::create_dir(&lock).unwrap();
        std::fs::File::open(&lock)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(old))
            .unwrap();
        assert_eq!(raw(&s, SettingsScope::Global).as_deref(), Some("{}"));
        assert!(!lock.exists());
    }
}

#[test]
fn lease_heartbeat_compromise_and_release_checks() {
    let scratch = Scratch::new();
    let s = scratch.storage();
    put(&s, SettingsScope::Global, "{}");
    let lock = scratch.root.join("user/settings.json.lock");
    s.with_lock(SettingsScope::Global, &mut |_| {
        let before = std::fs::metadata(&lock).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert_eq!(
            std::fs::metadata(&lock).unwrap().modified().unwrap(),
            before
        );
        Ok(None)
    })
    .unwrap();
    assert!(!lock.exists());
}

#[test]
fn storage_errors_preserve_exact_messages() {
    let scratch = Scratch::new();
    let s = scratch.storage();
    put(&s, SettingsScope::Global, "{}");
    let error = s
        .with_lock(SettingsScope::Global, &mut |_| Err(Box::new(Sentinel(91))))
        .unwrap_err();
    assert_eq!(error.to_string(), "sentinel-91");
    assert_eq!(error.downcast_ref::<Sentinel>().unwrap().0, 91);
}

#[test]
fn manager_and_storage_are_send_sync() {
    fn traits<T: Send + Sync + ?Sized>() {}
    traits::<SettingsManager>();
    traits::<FileSettingsStorage>();
    traits::<InMemorySettingsStorage>();
    traits::<dyn SettingsStorage>();
    let (m, s, q) = support::empty();
    let shared = m.clone();
    shared.set_theme("one".into());
    m.set_project_extension_paths(vec!["p".into()]);
    shared.set_theme("two".into());
    assert_eq!(q.len(), 3);
    q.drive();
    assert_eq!(m.get_theme().as_deref(), Some("two"));
    assert_eq!(shared.get_extension_paths(), vec!["p"]);
    assert_eq!(disk(s.as_ref(), SettingsScope::Global)["theme"], "two");
}

#[test]
fn settings_operations_emit_no_console_output() {
    let scratch = Scratch::new();
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let library = std::fs::read_dir(&deps)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("libmaestro_settings-")
                && p.extension().is_some_and(|e| e == "rlib")
        })
        .max_by_key(|p| std::fs::metadata(p).unwrap().modified().unwrap())
        .unwrap();
    let output = scratch
        .root
        .join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let compiled = std::process::Command::new("rustc")
        .args(["--edition=2024", "--crate-name=settings_output_probe"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/output_probe.rs"
        ))
        .arg("--extern")
        .arg(format!("maestro_settings={}", library.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let result = std::process::Command::new(output)
        .arg(&scratch.root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
#[cfg(target_os = "linux")]
fn session_directory_uses_environment_or_account_home() {
    if let Ok(home) = std::env::var("SETTINGS_HOME_EXPECTED") {
        let (m, _) = memory(json!({"sessionDir":"~"}));
        assert_eq!(m.get_session_dir(), Some(home.clone()));
        let (m, _) = memory(json!({"sessionDir":"~/sessions"}));
        assert_eq!(
            m.get_session_dir(),
            Some(if home.is_empty() {
                "sessions".into()
            } else {
                format!("{home}/sessions")
            })
        );
        return;
    }
    let account = std::process::Command::new("sh")
        .args(["-c", "getent passwd $(id -u) | cut -d: -f6 | tr -d '\n'"])
        .env_remove("HOME")
        .output()
        .unwrap();
    assert!(account.status.success());
    let account = String::from_utf8(account.stdout).unwrap();
    assert!(!account.is_empty());
    for home in [None, Some("")] {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "session_directory_uses_environment_or_account_home",
                "--nocapture",
            ])
            .env("SETTINGS_HOME_EXPECTED", home.unwrap_or(&account));
        if let Some(home) = home {
            command.env("HOME", home);
        } else {
            command.env_remove("HOME");
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
