//! Deterministic publication phases exercised through the native transaction.
use super::*;
use std::cell::RefCell;

/// Per-test-thread observations; no global subscriber or process mutation.
#[derive(Default)]
struct Effects {
    events: Vec<&'static str>,
    fault: Option<(&'static str, io::ErrorKind)>,
    collision: bool,
    temporary_path: Option<PathBuf>,
    #[cfg(unix)]
    temporary_mode: Option<u32>,
}
thread_local! {
    static EFFECTS: RefCell<Effects> = RefCell::default();
}

/// Records an imminent operation and optionally refuses it with a native error.
pub(super) fn effect(phase: &'static str) -> io::Result<()> {
    EFFECTS.with_borrow_mut(|effects| {
        effects.events.push(phase);
        #[cfg(unix)]
        if phase == "write_all" {
            use std::os::unix::fs::PermissionsExt;
            effects.temporary_mode = Some(
                fs::metadata(effects.temporary_path.as_ref().unwrap())?
                    .permissions()
                    .mode()
                    & 0o7777,
            );
        }
        match effects.fault {
            Some((at, kind)) if phase == at => {
                Err(io::Error::new(kind, "controlled operation failure"))
            }
            _ => Ok(()),
        }
    })
}

/// Result, operation transcript, committed bytes and remaining entries.
struct Observation {
    result: io::Result<Committed>,
    events: Vec<&'static str>,
    bytes: Vec<u8>,
    entries: usize,
    temporary_bytes: Vec<Vec<u8>>,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    temporary_mode: Option<u32>,
}

/// Runs one isolated publication with an optional refused operation.
fn run(fault: Option<(&'static str, io::ErrorKind)>) -> Observation {
    run_with_collision(fault, false)
}

/// Optionally places a competing file at the selected temporary pathname.
fn run_with_collision(
    fault: Option<(&'static str, io::ErrorKind)>,
    collision: bool,
) -> Observation {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let dir = scratch.path();
    let target = dir.join("snapshot");
    fs::write(&target, b"old").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    }
    EFFECTS.set(Effects {
        events: vec![],
        fault,
        collision,
        ..Effects::default()
    });
    let result = replace(
        &target,
        b"new",
        Replace {
            through_symlinks: false,
            ..Replace::default()
        },
    );
    let effects = EFFECTS.take();
    let bytes = fs::read(&target).unwrap();
    let entries = fs::read_dir(dir).unwrap().count();
    let temporary_bytes = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path != &target)
        .map(|path| fs::read(path).unwrap())
        .collect();
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(&target).unwrap().permissions().mode() & 0o7777
    };
    Observation {
        result,
        events: effects.events,
        bytes,
        entries,
        temporary_bytes,
        #[cfg(unix)]
        mode,
        #[cfg(unix)]
        temporary_mode: effects.temporary_mode,
    }
}

#[test]
fn replacement_syncs_before_and_after_commit() {
    let observed = run(None);
    assert!(observed.result.unwrap().dir_sync.is_none());
    let committed = ["create", "write_all", "file_sync", "close", "rename"];
    assert_eq!(observed.events, [&committed[..], sync_phases()].concat());
    assert_eq!(observed.bytes, b"new");
    assert_eq!(observed.entries, 1);
}

/// Observes exclusive creation, optionally simulating a competing creator.
pub(super) fn before_create(path: &Path) -> io::Result<()> {
    effect("create")?;
    EFFECTS.with_borrow_mut(|effects| effects.temporary_path = Some(path.to_path_buf()));
    if EFFECTS.with_borrow(|effects| effects.collision) {
        fs::write(path, b"competitor").unwrap();
    }
    Ok(())
}

/// Observes that the temporary handle is closed before publication.
pub(super) fn closed() {
    effect("close").unwrap();
}

