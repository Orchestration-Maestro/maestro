//! Environment and home decision boundaries with native witnesses.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use super::operations::{home, recover_environment};
use std::{cell::Cell, collections::BTreeMap, io};

#[test]
fn empty_linux_environment_recovery_preserves_entries() {
    let reads = Cell::new(0);
    let result = recover_environment("linux", BTreeMap::new(), || {
        reads.set(reads.get() + 1);
        Ok(b"A=1\0NOEQ\0=bad\0B=x=y\0A=last\0EMPTY=\0".to_vec())
    });
    assert_eq!(
        result,
        BTreeMap::from([
            ("A".into(), "last".into()),
            ("B".into(), "x=y".into()),
            ("EMPTY".into(), String::new())
        ])
    );
    assert_eq!(reads.get(), 1);
    assert!(
        recover_environment("darwin", BTreeMap::new(), || panic!("unexpected read")).is_empty()
    );
    assert_eq!(
        recover_environment(
            "linux",
            BTreeMap::from([("A".into(), "present".into())]),
            || panic!("unexpected read")
        ),
        BTreeMap::from([("A".into(), "present".into())])
    );
    assert!(
        recover_environment("linux", BTreeMap::new(), || Err(io::Error::other(
            "unavailable"
        )))
        .is_empty()
    );
}
#[test]
fn home_lookup_uses_truthy_environment_before_platform_fallback() {
    assert_eq!(
        home(Some("/explicit/home".into()), || panic!("eager fallback")).unwrap(),
        "/explicit/home"
    );
    for explicit in [None, Some(String::new())] {
        let reads = Cell::new(0);
        assert_eq!(
            home(explicit, || {
                reads.set(reads.get() + 1);
                Ok("/platform/home".into())
            })
            .unwrap(),
            "/platform/home"
        );
        assert_eq!(reads.get(), 1);
    }
}
#[cfg(target_os = "linux")]
#[test]
fn native_empty_environment_recovery_reads_procfs() {
    use super::{NativePackageOperations, PackageOperations};
    if std::env::var_os("MAESTRO_ENV_CHILD").is_some() {
        let result = recover_environment("linux", BTreeMap::new(), || {
            std::fs::read("/proc/self/environ")
        });
        assert_eq!(
            result.get("MAESTRO_ENV_SENTINEL").map(String::as_str),
            Some("a=b")
        );
        let output = NativePackageOperations::new(
            |_| false,
            std::rc::Rc::new(|| false),
            &std::rc::Rc::new(tokio::task::LocalSet::new()),
        )
        .run_command_sync(
            "/bin/sh",
            &["-c".into(), "printf '%s' \"$MAESTRO_ENV_SENTINEL\"".into()],
        )
        .unwrap();
        assert_eq!(output.stdout, "a=b");
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "package_manager::tests::native_empty_environment_recovery_reads_procfs",
            "--nocapture",
        ])
        .env_clear()
        .env("MAESTRO_ENV_CHILD", "1")
        .env("MAESTRO_ENV_SENTINEL", "a=b")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// Creates an inert native manager over supplied command settings.
#[cfg(unix)]
fn native_manager(
    settings: maestro_settings::Settings,
) -> super::DefaultPackageManager<super::NativePackageOperations> {
    super::DefaultPackageManager::new(
        super::PackageManagerOptions {
            cwd: std::env::temp_dir().to_string_lossy().into_owned(),
            agent_dir: "/unused".into(),
            settings_manager: std::rc::Rc::new(std::cell::RefCell::new(
                maestro_settings::SettingsManager::in_memory(settings),
            )),
        },
        super::NativePackageOperations::new(
            |_| false,
            std::rc::Rc::new(|| false),
            &std::rc::Rc::new(tokio::task::LocalSet::new()),
        ),
    )
}
#[test]
#[cfg(unix)]
fn latest_version_rejects_empty_capture_with_authored_message() {
    for (output, expected) in [
        ("\"2\"", Some("2")),
        (" \u{feff}\"2\"\n", Some("2")),
        ("", None),
        ("\u{85}\"2\"", None),
        ("bad", None),
    ] {
        let script = format!("printf '%s' '{output}' #");
        let settings = maestro_settings::Settings(
            serde_json::json!({"npmCommand":["/bin/sh","-c",script]})
                .as_object()
                .unwrap()
                .clone(),
        );
        let manager = native_manager(settings);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(manager.latest_npm_version("x"));
        if let Some(expected) = expected {
            assert_eq!(result.unwrap(), expected);
        } else if output.is_empty() {
            assert_eq!(
                result.unwrap_err().to_string(),
                "Empty response from npm view"
            );
        } else {
            assert!(result.is_err());
        }
    }
    let settings = maestro_settings::Settings(
        serde_json::json!({"npmCommand":["/bin/sh","-c","printf '\\357\\273\\277' #"]})
            .as_object()
            .unwrap()
            .clone(),
    );
    let manager = native_manager(settings);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert_eq!(
        runtime
            .block_on(manager.latest_npm_version("example"))
            .unwrap_err()
            .to_string(),
        "Empty response from npm view"
    );
}

#[test]
#[cfg(unix)]
fn remote_head_rejects_unmatched_head_with_authored_message() {
    let path = std::env::temp_dir().join(format!(
        "maestro-empty-remote-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    for args in [
        vec!["init"],
        vec!["init", "--bare", "remote"],
        vec![
            "remote",
            "add",
            "origin",
            path.join("remote").to_str().unwrap(),
        ],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(&path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap()
                .success()
        );
    }
    let manager = native_manager(maestro_settings::Settings::default());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(manager.remote_git_head(path.to_str().unwrap()));
    std::fs::remove_dir_all(path).unwrap();
    assert_eq!(
        result.unwrap_err().to_string(),
        "Failed to determine remote HEAD"
    );
}
