//! Link-identity decisions and native Windows relocation witnesses.

use super::link_rename_preserves_referent;
use std::path::Path;

#[cfg(windows)]
use super::{migrate_tools_to_bin, publish_link};
#[cfg(windows)]
use std::{fs, os::windows::fs::symlink_dir, path::PathBuf};

#[test]
fn link_rename_preserves_captured_referent() {
    let root = std::env::temp_dir();
    let referent = root.join("agent/tools/lib/program");
    let destination = root.join("agent/bin/fd");
    assert!(link_rename_preserves_referent(
        &referent,
        &destination,
        &referent
    ));
    assert!(link_rename_preserves_referent(
        Path::new("lib/program"),
        &root.join("agent/tools/rg"),
        &referent
    ));
    // The same stored target dangles or selects a decoy after moving to bin.
    assert!(!link_rename_preserves_referent(
        Path::new("lib/program"),
        &destination,
        &referent
    ));
    assert!(!link_rename_preserves_referent(
        Path::new("../tools/lib/program"),
        &destination,
        &referent
    ));
}

/// Create an isolated directory for one Windows filesystem witness.
#[cfg(windows)]
fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("maestro-links-{}-{name}", std::process::id()));
    fs::create_dir(&root).unwrap();
    root
}

#[cfg(windows)]
#[test]
fn directory_link_publication_retires_source() {
    let root = scratch("retirement");
    let referent = root.join("original");
    fs::create_dir(&referent).unwrap();
    fs::write(referent.join("payload"), b"retained").unwrap();
    let source = root.join("source");
    let target = root.join("target");
    symlink_dir("original", &source).unwrap();
    publish_link(&source, &target, &referent).unwrap();
    assert!(fs::symlink_metadata(&source).is_err());
    assert_eq!(fs::read(target.join("payload")).unwrap(), b"retained");
    assert!(fs::symlink_metadata(&target).unwrap().is_symlink());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn failed_directory_link_publication_cleans_staging_and_retries() {
    let root = scratch("retry");
    let referent = root.join("original");
    fs::create_dir(&referent).unwrap();
    fs::write(referent.join("payload"), b"retained").unwrap();
    let source = root.join("source");
    let target = root.join("target");
    symlink_dir("original", &source).unwrap();
    fs::create_dir(&target).unwrap();
    fs::write(target.join("blocker"), b"occupied").unwrap();
    assert!(publish_link(&source, &target, &referent).is_err());
    assert!(fs::symlink_metadata(root.join("source.migrating")).is_err());
    assert_eq!(fs::read(source.join("payload")).unwrap(), b"retained");
    assert_eq!(fs::read(target.join("blocker")).unwrap(), b"occupied");
    fs::remove_dir_all(&target).unwrap();
    publish_link(&source, &target, &referent).unwrap();
    assert!(fs::symlink_metadata(&source).is_err());
    assert_eq!(fs::read(target.join("payload")).unwrap(), b"retained");
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn directory_link_collision_removes_only_source_entry() {
    if std::env::var_os("MAESTRO_LINK_COLLISION_CHILD").is_some() {
        migrate_tools_to_bin(&mut |_, _| panic!("collision is not a move")).unwrap();
        return;
    }
    let root = scratch("collision");
    fs::create_dir(root.join("tools")).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    fs::create_dir(root.join("original")).unwrap();
    fs::write(root.join("original/payload"), b"retained").unwrap();
    symlink_dir("../original", root.join("tools/fd")).unwrap();
    fs::write(root.join("bin/fd"), b"destination").unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!(
                "{}::directory_link_collision_removes_only_source_entry",
                module_path!().split_once("::").unwrap().1
            ),
            "--nocapture",
        ])
        .env("MAESTRO_LINK_COLLISION_CHILD", "1")
        .env("MAESTRO_CODING_AGENT_DIR", &root)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    assert!(fs::symlink_metadata(root.join("tools/fd")).is_err());
    assert_eq!(fs::read(root.join("bin/fd")).unwrap(), b"destination");
    assert_eq!(
        fs::read(root.join("original/payload")).unwrap(),
        b"retained"
    );
    fs::remove_dir_all(root).unwrap();
}
