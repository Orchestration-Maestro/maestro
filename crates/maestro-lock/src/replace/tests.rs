//! Deterministic publication phases exercised through the native transaction.
use super::*;
use std::{cell::RefCell, path::PathBuf};

/// Per-test-thread observations; no global subscriber or process mutation.
#[derive(Default)]
struct Effects {
    events: Vec<&'static str>,
    fault: Option<(&'static str, io::ErrorKind)>,
    collision: bool,
    before_sync: Option<Box<dyn FnOnce()>>,
    temporary_path: Option<PathBuf>,
    #[cfg(unix)]
    temporary_mode: Option<u32>,
}
thread_local! {
    static EFFECTS: RefCell<Effects> = RefCell::default();
    static ROOT: maestro_test_directory::Scratch = maestro_test_directory::Scratch::new().unwrap();
}

/// Keeps each unit-test process's recovery namespace inside a disposable root.
pub(crate) fn root() -> PathBuf {
    std::env::var_os("MAESTRO_TEST_LOCK_ROOT").map_or_else(
        || ROOT.with(|root| root.path().join(".maestro/state/file-locks")),
        PathBuf::from,
    )
}

/// Records an imminent operation and optionally refuses it with a native error.
pub(crate) fn effect(phase: &'static str) -> io::Result<()> {
    if phase == "target_sync" {
        let callback = EFFECTS.with_borrow_mut(|effects| effects.before_sync.take());
        if let Some(callback) = callback {
            callback();
        }
    }
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
    let _ = root();
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
    let temporary_bytes = effects
        .temporary_path
        .as_ref()
        .filter(|path| path.exists())
        .map(|path| vec![fs::read(path).unwrap()])
        .unwrap_or_default();
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
    let committed = [
        "create",
        "write_all",
        "file_sync",
        "close",
        "rename",
        "recovery_sync",
        "target_write",
        "target_sync",
    ];
    assert_eq!(
        observed.events,
        [
            &["ancestry_sync", "recovery_sync"][..],
            &committed[..],
            sync_phases(),
            &["recovery_sync"]
        ]
        .concat()
    );
    assert_eq!(observed.bytes, b"new");
    assert_eq!(observed.entries, 1);
}

/// Observes exclusive creation, optionally simulating a competing creator.
pub(crate) fn before_create(path: &Path) -> io::Result<()> {
    effect("create")?;
    EFFECTS.with_borrow_mut(|effects| effects.temporary_path = Some(path.to_path_buf()));
    if EFFECTS.with_borrow(|effects| effects.collision) {
        fs::write(path, b"competitor").unwrap();
    }
    Ok(())
}

/// Observes that the temporary handle is closed before publication.
pub(crate) fn closed() {
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
    assert_eq!(observed.entries, 1);
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
    if launch_umask_cases("creation_mode_is_private_before_publication") {
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
fn launch_umask_cases(test: &str) -> bool {
    use maestro_test_process::TestProcess;
    if std::env::var_os("MAESTRO_REPLACE_UMASK").is_some() {
        return false;
    }
    for mask in ["000", "077"] {
        let output = TestProcess::current(test)
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

#[test]
fn interrupted_publication_is_replayed_before_the_next_write() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let target = root.path().join("session");
    fs::write(&target, b"before").unwrap();
    EFFECTS.set(Effects {
        fault: Some(("target_write", io::ErrorKind::Interrupted)),
        ..Effects::default()
    });
    assert_eq!(
        replace(&target, b"intended", Replace::default())
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(fs::read(&target).unwrap(), b"before");
    EFFECTS.take();
    drop(crate::acquire_write(&target, true).unwrap());
    assert_eq!(fs::read(&target).unwrap(), b"intended");
    assert_eq!(
        EFFECTS.take().events[..2],
        ["ancestry_sync", "recovery_sync"]
    );
}

#[test]
fn recovery_refuses_a_record_for_another_target() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let target = scratch.path().join("session");
    fs::write(&target, b"before").unwrap();
    EFFECTS.set(Effects {
        fault: Some(("target_write", io::ErrorKind::Interrupted)),
        ..Effects::default()
    });
    assert!(replace(&target, b"intended", Replace::default()).is_err());
    EFFECTS.take();
    let recovery = fs::read_dir(root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "recovery")
        })
        .unwrap();
    fs::write(&recovery, [0; 37]).unwrap();
    assert_eq!(
        crate::acquire_write(&target, true).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(fs::read(&target).unwrap(), b"before");
}

#[test]
fn recovery_completes_a_target_interrupted_before_sync() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let target = scratch.path().join("session");
    fs::write(&target, b"before").unwrap();
    EFFECTS.set(Effects {
        fault: Some(("target_sync", io::ErrorKind::Interrupted)),
        ..Effects::default()
    });
    assert_eq!(
        replace(&target, b"intended", Replace::default())
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
    EFFECTS.take();
    fs::write(&target, b"partial").unwrap();
    drop(crate::acquire_write(&target, true).unwrap());
    assert_eq!(fs::read(&target).unwrap(), b"intended");
}

#[test]
fn target_parent_sync_failure_keeps_the_recovery_copy() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let target = scratch.path().join("session");
    fs::write(&target, b"before").unwrap();
    EFFECTS.set(Effects {
        fault: Some(("parent_open", io::ErrorKind::Other)),
        ..Effects::default()
    });
    assert!(
        replace(&target, b"intended", Replace::default())
            .unwrap()
            .dir_sync
            .is_some()
    );
    EFFECTS.take();
    assert!(fs::read_dir(root()).unwrap().any(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|extension| extension == "recovery")
    }));
    fs::write(&target, b"partial").unwrap();
    drop(crate::acquire_write(&target, true).unwrap());
    assert_eq!(fs::read(&target).unwrap(), b"intended");
}

