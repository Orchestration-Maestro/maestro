//! Canonical target write exclusion.

use std::fs::{self, OpenOptions, TryLockError};

#[test]
fn write_lock_sits_beside_its_target_and_is_removed_on_release() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let target = root.path().join("settings.json");
    let sidecar = root.path().join("settings.json.lock");
    fs::write(&sidecar, b"left by an exited holder").unwrap();
    let holder = maestro_lock::acquire_write(&target, true).unwrap();
    assert!(sidecar.is_file());
    let independent = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&sidecar)
        .unwrap();
    assert!(matches!(
        independent.try_lock(),
        Err(TryLockError::WouldBlock)
    ));
    drop(independent);
    drop(holder);
    assert!(!sidecar.exists());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn bounded_acquisition_reports_contention_after_ten_attempts() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let target = root.path().join("settings.json");
    let _holder = maestro_lock::acquire_write(&target, true).unwrap();
    assert!(matches!(
        maestro_lock::acquire_write_bounded(&target, true),
        Err(TryLockError::WouldBlock)
    ));
}

#[test]
fn try_acquisition_reports_contention_at_once() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let target = root.path().join("settings.json");
    let _holder = maestro_lock::acquire_write(&target, true).unwrap();
    assert!(matches!(
        maestro_lock::try_acquire_write(&target, true),
        Err(TryLockError::WouldBlock)
    ));
}

#[test]
fn unopenable_lock_file_fails_in_every_form() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let target = root.path().join("settings.json");
    fs::create_dir(root.path().join("settings.json.lock")).unwrap();
    assert!(maestro_lock::try_acquire_write(&target, true).is_err());
    assert!(maestro_lock::acquire_write_bounded(&target, true).is_err());
    assert!(maestro_lock::acquire_write(&target, true).is_err());
}

#[test]
fn acquisition_never_reads_home() {
    if let Some(target) = std::env::var_os("MAESTRO_LOCK_TARGET") {
        maestro_lock::replace(
            std::path::Path::new(&target),
            b"saved",
            maestro_lock::Replace::default(),
        )
        .unwrap();
        return;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let readonly = root.path().join("readonly-home");
    fs::create_dir(&readonly).unwrap();
    #[cfg(unix)]
    let _restore = maestro_test_directory::deny(&readonly, 0o555).unwrap();
    for home in [
        None,
        Some(root.path().join("missing-home")),
        Some(readonly.clone()),
    ] {
        let target = root.path().join("session");
        let mut child = maestro_test_process::TestProcess::current("acquisition_never_reads_home")
            .unwrap()
            .env("MAESTRO_LOCK_TARGET", &target)
            .env_remove("HOME")
            .env_remove("USERPROFILE");
        if let Some(home) = &home {
            child = child.env("HOME", home).env("USERPROFILE", home);
        }
        let output = child.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(fs::read(&target).unwrap(), b"saved");
        assert!(!root.path().join("missing-home").exists());
        assert_eq!(fs::read_dir(&readonly).unwrap().count(), 0);
    }
}

#[cfg(any(unix, windows))]
#[test]
fn symbolic_link_lock_path_is_refused_without_touching_its_target() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let target = root.path().join("settings.json");
    let victim = root.path().join("victim");
    let sidecar = root.path().join("settings.json.lock");
    fs::write(&victim, b"untouched").unwrap();
    let before = fs::metadata(&victim).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&victim, &sidecar).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&victim, &sidecar).unwrap();
    for result in [
        maestro_lock::acquire_write(&target, true).map_err(std::fs::TryLockError::Error),
        maestro_lock::acquire_write_bounded(&target, true),
        maestro_lock::try_acquire_write(&target, true),
    ] {
        let Err(TryLockError::Error(error)) = result else {
            panic!("symbolic lock path accepted");
        };
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
    let after = fs::metadata(&victim).unwrap();
    assert_eq!(after.len(), before.len());
    assert_eq!(after.permissions(), before.permissions());
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
    }
    assert_eq!(fs::read(victim).unwrap(), b"untouched");
    assert!(
        fs::symlink_metadata(sidecar)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(!target.exists());
}
