//! Native saves retain file identity and yield to unrelated local work.
use std::{cell::Cell, rc::Rc, task::Context};

#[cfg(unix)]
#[test]
fn native_save_preserves_existing_identity_and_hard_links() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = scratch.path().join("saved");
    let alias = scratch.path().join("alias");
    std::fs::write(&path, b"previous").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    std::fs::hard_link(&path, &alias).unwrap();
    let before = std::fs::metadata(&path).unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    let io = maestro_app::extension_native_io::extension_native_io(&local);

    crate::support::run(async {
        assert_eq!(
            io.save_file(path.display().to_string(), vec![0, 255], 0o600)
                .await
                .unwrap(),
            None
        );
    });

    let after = std::fs::metadata(&path).unwrap();
    assert_eq!(std::fs::read(&alias).unwrap(), [0, 255]);
    assert_eq!(std::fs::read(&path).unwrap(), [0, 255]);
    assert_eq!(
        (
            after.ino(),
            after.uid(),
            after.gid(),
            after.permissions().mode()
        ),
        (
            before.ino(),
            before.uid(),
            before.gid(),
            before.permissions().mode()
        )
    );
}

#[test]
fn native_save_waits_for_blocking_pool_without_blocking_local_work() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = scratch.path().join("saved");
    let (release, wait_for_release) = tokio::sync::oneshot::channel::<()>();
    let (started, ready) = tokio::sync::oneshot::channel();
    let blocker = runtime.spawn_blocking(move || {
        started.send(()).unwrap();
        let _ = wait_for_release.blocking_recv();
    });
    local.block_on(&runtime, async {
        ready.await.unwrap();
        let io = maestro_app::extension_native_io::extension_native_io(&local);
        let mut save = io.save_file(path.display().to_string(), vec![0, 255], 0o600);
        let pending = save
            .as_mut()
            .poll(&mut Context::from_waker(std::task::Waker::noop()))
            .is_pending();
        let progressed = Rc::new(Cell::new(false));
        let observed = Rc::clone(&progressed);
        local
            .spawn_local(async move { observed.set(true) })
            .await
            .unwrap();
        let untouched = !path.exists();
        drop(release);
        blocker.await.unwrap();
        assert!(progressed.get(), "unrelated local work did not run");
        assert!(pending, "native save bypassed the occupied blocking pool");
        assert!(untouched, "filesystem work ran on the local executor");
        assert_eq!(save.await.unwrap(), None);
        assert_eq!(std::fs::read(&path).unwrap(), [0, 255]);
    });
}
