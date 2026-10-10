//! Native adapter: notification projection and error delivery.
use super::native::{NativeThemeWatchOperations, entry_names};
use notify::event::{
    AccessKind, CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind, RenameMode,
};
use notify::{Event, EventKind};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::task::LocalSet;

/// An event of `kind` about the given paths.
fn event(kind: EventKind, paths: &[&str]) -> Event {
    let mut event = Event::new(kind);
    event.paths = paths.iter().map(PathBuf::from).collect();
    event
}

fn names(names: &[&str]) -> Vec<Option<String>> {
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
        assert_eq!(changed, names(&["a.json"]), "kind {kind:?}");
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
    assert_eq!(entry_names(&rename, dir), names(&["a.json.tmp", "a.json"]));

    let rescan = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
    assert_eq!(entry_names(&rescan, dir), [None]);
    let no_path = event(EventKind::Modify(ModifyKind::Any), &[]);
    assert_eq!(entry_names(&no_path, dir), [None]);
    for unrelated in ["/watched/custom", "/elsewhere/a.json", "/watched/custom/"] {
        let about = event(EventKind::Remove(RemoveKind::Folder), &[unrelated]);
        assert_eq!(entry_names(&about, dir), [None], "path {unrelated}");
    }
}

#[test]
fn theme_native_registered_watch_survives_error() {
    let dir = std::env::temp_dir().join(format!("maestro-watch-error-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let local = Rc::new(LocalSet::new());
    let operations = NativeThemeWatchOperations::new(Rc::clone(&local));
    let listened = Rc::new(Cell::new(0));
    let failed = Rc::new(Notify::new());
    let (count, signal) = (Rc::clone(&listened), Rc::clone(&failed));
    let (mut watcher, sink) = operations
        .open(
            dir.to_str().unwrap(),
            Rc::new(move |_| count.set(count.get() + 1)),
            Rc::new(move || signal.notify_one()),
        )
        .unwrap();

    local.block_on(&runtime, async {
        sink.deliver(Err(notify::Error::generic(
            "simulated operating-system failure",
        )));
        tokio::time::timeout(Duration::from_secs(30), failed.notified())
            .await
            .unwrap();
    });
    assert_eq!(listened.get(), 0);
    super::ThemeWatcher::close(&mut watcher).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