#[cfg(unix)]
#[test]
fn interrupted_private_creation_replays_its_requested_mode() {
    new_target_mode(Some(0o600), true);
}

#[cfg(unix)]
#[test]
fn new_read_only_target_finishes_with_the_requested_mode() {
    new_target_mode(Some(0o400), false);
    new_target_mode(Some(0o400), true);
}

#[cfg(unix)]
#[test]
fn new_target_and_replay_keep_umask_default_permissions() {
    if launch_umask_cases("new_target_and_replay_keep_umask_default_permissions") {
        return;
    }
    new_target_mode(None, false);
    new_target_mode(None, true);
}

/// Checks both ordinary and interrupted new-file permission selection.
#[cfg(unix)]
fn new_target_mode(mode: Option<u32>, interrupted: bool) {
    use std::os::unix::fs::PermissionsExt;
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let target = scratch.path().join("session");
    let control = scratch.path().join("control");
    fs::write(&control, b"").unwrap();
    let default = fs::metadata(&control).unwrap().permissions().mode() & 0o777;
    EFFECTS.set(Effects {
        fault: interrupted.then_some(("target_write", io::ErrorKind::Interrupted)),
        ..Effects::default()
    });
    let result = replace(
        &target,
        b"intended",
        Replace {
            creation_mode: mode,
            ..Replace::default()
        },
    );
    if interrupted {
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        assert!(!target.exists());
        EFFECTS.take();
        drop(crate::acquire_write(&target, true).unwrap());
    } else {
        result.unwrap();
    }
    EFFECTS.take();
    assert_eq!(fs::read(&target).unwrap(), b"intended");
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        mode.unwrap_or(default)
    );
}

#[test]
fn acquisition_syncs_recovery_absence_before_admitting_a_writer() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let target = scratch.path().join("session");
    fs::write(&target, b"before").unwrap();
    EFFECTS.set(Effects {
        fault: Some(("recovery_sync", io::ErrorKind::Other)),
        ..Effects::default()
    });
    assert_eq!(
        crate::acquire_write(&target, true).unwrap_err().kind(),
        io::ErrorKind::Other
    );
    EFFECTS.take();
    assert_eq!(fs::read(&target).unwrap(), b"before");
}

#[test]
fn first_use_persists_directory_ancestry_before_target_write() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let target = scratch.path().join("session");
    fs::write(&target, b"before").unwrap();
    EFFECTS.set(Effects {
        fault: Some(("ancestry_sync", io::ErrorKind::Other)),
        ..Effects::default()
    });
    assert!(replace(&target, b"intended", Replace::default()).is_err());
    let events = EFFECTS.take().events;
    assert_eq!(events, ["ancestry_sync"]);
    assert_eq!(fs::read(&target).unwrap(), b"before");
    replace(&target, b"intended", Replace::default()).unwrap();
    let events = EFFECTS.take().events;
    assert!(
        events.iter().position(|p| *p == "ancestry_sync").unwrap()
            < events.iter().position(|p| *p == "target_write").unwrap()
    );
}

