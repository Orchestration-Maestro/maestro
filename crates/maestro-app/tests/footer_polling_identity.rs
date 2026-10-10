//! A relative polling path stays bound to the directory current at registration.
#![cfg(unix)]
#![cfg(test)]
use maestro_app::presentation_data::footer_data_provider::{
    FooterFileStats, FooterOperations, NativeFooterOperations,
};
use std::{path::PathBuf, rc::Rc, time::Duration};

fn directory(name: &str, content: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("maestro-polling-{}-{name}", std::process::id()));
    std::fs::create_dir_all(root.join("refs")).unwrap();
    std::fs::write(root.join("refs/table"), content).unwrap();
    root
}

#[test]
fn footer_polling_keeps_registration_directory_after_cwd_change() {
    let first = directory("first", "seven77");
    let second = directory("second", "decoy");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    let ops = NativeFooterOperations::new(local.clone());
    std::env::set_current_dir(&first).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let watch = ops.watch_file(
        "refs/table",
        Duration::from_millis(250),
        Rc::new(move |now: FooterFileStats, _| tx.send(now).unwrap()),
    );
    std::env::set_current_dir(&second).unwrap();
    std::fs::write(first.join("refs/table"), "changed-original").unwrap();
    let seen = local.block_on(&runtime, async { rx.recv().await.unwrap() });
    assert_eq!(seen.size.to_bits(), 16.0_f64.to_bits());
    drop(watch);
    drop(ops);
    drop(local);
    for root in [first, second] {
        std::fs::remove_dir_all(root).unwrap();
    }
}
