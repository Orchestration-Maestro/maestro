//! Native adapter: notification projection and error delivery.
use super::native::{NativeWatchOperations, entry_names};
use super::{FsWatcher, WatchOperations};
use notify::event::{
    AccessKind, CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind, RenameMode,
};
use notify::{Event, EventKind};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use tokio::task::LocalSet;

/// An event of `kind` about the given paths.
fn event(kind: EventKind, paths: &[&str]) -> Event {
    let mut event = Event::new(kind);
    event.paths = paths.iter().map(PathBuf::from).collect();
    event
}

fn names_of(names: &[&str]) -> Vec<Option<String>> {
    names.iter().map(|name| Some((*name).to_owned())).collect()
}

#[test]
fn native_events_ignore_reads_and_keep_rename_destinations() {
    let dir = Path::new("/watched/custom");
    let kinds = [
        EventKind::Create(CreateKind::File),
        EventKind::Modify(ModifyKind::Data(DataChange::Any)),
        EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
        EventKind::Remove(RemoveKind::File),
        EventKind::Modify(ModifyKind::Name(RenameMode::From)),
        EventKind::Modify(ModifyKind::Name(RenameMode::To)),
    ];
    for kind in kinds {
        let changed = entry_names(&event(kind, &["/watched/custom/a.json"]), dir);
        assert_eq!(changed, names_of(&["a.json"]), "kind {kind:?}");
    }
    for kind in [
        AccessKind::Read,
        AccessKind::Open(notify::event::AccessMode::Any),
    ] {
        let read = event(EventKind::Access(kind), &["/watched/custom/a.json"]);
        assert!(entry_names(&read, dir).is_empty(), "kind {kind:?}");
        assert!(entry_names(&read.set_flag(notify::event::Flag::Rescan), dir).is_empty());
    }
    let rename = event(
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        &["/watched/custom/a.json.tmp", "/watched/custom/a.json"],
    );
    assert_eq!(
        entry_names(&rename, dir),
        names_of(&["a.json.tmp", "a.json"])
    );

    let repeated = event(
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        &[
            "/watched/custom/b.json",
            "/watched/custom/a.json",
            "/watched/custom/b.json",
        ],
    );
    assert_eq!(
        entry_names(&repeated, dir),
        names_of(&["b.json", "a.json", "b.json"])
    );
    let mixed = event(
        EventKind::Other,
        &[
            "/watched/custom/a.json",
            "/elsewhere/a.json",
            "/watched/custom/b.json",
        ],
    );
    assert_eq!(
        entry_names(&mixed, dir),
        [Some("a.json".into()), None, Some("b.json".into())]
    );
    unknown_events(dir);
}

/// A watch on a fresh directory, driven by a current-thread runtime and local set.
struct Harness {
    runtime: tokio::runtime::Runtime,
    local: Rc<LocalSet>,
    operations: NativeWatchOperations,
}

impl Harness {
    #[cfg(test)]
    fn new() -> Self {
        let local = Rc::new(LocalSet::new());
        Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap(),
            operations: NativeWatchOperations::new(Rc::clone(&local)),
            local,
        }
    }
}

