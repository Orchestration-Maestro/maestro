//! Labels from supplied native paths.
#![cfg(test)]

#[test]
fn labels_use_supplied_home_checkout_and_path_fallback() {
    use maestro_workspace::workspace::git::{automatic_workspace_label, fallback_label_from_cwd};
    use std::path::Path;
    let home = Path::new("/home/member");
    assert_eq!(fallback_label_from_cwd(home, Some(home)), "~");
    assert_eq!(
        fallback_label_from_cwd(home, Some(Path::new("/elsewhere"))),
        "member"
    );
    assert_eq!(fallback_label_from_cwd(home, None), "member");
    assert_eq!(fallback_label_from_cwd(Path::new(""), None), "");
    assert_eq!(fallback_label_from_cwd(Path::new("/"), None), "/");
    assert_eq!(
        fallback_label_from_cwd(Path::new("/work/日本語"), None),
        "日本語"
    );
    assert_eq!(
        automatic_workspace_label(home, Path::new("/work/checkout"), Some(home)),
        "checkout"
    );
    assert_eq!(
        automatic_workspace_label(home, Path::new("/"), Some(home)),
        "~"
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let native = std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 255]));
        assert_eq!(fallback_label_from_cwd(&native, None), "/�");
    }
}

#[test]
fn native_discovery_preserves_footer_and_workspace_metadata_rules() {
    use super::support::snapshot::Directory;
    use maestro_workspace::workspace::git::native::{DiscoveryPolicy, discover_git_paths_with};
    for scenario in ["unspaced", "empty", "unreadable-common", "directory-head"] {
        let dir = Directory::new();
        let root = dir.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        let marker = root.join(".git");
        if scenario == "unspaced" || scenario == "empty" {
            std::fs::create_dir(dir.path().join("store")).unwrap();
            std::fs::write(dir.path().join("store/HEAD"), "ref: refs/heads/work").unwrap();
            std::fs::write(root.join("HEAD"), "ref: refs/heads/empty").unwrap();
            let target = if scenario == "unspaced" {
                "../store"
            } else {
                ""
            };
            std::fs::write(&marker, format!("gitdir:{target}")).unwrap();
        } else {
            std::fs::create_dir(&marker).unwrap();
            if scenario == "directory-head" {
                std::fs::create_dir(marker.join("HEAD")).unwrap();
            } else {
                std::fs::write(marker.join("HEAD"), "ref: refs/heads/main").unwrap();
            }
            std::fs::create_dir(marker.join("commondir")).unwrap();
        }
        let workspace = discover_git_paths_with(&root, DiscoveryPolicy::Workspace);
        let footer = discover_git_paths_with(&root, DiscoveryPolicy::Footer);
        match scenario {
            "unspaced" => {
                assert_eq!(workspace.unwrap().git_dir, dir.path().join("store"));
                assert!(footer.is_none());
            }
            "empty" => {
                assert_eq!(workspace.unwrap().git_dir, root);
                assert!(footer.is_none());
            }
            _ => {
                if scenario == "directory-head" {
                    assert!(workspace.is_none());
                } else {
                    assert_eq!(workspace.unwrap().git_common_dir, marker);
                }
                let footer = footer.unwrap();
                assert_eq!(footer.git_dir, marker);
                assert_eq!(footer.git_common_dir, marker);
            }
        }
    }
}

#[test]
fn native_worktree_unreadable_common_metadata_keeps_workspace_identity() {
    use super::support::snapshot::Directory;
    use maestro_workspace::workspace::git::native::{DiscoveryPolicy, discover_git_paths_with};
    let dir = Directory::new();
    let root = dir.path().join("repo");
    let metadata = dir.path().join("store");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&metadata).unwrap();
    std::fs::write(root.join(".git"), "gitdir: ../store").unwrap();
    std::fs::write(metadata.join("HEAD"), "ref: refs/heads/work").unwrap();
    std::fs::create_dir(metadata.join("commondir")).unwrap();
    let workspace = discover_git_paths_with(&root, DiscoveryPolicy::Workspace).unwrap();
    assert_eq!(workspace.repo_root, root);
    assert_eq!(workspace.git_dir, metadata);
    assert_eq!(workspace.git_common_dir, metadata);
    assert!(discover_git_paths_with(&root, DiscoveryPolicy::Footer).is_none());
}

/// A directory laid out as a bare repository store with the supplied configuration.
fn bare_store(config: &str) -> (super::support::snapshot::Directory, std::path::PathBuf) {
    let dir = super::support::snapshot::Directory::new();
    let store = dir.path().join("store");
    std::fs::create_dir_all(store.join("objects")).unwrap();
    std::fs::create_dir_all(store.join("refs")).unwrap();
    std::fs::write(store.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::write(store.join("config"), config).unwrap();
    (dir, store)
}

#[test]
fn bare_flag_in_another_section_does_not_make_a_store_bare() {
    use maestro_workspace::workspace::git::native::discover_git_paths;
    let (_dir, store) =
        bare_store("[core]\n\tfilemode = true\n[remote \"origin\"]\n\tbare = true\n");
    assert!(discover_git_paths(&store).is_none());
}

#[test]
fn last_bare_flag_decides_whether_a_store_is_bare() {
    use maestro_workspace::workspace::git::native::discover_git_paths;
    let (_dir, store) = bare_store("[core]\n\tbare = true\n\tbare = false\n");
    assert!(discover_git_paths(&store).is_none());
    let (_dir, store) = bare_store("[core]\n\tbare = false\n\tbare = true\n");
    assert!(discover_git_paths(&store).is_some());
}

#[test]
fn quoted_bare_flag_makes_a_store_bare() {
    use maestro_workspace::workspace::git::native::discover_git_paths;
    let (_dir, store) = bare_store("[core]\n\tbare = \"true\"\n");
    assert!(discover_git_paths(&store).is_some());
}

#[test]
fn last_ref_storage_value_decides_reftable() {
    use maestro_workspace::workspace::git::native::git_ref_storage_is_reftable;
    let (_dir, store) = bare_store("[extensions]\n\trefStorage = files\n\trefStorage = reftable\n");
    assert!(git_ref_storage_is_reftable(&store));
    let (_dir, store) = bare_store("[extensions]\n\trefStorage = reftable\n\trefStorage = files\n");
    assert!(!git_ref_storage_is_reftable(&store));
}
