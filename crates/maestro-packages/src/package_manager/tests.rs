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
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let alias = scratch.path().join("sentinel");
        maestro_test_process::install_command(
            &alias,
            r#"{"scenario":"report","fields":[{"variable":"MAESTRO_ENV_SENTINEL"}]}"#,
        )
        .unwrap();
        let output = NativePackageOperations::new(
            std::rc::Rc::new(|| false),
            &std::rc::Rc::new(tokio::task::LocalSet::new()),
        )
        .run_command_sync(&alias.to_string_lossy(), &[])
        .unwrap();
        assert_eq!(output.stdout, "a=b");
        return;
    }
    let profile = std::env::var_os("LLVM_PROFILE_FILE");
    // The prepared fixture stays reachable from a cleared child, so the case can install a
    // stand-in without repeating the artifact path.
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE");
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
    .envs(
        artifact
            .iter()
            .map(|value| ("MAESTRO_NATIVE_FIXTURE", value)),
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
            cwd: maestro_test_directory::temporary_root()
                .to_string_lossy()
                .into_owned(),
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
        (b"\"2\"".to_vec(), Some("2")),
        (" \u{feff}\"2\"\n".as_bytes().to_vec(), Some("2")),
        (Vec::new(), None),
        ("\u{85}\"2\"".as_bytes().to_vec(), None),
        (b"bad".to_vec(), None),
    ] {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let manager = native_manager(answering(&scratch, &output));
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
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let manager = native_manager(answering(&scratch, b"\xef\xbb\xbf"));
    assert_eq!(
        runtime::run(manager.latest_npm_version("example"))
            .unwrap_err()
            .to_string(),
        "Empty response from npm view"
    );
}

/// Settings whose package command prints `stdout` and succeeds.
#[cfg(unix)]
fn answering(
    scratch: &maestro_test_directory::Scratch,
    stdout: &[u8],
) -> maestro_settings::Settings {
    let alias = scratch.path().join("npm");
    maestro_test_process::install_command(
        &alias,
        &serde_json::json!({
            "scenario": "output",
            "stdout": stdout,
            "stderr": [],
            "code": 0
        })
        .to_string(),
    )
    .unwrap();
    maestro_settings::Settings(
        serde_json::json!({"npmCommand": [alias.to_string_lossy()]})
            .as_object()
            .unwrap()
            .clone(),
    )
}

#[test]
#[cfg(unix)]
fn remote_head_rejects_unmatched_head_with_authored_message() {
    let scratch = maestro_test_directory::Scratch::new().expect("test directory should be created");
    let path = scratch.path();
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
                .current_dir(path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap()
                .success()
        );
    }
    let manager = native_manager(maestro_settings::Settings::default());
    let result = runtime::run(manager.remote_git_head(path.to_str().unwrap()));
    assert_eq!(
        result.unwrap_err().to_string(),
        "Failed to determine remote HEAD"
    );
}
