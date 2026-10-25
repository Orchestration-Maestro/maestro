//! Link-identity decisions and native Windows relocation witnesses.

use super::relocation::privilege_rename;
#[cfg(windows)]
use super::{PreparedMove, migrate_tools_to_bin, relocate};
#[cfg(windows)]
use std::{fs, os::windows::fs::symlink_dir, path::PathBuf};

#[test]
fn privilege_failure_requires_proven_stable_binding() {
    let privilege = std::io::Error::from_raw_os_error(1314);
    assert!(privilege_rename(&privilege, true));
    assert!(!privilege_rename(&privilege, false));
    assert!(!privilege_rename(
        &std::io::Error::from_raw_os_error(5),
        true
    ));
}

/// Exercise the shared owner with one directory link.
#[cfg(windows)]
fn publish_link(
    path: &std::path::Path,
    target: &std::path::Path,
    referent: &std::path::Path,
) -> std::io::Result<()> {
    if relocate(vec![PreparedMove {
        path: path.into(),
        target: target.into(),
        referent: Some(referent.into()),
    }]) == 1
    {
        Ok(())
    } else {
        Err(std::io::Error::other("retained source"))
    }
}

/// Create an isolated directory for one Windows filesystem witness.
#[cfg(windows)]
fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("maestro-links-{}-{name}", std::process::id()));
    fs::create_dir(&root).unwrap();
    // Settlement compares canonical identities, so inputs use the canonical spelling.
    fs::canonicalize(root).unwrap()
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
    if maestro_test_process::selection().is_child() {
        migrate_tools_to_bin(&mut |_, _| panic!("collision is not a move")).unwrap();
        return;
    }
    let root = scratch("collision");
    fs::create_dir(root.join("tools")).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    fs::create_dir(root.join("original")).unwrap();
    fs::write(root.join("original/payload"), b"retained").unwrap();
    // Windows resolves a relative link target only when it uses its own separator.
    symlink_dir(r"..\original", root.join("tools/fd")).unwrap();
    fs::write(root.join("bin/fd"), b"destination").unwrap();
    let output = maestro_test_process::TestProcess::current(
        "directory_link_collision_removes_only_source_entry",
    )
    .unwrap()
    .nocapture()
    .case("*")
    .env("MAESTRO_CODING_AGENT_DIR", &root)
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(fs::symlink_metadata(root.join("tools/fd")).is_err());
    assert_eq!(fs::read(root.join("bin/fd")).unwrap(), b"destination");
    assert_eq!(
        fs::read(root.join("original/payload")).unwrap(),
        b"retained"
    );
    fs::remove_dir_all(root).unwrap();
}

/// Both managed-binary shapes and session equivalents use the same settlement seam.
#[cfg(unix)]
#[test]
fn moved_absolute_links_retain_readable_sources_until_settlement() {
    for (alias, referent) in [("rg", "rg.exe"), ("fd", "rg"), ("a.jsonl", "b.jsonl")] {
        for obstruction in ["stage", "publication", "referent", "none"] {
            check_absolute_batch(alias, referent, obstruction);
        }
    }
}

/// Exercise real creation, publication and referent-rename failures plus ordinary retry.
#[cfg(unix)]
fn check_absolute_batch(alias: &str, referent: &str, obstruction: &str) {
    use super::{PreparedMove, relocate};
    use std::{fs, os::unix::fs::symlink};
    let root = std::env::temp_dir().join(format!(
        "maestro-settlement-{}-{alias}-{obstruction}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let source = root.join("source");
    let target = root.join("target");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&target).unwrap();
    fs::write(source.join(referent), b"original").unwrap();
    symlink(source.join(referent), source.join(alias)).unwrap();
    let blocker = match obstruction {
        "stage" => source.join(format!("{alias}.migrating")),
        "publication" => target.join(alias),
        "referent" => target.join(referent),
        _ => root.join("unused"),
    };
    if obstruction != "none" {
        fs::create_dir(&blocker).unwrap();
        fs::write(blocker.join("occupied"), b"blocker").unwrap();
    }
    for call in 0..2 {
        let entries = vec![
            PreparedMove {
                path: source.join(alias),
                target: target.join(alias),
                referent: Some(source.join(referent)),
            },
            PreparedMove {
                path: source.join(referent),
                target: target.join(referent),
                referent: None,
            },
        ];
        let count = relocate(entries);
        if call == 0 && obstruction != "none" {
            assert_eq!(count, 0, "{obstruction}");
            assert_retained(&source, &target, (alias, referent), &blocker, obstruction);
            fs::remove_dir_all(&blocker).unwrap();
        } else {
            assert_eq!(count, 2);
            assert_eq!(fs::read(target.join(alias)).unwrap(), b"original");
            assert_eq!(
                fs::read_link(target.join(alias)).unwrap(),
                target.join(referent)
            );
            assert!(fs::symlink_metadata(source.join(alias)).is_err());
            assert!(fs::symlink_metadata(source.join(referent)).is_err());
            break;
        }
    }
    fs::remove_dir_all(root).unwrap();
}

/// Failed batches preserve readable sources and clean only their own links.
#[cfg(unix)]
fn assert_retained(
    source: &std::path::Path,
    target: &std::path::Path,
    names: (&str, &str),
    blocker: &std::path::Path,
    obstruction: &str,
) {
    use std::fs;
    let (alias, referent) = names;
    assert_eq!(fs::read(source.join(alias)).unwrap(), b"original");
    assert_eq!(fs::read(source.join(referent)).unwrap(), b"original");
    assert_eq!(fs::read(blocker.join("occupied")).unwrap(), b"blocker");
    if obstruction != "publication" {
        assert!(fs::symlink_metadata(target.join(alias)).is_err());
    }
    if obstruction != "stage" {
        assert!(fs::symlink_metadata(source.join(format!("{alias}.migrating"))).is_err());
    }
}