#[test]
fn native_relative_root_projects_absolute_event_paths_to_names() {
    let dir = format!("maestro-watch-relative-{}", std::process::id());
    std::fs::create_dir_all(&dir).unwrap();
    let harness = Harness::new();
    let names = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&names);
    let (completed, completion) = tokio::sync::oneshot::channel();
    let completed = RefCell::new(Some(completed));
    let (mut watcher, results) = harness
        .operations
        .open(
            &dir,
            Rc::new(move |name| {
                seen.borrow_mut().push(name);
                if seen.borrow().len() == 2 {
                    completed.borrow_mut().take().unwrap().send(()).unwrap();
                }
            }),
            Rc::new(|| {}),
        )
        .unwrap();
    let root = std::path::absolute(&dir).unwrap();
    for file in ["theme.json", "decoy.json"] {
        let path = root.join(file);
        results
            .send(Ok(event(
                EventKind::Create(CreateKind::File),
                &[path.to_str().unwrap()],
            )))
            .unwrap();
    }
    harness
        .local
        .block_on(&harness.runtime, completion)
        .unwrap();
    assert_eq!(*names.borrow(), names_of(&["theme.json", "decoy.json"]));
    super::FsWatcher::close(&mut watcher).unwrap();
    harness.local.block_on(&harness.runtime, closed(&results));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn native_close_inside_callback_stops_queued_notifications() {
    let dir = std::env::temp_dir().join(format!("maestro-watch-close-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let harness = Harness::new();
    let slot: Rc<RefCell<Option<super::native::NativeWatcher>>> = Rc::default();
    let delivered = Rc::new(Cell::new(0));
    let (close, count) = (Rc::clone(&slot), Rc::clone(&delivered));
    let (watcher, results) = harness
        .operations
        .open(
            dir.to_str().unwrap(),
            Rc::new(move |_| {
                count.set(count.get() + 1);
                let closing = close.borrow_mut().take();
                if let Some(mut watcher) = closing {
                    super::FsWatcher::close(&mut watcher).unwrap();
                }
            }),
            Rc::new(|| {}),
        )
        .unwrap();
    *slot.borrow_mut() = Some(watcher);
    results
        .send(Ok(event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            &[
                dir.join("first.json").to_str().unwrap(),
                dir.join("within-event.json").to_str().unwrap(),
            ],
        )))
        .unwrap();
    {
        let path = dir.join("later.json");
        results
            .send(Ok(event(
                EventKind::Create(CreateKind::File),
                &[path.to_str().unwrap()],
            )))
            .unwrap();
    }
    harness.local.block_on(&harness.runtime, closed(&results));
    assert_eq!(delivered.get(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn native_drop_inside_callback_stops_queued_notifications() {
    let dir = std::env::temp_dir().join(format!("maestro-watch-drop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let harness = Harness::new();
    let slot: Rc<RefCell<Option<super::native::NativeWatcher>>> = Rc::default();
    let delivered = Rc::new(Cell::new(0));
    let (held, count) = (Rc::clone(&slot), Rc::clone(&delivered));
    let (watcher, results) = harness
        .operations
        .open(
            dir.to_str().unwrap(),
            Rc::new(move |_| {
                count.set(count.get() + 1);
                let dropping = held.borrow_mut().take();
                drop(dropping);
            }),
            Rc::new(|| {}),
        )
        .unwrap();
    *slot.borrow_mut() = Some(watcher);
    results
        .send(Ok(event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            &[
                dir.join("first.json").to_str().unwrap(),
                dir.join("within-event.json").to_str().unwrap(),
            ],
        )))
        .unwrap();
    for file in ["first.json", "second.json"] {
        let path = dir.join(file);
        results
            .send(Ok(event(
                EventKind::Create(CreateKind::File),
                &[path.to_str().unwrap()],
            )))
            .unwrap();
    }
    harness.local.block_on(&harness.runtime, closed(&results));
    assert_eq!(delivered.get(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn native_error_callback_can_close_and_releases_listener() {
    let dir = std::env::temp_dir().join(format!("maestro-watch-error-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let harness = Harness::new();
    let slot: Rc<RefCell<Option<super::native::NativeWatcher>>> = Rc::default();
    let errors = Rc::new(Cell::new(0));
    let delivered = Rc::new(Cell::new(0));
    let (held, count, seen) = (Rc::clone(&slot), Rc::clone(&errors), Rc::clone(&delivered));
    let (first, first_done) = tokio::sync::oneshot::channel();
    let first = RefCell::new(Some(first));
    let listener_alive = Rc::new(());
    let weak_listener = Rc::downgrade(&listener_alive);
    let (watcher, results) = harness
        .operations
        .open(
            dir.to_str().unwrap(),
            Rc::new(move |_| {
                let _ = &listener_alive;
                seen.set(seen.get() + 1);
            }),
            Rc::new(move || {
                count.set(count.get() + 1);
                if count.get() == 1 {
                    first.borrow_mut().take().unwrap().send(()).unwrap();
                } else {
                    let closing = held.borrow_mut().take();
                    closing.unwrap().close().unwrap();
                }
            }),
        )
        .unwrap();
    *slot.borrow_mut() = Some(watcher);
    results
        .send(Err(notify::Error::generic("first failure")))
        .unwrap();
    harness
        .local
        .block_on(&harness.runtime, first_done)
        .unwrap();
    assert!(slot.borrow().is_some());
    assert!(weak_listener.upgrade().is_some());
    results
        .send(Err(notify::Error::generic("second failure")))
        .unwrap();
    results
        .send(Ok(event(
            EventKind::Create(CreateKind::File),
            &[dir.join("later.json").to_str().unwrap()],
        )))
        .unwrap();
    harness.local.block_on(&harness.runtime, closed(&results));
    assert_eq!(errors.get(), 2);
    assert_eq!(delivered.get(), 0);
    assert!(slot.borrow().is_none());
    assert!(weak_listener.upgrade().is_none());
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn native_registration_teardown_leaves_other_watch_active() {
    let dir =
        std::env::temp_dir().join(format!("maestro-watch-independent-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let harness = Harness::new();
    let (observed, mut observations) = tokio::sync::mpsc::unbounded_channel();
    let (b, b_results) = harness
        .operations
        .open(
            dir.to_str().unwrap(),
            Rc::new(move |name| {
                observed.send(name).unwrap();
            }),
            Rc::new(|| {}),
        )
        .unwrap();
    for dropping in [false, true] {
        let (mut a, a_results) = harness
            .operations
            .open(
                dir.to_str().unwrap(),
                Rc::new(|_| panic!("closed registration delivered")),
                Rc::new(|| {}),
            )
            .unwrap();
        if dropping {
            drop(a);
        } else {
            a.close().unwrap();
            a.close().unwrap();
        }
        harness.local.block_on(&harness.runtime, async {
            closed(&a_results).await;
            b_results
                .send(Ok(event(
                    EventKind::Create(CreateKind::File),
                    &[dir.join("survives.json").to_str().unwrap()],
                )))
                .unwrap();
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), observations.recv())
                    .await
                    .expect("other registration still delivers")
                    .unwrap(),
                Some("survives.json".into())
            );
        });
    }
    drop(b);
    harness.local.block_on(&harness.runtime, closed(&b_results));
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn native_timers_obey_deadlines_and_per_handle_teardown() {
    let local = Rc::new(LocalSet::new());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .start_paused(true)
        .build()
        .unwrap();
    let operations = NativeWatchOperations::new(Rc::clone(&local));
    local.block_on(&runtime, async {
        let calls = Rc::new(Cell::new(0));
        let (sent, mut received) = tokio::sync::oneshot::channel();
        let counted = Rc::clone(&calls);
        let mut keeper = operations.schedule(
            Duration::from_millis(100),
            Box::new(move || {
                counted.set(counted.get() + 1);
                sent.send(()).unwrap();
            }),
        );
        let (zero, mut zero_done) = tokio::sync::oneshot::channel();
        let zero_timer = operations.schedule(
            Duration::ZERO,
            Box::new(move || {
                zero.send(()).unwrap();
            }),
        );
        assert_eq!(
            zero_done.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        );
        assert_eq!(calls.get(), 0);
        // A queued barrier runs after the timer tasks have registered their sleeps.
        local.spawn_local(async {}).await.unwrap();
        cancelled_timers(&operations, &local).await;
        zero_done.await.unwrap();
        tokio::time::advance(Duration::from_millis(99)).await;
        local.spawn_local(async {}).await.unwrap();
        assert_eq!(
            received.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        );
        assert_eq!(calls.get(), 0);
        tokio::time::advance(Duration::from_millis(1)).await;
        received.await.unwrap();
        assert_eq!(calls.get(), 1);
        keeper.cancel();
        tokio::time::advance(Duration::from_millis(100)).await;
        local.spawn_local(async {}).await.unwrap();
        assert_eq!(calls.get(), 1);
        drop(keeper);
        drop(zero_timer);
    });
}

/// Observe receiver teardown with a bounded failure diagnostic.
#[cfg(test)]
async fn closed(results: &tokio::sync::mpsc::UnboundedSender<notify::Result<Event>>) {
    tokio::time::timeout(Duration::from_secs(2), results.closed())
        .await
        .expect("receiver teardown completion");
}

/// Unknown names retain their positions rather than inventing a filename.
#[cfg(test)]
fn unknown_events(dir: &Path) {
    let rescan = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
    assert_eq!(entry_names(&rescan, dir), [None]);
    let no_path = event(EventKind::Modify(ModifyKind::Any), &[]);
    assert_eq!(entry_names(&no_path, dir), [None]);
    for unrelated in ["/watched/custom", "/elsewhere/a.json", "/watched/custom/"] {
        let about = event(EventKind::Remove(RemoveKind::Folder), &[unrelated]);
        assert_eq!(entry_names(&about, dir), [None], "path {unrelated}");
    }
}

/// Cancellation and drop release pending callbacks independently.
#[cfg(test)]
async fn cancelled_timers(operations: &NativeWatchOperations, local: &LocalSet) {
    let (cancelled, cancel_done) = tokio::sync::oneshot::channel();
    let mut cancelled_timer = operations.schedule(
        Duration::from_millis(50),
        Box::new(move || {
            let _ = cancelled.send(());
        }),
    );
    let (dropped, drop_done) = tokio::sync::oneshot::channel();
    let dropped_timer = operations.schedule(
        Duration::from_millis(50),
        Box::new(move || {
            let _ = dropped.send(());
        }),
    );
    local.spawn_local(async {}).await.unwrap();
    cancelled_timer.cancel();
    cancelled_timer.cancel();
    drop(dropped_timer);
    assert!(cancel_done.await.is_err());
    assert!(drop_done.await.is_err());
    drop(cancelled_timer);
}
