//! Successful path resolution binds polling and Git query paths before the caller drives their task set.
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
fn footer_native_effects_keep_admission_directory_after_cwd_change() {
    const CASE: &str = "footer-cwd-identity";
    if std::env::var("MAESTRO_CHILD_CASE").ok().as_deref() != Some(CASE) {
        let path = std::env::var("PATH").unwrap();
        {
            let output = maestro_test_process::TestProcess::current(
                "footer_native_effects_keep_admission_directory_after_cwd_change",
            )
            .unwrap()
            .nocapture()
            .env_clear()
            .envs([("PATH", &path)].iter().copied())
            .env("MAESTRO_CHILD_CASE", CASE)
            .output()
            .unwrap();
            assert!(output.status.success(), "{output:?}");
        };
        return;
    }
    let first = directory("first", "seven77");
    let second = directory("second", "decoy");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    let ops = NativeFooterOperations::new(local.clone());
    let original_cwd = std::env::current_dir().unwrap();
    for (root, branch) in [(&first, "original"), (&second, "decoy")] {
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet", "--initial-branch", branch])
                .arg(root)
                .status()
                .unwrap()
                .success()
        );
    }
    std::env::set_current_dir(&first).unwrap();
    let (branch_tx, branch_rx) = tokio::sync::oneshot::channel();
    ops.symbolic_ref_async(".", Box::new(move |result| branch_tx.send(result).unwrap()));
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
    let branch = local.block_on(&runtime, async { branch_rx.await.unwrap().unwrap() });
    assert_eq!(branch.as_deref(), Some("original\n"));
    std::env::set_current_dir(original_cwd).unwrap();
    drop(watch);
    drop(ops);
    drop(local);
    for root in [first, second] {
        std::fs::remove_dir_all(root).unwrap();
    }
}