#[cfg(unix)]
#[test]
fn completed_readonly_creation_recovers_after_parent_sync_failure() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let target = scratch.path().join("session");
    EFFECTS.set(Effects {
        fault: Some(("parent_open", io::ErrorKind::Other)),
        ..Effects::default()
    });
    assert!(
        replace(
            &target,
            b"intended",
            Replace {
                creation_mode: Some(0o400),
                ..Replace::default()
            }
        )
        .unwrap()
        .dir_sync
        .is_some()
    );
    EFFECTS.take();
    // A completed recovery must not need a second writable open, even as root.
    EFFECTS.set(Effects {
        fault: Some(("target_write", io::ErrorKind::PermissionDenied)),
        ..Effects::default()
    });
    drop(crate::acquire_write(&target, true).unwrap());
    EFFECTS.take();
    assert_eq!(fs::read(&target).unwrap(), b"intended");
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o400
    );
    assert!(!fs::read_dir(root()).unwrap().any(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "recovery")
    }));
    if let Some(_restore) = maestro_test_directory::deny(&target, 0o400).unwrap() {
        assert!(replace(&target, b"different", Replace::default()).is_err());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn append_waits_at_target_sync_until_publication_finishes() {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
    };
    if let Ok(path) = std::env::var("MAESTRO_SYNC_APPEND") {
        let mut stream =
            TcpStream::connect(std::env::var("MAESTRO_SYNC_ADDRESS").unwrap()).unwrap();
        stream.write_all(&std::process::id().to_be_bytes()).unwrap();
        let holder = crate::acquire_write(Path::new(&path), true).unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(holder.path())
            .unwrap()
            .write_all(b"after")
            .unwrap();
        stream.write_all(b"done").unwrap();
        return;
    }
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let target = scratch.path().join("session");
    fs::write(&target, b"before").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let child = maestro_test_process::TestProcess::current(
        "append_waits_at_target_sync_until_publication_finishes",
    )
    .unwrap()
    .env("MAESTRO_SYNC_APPEND", &target)
    .env("MAESTRO_TEST_LOCK_ROOT", root())
    .env(
        "MAESTRO_SYNC_ADDRESS",
        listener.local_addr().unwrap().to_string(),
    );
    let (sender, receiver) = std::sync::mpsc::channel();
    EFFECTS.set(Effects {
        before_sync: Some(Box::new(move || {
            let child = child.spawn().unwrap();
            let (mut stream, _) = listener.accept().unwrap();
            let mut pid = [0; 4];
            stream.read_exact(&mut pid).unwrap();
            let blocked = sync_waiter(u32::from_be_bytes(pid), &stream);
            sender.send((child, blocked)).unwrap();
        })),
        ..Effects::default()
    });
    replace(&target, b"rewritten", Replace::default()).unwrap();
    EFFECTS.take();
    let (child, blocked) = receiver.recv().unwrap();
    assert!(child.wait_with_output().unwrap().status.success());
    assert!(blocked, "append escaped while target sync was pending");
    assert_eq!(fs::read(&target).unwrap(), b"rewrittenafter");
}

/// Observes the native waiter while the target synchronization remains paused.
#[cfg(target_os = "linux")]
fn sync_waiter(pid: u32, stream: &std::net::TcpStream) -> bool {
    stream.set_nonblocking(true).unwrap();
    loop {
        let locks = fs::read_to_string("/proc/locks").unwrap();
        if locks.lines().any(|line| {
            line.contains("->") && line.split_whitespace().any(|word| word == pid.to_string())
        }) {
            break true;
        }
        match stream.peek(&mut [0; 1]) {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::yield_now();
            }
            result => {
                result.unwrap();
                break false;
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn writable_creation_replays_even_when_unsynced_bytes_and_mode_match() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let target = scratch.path().join("session");
    EFFECTS.set(Effects {
        fault: Some(("target_sync", io::ErrorKind::Interrupted)),
        ..Effects::default()
    });
    assert!(
        replace(
            &target,
            b"intended",
            Replace {
                creation_mode: Some(0o600),
                ..Replace::default()
            }
        )
        .is_err()
    );
    EFFECTS.take();
    drop(crate::acquire_write(&target, true).unwrap());
    let events = EFFECTS.take().events;
    assert!(events.contains(&"target_write"));
    assert!(events.contains(&"target_sync"));
    assert_eq!(fs::read(&target).unwrap(), b"intended");
}
