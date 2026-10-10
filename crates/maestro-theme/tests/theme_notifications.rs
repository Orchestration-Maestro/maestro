//! Native directory notifications reload the selected custom theme file.
#![cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod live;

use live::{Ops, Scratch, custom_json, state};
use maestro_theme::{NativeThemeWatchOperations, ThemeState, ThemeWatchOperations};
use std::fs;
use std::rc::Rc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::task::LocalSet;

/// Wait until the published theme has the given authored name.
#[cfg(test)]
async fn published(state: &ThemeState, changed: &Notify, name: &str) {
    let live = state.theme();
    tokio::time::timeout(Duration::from_secs(30), async {
        while live.get().unwrap().name() != Some(name) {
            changed.notified().await;
        }
    })
    .await
    .unwrap();
}

#[test]
fn theme_native_notification_reloads_selected_file() {
    let scratch = Scratch::new("native");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    scratch.write("custom/a.json", &custom_json("a", "#112233").to_string());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let local = Rc::new(LocalSet::new());
    let effects: Rc<dyn ThemeWatchOperations> =
        Rc::new(NativeThemeWatchOperations::new(Rc::clone(&local)));
    let changed = Rc::new(Notify::new());
    let signal = Rc::clone(&changed);
    state.on_theme_change(Rc::new(move || {
        signal.notify_one();
        Ok(())
    }));

    local.block_on(&runtime, async {
        state.init_theme(Some("a"), Some(effects)).unwrap();
        scratch.write(
            "custom/a.json",
            &custom_json("edited", "#445566").to_string(),
        );
        published(&state, &changed, "edited").await;

        scratch.write(
            "incoming/a.json",
            &custom_json("replaced", "#778899").to_string(),
        );
        fs::rename(
            scratch.path("incoming/a.json"),
            scratch.path("custom/a.json"),
        )
        .unwrap();
        published(&state, &changed, "replaced").await;
        state.stop_theme_watcher();
    });
    assert_eq!(
        state.theme().get().unwrap().source_path(),
        Some(scratch.path("custom/a.json").as_str())
    );
}
