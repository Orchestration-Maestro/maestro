//! Native adapter: notification projection and error delivery.
use super::native::{NativeThemeWatchOperations, entry_names};
use super::{ThemeReloadTimer, ThemeWatchOperations, ThemeWatcher};
use crate::theme::{NativeThemeOperations, ThemeDirectories, ThemeState};
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
fn theme_native_events_ignore_reads_and_keep_rename_destinations() {
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
    }
    let rename = event(
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        &["/watched/custom/a.json.tmp", "/watched/custom/a.json"],
    );
    assert_eq!(
        entry_names(&rename, dir),
        names_of(&["a.json.tmp", "a.json"])
    );

    let rescan = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
    assert_eq!(entry_names(&rescan, dir), [None]);
    let no_path = event(EventKind::Modify(ModifyKind::Any), &[]);
    assert_eq!(entry_names(&no_path, dir), [None]);
    for unrelated in ["/watched/custom", "/elsewhere/a.json", "/watched/custom/"] {
        let about = event(EventKind::Remove(RemoveKind::Folder), &[unrelated]);
        assert_eq!(entry_names(&about, dir), [None], "path {unrelated}");
    }
}

/// A watch on a fresh directory, driven by a current-thread runtime and local set.
struct Harness {
    runtime: tokio::runtime::Runtime,
    local: Rc<LocalSet>,
    operations: NativeThemeWatchOperations,
}

impl Harness {
    fn new() -> Self {
        let local = Rc::new(LocalSet::new());
        Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap(),
            operations: NativeThemeWatchOperations::new(Rc::clone(&local)),
            local,
        }
    }
}

#[test]
fn theme_native_relative_root_projects_absolute_event_paths_to_names() {
    let dir = format!("maestro-watch-relative-{}", std::process::id());
    std::fs::create_dir_all(&dir).unwrap();
    let harness = Harness::new();
    let names = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&names);
    let (mut watcher, results) = harness
        .operations
        .open(
            &dir,
            Rc::new(move |name| seen.borrow_mut().push(name)),
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
    harness.local.block_on(&harness.runtime, async {
        while names.borrow().len() < 2 {
            tokio::task::yield_now().await;
        }
    });
    assert_eq!(*names.borrow(), names_of(&["theme.json", "decoy.json"]));
    super::ThemeWatcher::close(&mut watcher).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn theme_native_close_inside_a_callback_stops_queued_notifications() {
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
                    super::ThemeWatcher::close(&mut watcher).unwrap();
                }
            }),
            Rc::new(|| {}),
        )
        .unwrap();
    *slot.borrow_mut() = Some(watcher);
    for file in ["first.json", "second.json"] {
        let path = dir.join(file);
        results
            .send(Ok(event(
                EventKind::Create(CreateKind::File),
                &[path.to_str().unwrap()],
            )))
            .unwrap();
    }
    harness.local.block_on(&harness.runtime, results.closed());
    assert_eq!(delivered.get(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn theme_native_drop_inside_a_callback_stops_queued_notifications() {
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
                drop(held.borrow_mut().take());
            }),
            Rc::new(|| {}),
        )
        .unwrap();
    *slot.borrow_mut() = Some(watcher);
    for file in ["first.json", "second.json"] {
        let path = dir.join(file);
        results
            .send(Ok(event(
                EventKind::Create(CreateKind::File),
                &[path.to_str().unwrap()],
            )))
            .unwrap();
    }
    harness.local.block_on(&harness.runtime, results.closed());
    assert_eq!(delivered.get(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Native effects that keep each watch's result channel so a test can inject results.
struct Injecting {
    /// Native effects that create the watches and timers.
    operations: NativeThemeWatchOperations,
    /// Result channel of the latest watch.
    results: RefCell<Option<tokio::sync::mpsc::UnboundedSender<notify::Result<Event>>>>,
    /// Number of reloads scheduled.
    scheduled: Cell<usize>,
}

impl ThemeWatchOperations for Injecting {
    fn watch(
        &self,
        path: &str,
        listener: Rc<dyn Fn(Option<String>)>,
        on_error: Rc<dyn Fn()>,
    ) -> std::io::Result<Box<dyn ThemeWatcher>> {
        let (watcher, results) = self.operations.open(path, listener, on_error)?;
        *self.results.borrow_mut() = Some(results);
        Ok(Box::new(watcher))
    }

    fn schedule(&self, delay: Duration, callback: Box<dyn FnOnce()>) -> Box<dyn ThemeReloadTimer> {
        self.scheduled.set(self.scheduled.get() + 1);
        self.operations.schedule(delay, callback)
    }
}

#[test]
fn theme_native_error_closes_through_the_state_and_releases_the_listener() {
    let dir = std::env::temp_dir().join(format!("maestro-watch-error-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.json"), "{}").unwrap();
    let harness = Harness::new();
    let injecting = Rc::new(Injecting {
        operations: NativeThemeWatchOperations::new(Rc::clone(&harness.local)),
        results: RefCell::default(),
        scheduled: Cell::new(0),
    });
    let state = Rc::new(ThemeState::new(
        ThemeDirectories {
            themes_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme").to_owned(),
            custom_themes_dir: dir.to_str().unwrap().to_owned(),
        },
        Rc::new(NativeThemeOperations),
    ));
    state.select("a");
    state.start_theme_watcher(Some(Rc::clone(&injecting) as Rc<dyn ThemeWatchOperations>));
    let results = injecting.results.borrow().clone().unwrap();
    results
        .send(Err(notify::Error::generic("simulated failure")))
        .unwrap();
    results
        .send(Ok(event(
            EventKind::Create(CreateKind::File),
            &[dir.join("a.json").to_str().unwrap()],
        )))
        .unwrap();
    harness.local.block_on(&harness.runtime, async {
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
    });
    assert!(results.is_closed());
    assert_eq!(injecting.scheduled.get(), 0);
    drop(state);
    std::fs::remove_dir_all(&dir).unwrap();
}
