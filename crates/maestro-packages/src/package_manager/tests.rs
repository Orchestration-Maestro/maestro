//! Environment recovery boundaries with native witnesses.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use super::operations::recover_environment;
use std::{cell::Cell, collections::BTreeMap, io};

#[cfg(unix)]
#[path = "../../tests/support/runtime.rs"]
pub(super) mod runtime;

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
#[cfg(target_os = "linux")]
#[test]
fn native_empty_environment_recovery_reads_procfs() {
    use super::{NativePackageOperations, PackageOperations};
    if maestro_test_process::selection().is_child() {
        assert_eq!(
            std::env::var_os("LLVM_PROFILE_FILE"),
            std::env::var_os("MAESTRO_EXPECT_PROFILE")
        );
        assert!(std::env::var_os("MAESTRO_UNRELATED").is_none());
        let result = recover_environment("linux", BTreeMap::new(), || {
            std::fs::read("/proc/self/environ")
        });
        assert_eq!(
            result.get("MAESTRO_ENV_SENTINEL").map(String::as_str),
            Some("a=b")
        );
        let output = NativePackageOperations::new(
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
    let profile = std::env::var_os("LLVM_PROFILE_FILE");
    let output = maestro_test_process::TestProcess::current(
        "package_manager::tests::native_empty_environment_recovery_reads_procfs",
    )
    .unwrap()
    .nocapture()
    .env_clear()
    .envs(
        profile
            .iter()
            .map(|value| ("MAESTRO_EXPECT_PROFILE", value)),
    )
    .case("*")
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
        let result = runtime::run(manager.latest_npm_version("x"));
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
    assert_eq!(
        runtime::run(manager.latest_npm_version("example"))
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
    let result = runtime::run(manager.remote_git_head(path.to_str().unwrap()));
    std::fs::remove_dir_all(path).unwrap();
    assert_eq!(
        result.unwrap_err().to_string(),
        "Failed to determine remote HEAD"
    );
}
