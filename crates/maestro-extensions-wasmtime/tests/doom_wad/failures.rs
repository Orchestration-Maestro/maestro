//! Player failures cross the command, retained view and replacement boundary.
use super::{loaded, reload};
use std::{cell::Cell, path::Path, rc::Rc};

/// Observes one controlled engine outcome without bypassing the player.
pub(super) async fn run(root: &Path, case: &str) {
    let wad = root.join("case.wad");
    std::fs::write(&wad, case).unwrap();
    let demo = loaded(root, case).await;
    let reloads = Rc::new(Cell::new(0));
    reload::bind_reload(&demo, reloads.clone());
    reload::command(&demo, &wad).await;
    if case != "small-frame" {
        if case == "tick-error" {
            for tail in demo.tails.take() {
                tail.await.unwrap();
            }
        }
        assert!(
            demo.ui
                .notices
                .borrow()
                .iter()
                .any(|notice| notice.starts_with("DOOM stopped:")),
            "{:?}",
            demo.ui.notices.borrow()
        );
        assert_eq!(demo.ui.wad.take(), Some(case.as_bytes().to_vec()));
        reload::command(&demo, &wad).await;
        assert_eq!(
            reloads.get(),
            1,
            "a failed engine requires a fresh component"
        );
        assert!(
            demo.ui.wad.borrow().is_none(),
            "the failed instance cannot reopen"
        );
        assert!(
            std::fs::read_dir(root.join("temporary"))
                .unwrap()
                .next()
                .is_none()
        );
    }
    reload::shutdown(&demo).await;
}
