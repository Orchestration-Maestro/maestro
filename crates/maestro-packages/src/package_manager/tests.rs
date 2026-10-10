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
        let output = NativePackageOperations::new(|_| false)
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
