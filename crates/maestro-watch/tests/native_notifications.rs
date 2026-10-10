//! Real directory notifications survive replacement of a watched entry.
#![cfg(not(target_arch = "wasm32"))]
use maestro_watch::fs_watch::{NativeWatchOperations, WatchOperations};
use std::{fs, rc::Rc};
use tokio::{sync::mpsc::unbounded_channel, task::LocalSet};

#[test]
fn native_notifications_report_writes_and_atomic_replacement() {
    let root = std::env::temp_dir().join(format!(
        "maestro-watch-notifications-{}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let directory = root.join("watched");
    fs::create_dir(&directory).unwrap();
    let selected = directory.join("selected.json");
    fs::write(&selected, "original").unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let local = Rc::new(LocalSet::new());
    local.block_on(&runtime, async {
        let operations = NativeWatchOperations::new(Rc::clone(&local));
        let (sent, mut received) = unbounded_channel();
        let read = selected.clone();
        let mut watcher = operations
            .watch(
                directory.to_str().unwrap(),
                Rc::new(move |name| {
                    if name.as_deref() == Some("selected.json") {
                        sent.send((name, fs::read_to_string(&read).unwrap()))
                            .unwrap();
                    }
                }),
                Rc::new(|| panic!("native watch failed")),
            )
            .unwrap();
        fs::write(&selected, "edited").unwrap();
        for expected in ["edited", "replaced", "edited-after-replacement"] {
            let (name, bytes) = matching(&mut received, expected).await;
            assert_eq!(name.as_deref(), Some("selected.json"));
            assert_eq!(bytes, expected);
            if expected == "edited" {
                fs::write(root.join("incoming.json"), "replaced").unwrap();
                fs::rename(root.join("incoming.json"), &selected).unwrap();
            } else if expected == "replaced" {
                fs::write(&selected, "edited-after-replacement").unwrap();
            }
        }
        watcher.close().unwrap();
        while received.recv().await.is_some() {}
    });
    fs::remove_dir_all(root).unwrap();
}

/// Await a notification that observed the selected contents.
#[cfg(test)]
async fn matching(
    received: &mut tokio::sync::mpsc::UnboundedReceiver<(Option<String>, String)>,
    expected: &str,
) -> (Option<String>, String) {
    loop {
        let observed = received.recv().await.unwrap();
        if observed.1 == expected {
            return observed;
        }
    }
}