#[test]
fn replacement_errors_before_commit_keep_old_bytes() {
    for phase in ["create", "write_all", "file_sync", "rename"] {
        let observed = run(Some((phase, io::ErrorKind::Other)));
        assert_eq!(observed.result.unwrap_err().kind(), io::ErrorKind::Other);
        assert_eq!(observed.events.last(), Some(&phase));
        assert_eq!(observed.bytes, b"old");
        assert_eq!(observed.entries, 1);
        #[cfg(unix)]
        assert_eq!(observed.mode, 0o640);
    }
    let observed = run_with_collision(None, true);
    assert_eq!(
        observed.result.unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(observed.bytes, b"old");
    assert_eq!(observed.entries, 2);
    assert_eq!(observed.temporary_bytes, [b"competitor".to_vec()]);
    let dir = std::env::temp_dir().join(format!("maestro-rename-directory-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let target = dir.join("existing");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("retained"), b"old").unwrap();
    assert!(replace(&target, b"new", Replace::default()).is_err());
    assert_eq!(fs::read(target.join("retained")).unwrap(), b"old");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}

/// The directory-sync phases this host reaches. Windows refuses to open a directory as
/// a file, so the sync after the open never runs there and the refusal is tolerated.
fn sync_phases() -> &'static [&'static str] {
    if File::open(".").is_ok() {
        &["parent_open", "parent_sync"]
    } else {
        &["parent_open"]
    }
}

#[test]
fn committed_directory_sync_failure_is_reported_as_success() {
    for &phase in sync_phases() {
        let observed = run(Some((phase, io::ErrorKind::Other)));
        assert_eq!(
            observed.result.unwrap().dir_sync.unwrap().kind(),
            io::ErrorKind::Other
        );
        assert_eq!(observed.events.last(), Some(&phase));
        assert_eq!(observed.bytes, b"new");
        assert_eq!(observed.entries, 1);
    }
}

#[test]
fn unsupported_directory_sync_is_tolerated() {
    for &phase in sync_phases() {
        for kind in [
            io::ErrorKind::Unsupported,
            io::ErrorKind::InvalidInput,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::Other,
        ] {
            let observed = run(Some((phase, kind)));
            let committed = observed.result.unwrap();
            assert_eq!(
                committed.dir_sync.map(|err| err.kind()),
                (kind == io::ErrorKind::Other).then_some(kind)
            );
            assert_eq!(observed.bytes, b"new");
            assert_eq!(observed.entries, 1);
        }
    }
}

#[cfg(unix)]
#[test]
fn root_link_resolution_reaches_final_target() {
    let dir = std::env::temp_dir().join(format!("maestro-resolver-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    for i in 0..17 {
        let next = if i == 16 {
            "target".into()
        } else {
            format!("link-{}", i + 1)
        };
        std::os::unix::fs::symlink(next, dir.join(format!("link-{i}"))).unwrap();
    }
    let actual = resolve_write_target(&dir.join("link-0")).unwrap();
    assert_eq!(actual, dir.join("target"));
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn replacement_preserves_existing_permissions_before_publication() {
    let observed = run(None);
    observed.result.unwrap();
    assert_eq!(observed.temporary_mode.unwrap() & !0o640, 0);
    assert_eq!(observed.mode, 0o640);
}

#[cfg(unix)]
#[test]
fn creation_mode_is_private_before_publication() {
    use std::os::unix::fs::PermissionsExt;
    if launch_umask_cases() {
        return;
    }
    let dir = maestro_test_directory::Scratch::new().unwrap();
    let target = dir.path().join("private");
    let control = dir.path().join("control");
    fs::write(&control, b"control").unwrap();
    let restrictive = std::env::var("MAESTRO_REPLACE_UMASK").unwrap() == "077";
    assert_eq!(
        fs::metadata(control).unwrap().permissions().mode() & 0o777,
        if restrictive { 0o600 } else { 0o666 }
    );
    let mode = if restrictive { 0o640 } else { 0o600 };
    EFFECTS.set(Effects::default());
    replace(
        &target,
        b"secret",
        Replace {
            creation_mode: Some(mode),
            ..Replace::default()
        },
    )
    .unwrap();
    assert_eq!(EFFECTS.take().temporary_mode, Some(0o600));
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        mode
    );
    assert_eq!(fs::read(&target).unwrap(), b"secret");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    replace(
        &target,
        b"rewritten",
        Replace {
            creation_mode: Some(0o777),
            ..Replace::default()
        },
    )
    .unwrap();
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
    fs::remove_file(&target).unwrap();
    assert_failed_creation_cleans_up(dir.path(), &target);
}

/// Checks cleanup before publication without an existing target.
#[cfg(unix)]
fn assert_failed_creation_cleans_up(parent: &Path, target: &Path) {
    for phase in ["write_all", "file_sync", "rename"] {
        EFFECTS.set(Effects {
            fault: Some((phase, io::ErrorKind::Other)),
            ..Effects::default()
        });
        let result = replace(
            target,
            b"secret",
            Replace {
                creation_mode: Some(0o600),
                ..Replace::default()
            },
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Other);
        let effects = EFFECTS.take();
        assert_eq!(effects.temporary_mode, Some(0o600));
        assert!(!target.exists());
        assert_eq!(fs::read_dir(parent).unwrap().count(), 1);
    }
}

/// Runs permission observations in checked children with caller-selected umasks.
#[cfg(unix)]
fn launch_umask_cases() -> bool {
    use maestro_test_process::TestProcess;
    if std::env::var_os("MAESTRO_REPLACE_UMASK").is_some() {
        return false;
    }
    for mask in ["000", "077"] {
        let output = TestProcess::current("creation_mode_is_private_before_publication")
            .unwrap()
            .shell_setup("/bin/sh", format!("umask {mask}"))
            .env("MAESTRO_REPLACE_UMASK", mask)
            .single_threaded()
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    true
}

#[cfg(windows)]
#[test]
fn creation_mode_maps_only_owner_write_to_windows_readonly() {
    let dir = maestro_test_directory::Scratch::new().unwrap();
    let target = dir.path().join("created");
    for mode in [0o600, 0o400] {
        EFFECTS.set(Effects::default());
        replace(
            &target,
            b"secret",
            Replace {
                creation_mode: Some(mode),
                ..Replace::default()
            },
        )
        .unwrap();
        let mut permissions = fs::metadata(&target).unwrap().permissions();
        assert_eq!(permissions.readonly(), mode & 0o200 == 0);
        assert_eq!(fs::read(&target).unwrap(), b"secret");
        if permissions.readonly() {
            assert!(replace(&target, b"new", Replace::default()).is_err());
            assert_eq!(fs::read(&target).unwrap(), b"secret");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        }
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "clears the Windows read-only attribute; cfg(windows) excludes Unix world-writable permissions"
        )]
        permissions.set_readonly(false);
        fs::set_permissions(&target, permissions).unwrap();
        fs::remove_file(&target).unwrap();
    }
    EFFECTS.set(Effects {
        fault: Some(("rename", io::ErrorKind::Other)),
        ..Effects::default()
    });
    assert!(
        replace(
            &target,
            b"secret",
            Replace {
                creation_mode: Some(0o400),
                ..Replace::default()
            }
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    EFFECTS.take();
}
