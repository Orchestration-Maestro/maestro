//! Public directory ownership qualification.
#![cfg(not(target_arch = "wasm32"))]
use maestro_test_directory::Scratch;
use std::fs;

#[test]
fn chosen_root_is_canonical_and_keeps_written_files() {
    let root = Scratch::new().unwrap();
    assert_eq!(root.path().as_os_str(), canonical(root.path()).unwrap());
    fs::write(root.path().join("kept"), "owned").unwrap();
    let file = root.path().join("kept");
    assert_eq!(file.as_os_str(), canonical(&file).unwrap());
    assert_eq!(
        fs::read_to_string(root.path().join("kept")).unwrap(),
        "owned"
    );
}

/// The canonical spelling of `path`, which on Windows drops the verbatim drive prefix.
fn canonical(path: &std::path::Path) -> std::io::Result<std::ffi::OsString> {
    let canonical = fs::canonicalize(path)?.into_os_string();
    #[cfg(windows)]
    let canonical = canonical
        .to_string_lossy()
        .strip_prefix(r"\\?\")
        .map_or_else(|| canonical.clone(), Into::into);
    Ok(canonical)
}

#[test]
fn chosen_root_accepts_slash_joined_children() {
    let root = Scratch::new().unwrap();
    fs::write(format!("{}/kept", root.path().display()), "owned").unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("kept")).unwrap(),
        "owned"
    );
}

#[test]
fn supplied_parent_keeps_components_and_existing_contents() {
    let root = Scratch::new().unwrap();
    let parent = root.path().join(".");
    fs::write(root.path().join("sibling"), "untouched").unwrap();
    for parent in [root.path(), parent.as_path()] {
        let child = Scratch::new_in(parent).unwrap();
        let expected = parent.join(child.path().file_name().unwrap());
        assert_eq!(child.path().as_os_str(), expected.as_os_str());
        assert_eq!(fs::read_dir(child.path()).unwrap().count(), 0);
        fs::write(child.path().join("payload"), "kept").unwrap();
        assert_eq!(
            fs::read_to_string(child.path().join("payload")).unwrap(),
            "kept"
        );
        drop(child);
    }
    let relative = Scratch::new_in(std::path::Path::new(".")).unwrap();
    assert!(!relative.path().is_absolute());
    fs::write(relative.path().join("relative"), "relative").unwrap();
    assert_eq!(
        fs::read_to_string(relative.path().join("relative")).unwrap(),
        "relative"
    );
    drop(relative);
    assert_eq!(
        fs::read_to_string(root.path().join("sibling")).unwrap(),
        "untouched"
    );
}

#[test]
fn parallel_owners_keep_distinct_live_trees() {
    let root = Scratch::new().unwrap();
    let gate = std::sync::Barrier::new(4);
    let trees = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..4)
            .map(|index| {
                let (parent, gate) = (root.path(), &gate);
                scope.spawn(move || {
                    let child = Scratch::new_in(parent).unwrap();
                    fs::write(child.path().join("value"), index.to_string()).unwrap();
                    gate.wait();
                    (child, index)
                })
            })
            .collect();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    let paths: std::collections::BTreeSet<_> = trees.iter().map(|(tree, _)| tree.path()).collect();
    assert_eq!(paths.len(), 4);
    for (tree, index) in &trees {
        assert_eq!(
            fs::read_to_string(tree.path().join("value")).unwrap(),
            index.to_string()
        );
    }
}

#[test]
fn drop_removes_only_the_owned_tree() {
    let root = Scratch::new().unwrap();
    let first = Scratch::new_in(root.path()).unwrap();
    let second = Scratch::new_in(root.path()).unwrap();
    fs::create_dir(first.path().join("nested")).unwrap();
    fs::write(first.path().join("nested/file"), "first").unwrap();
    fs::write(second.path().join("sentinel"), "second").unwrap();
    let removed = first.path().to_path_buf();
    drop(first);
    assert!(!removed.exists());
    assert_eq!(
        fs::read_to_string(second.path().join("sentinel")).unwrap(),
        "second"
    );
    fs::remove_dir_all(second.path()).unwrap();
    drop(second);
}

#[test]
fn invalid_parents_return_native_errors_without_creating_trees() {
    let root = Scratch::new().unwrap();
    let missing = root.path().join("missing");
    assert_eq!(
        Scratch::new_in(&missing).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(!missing.exists());
    let file = root.path().join("file");
    fs::write(&file, "sentinel").unwrap();
    let expected = fs::create_dir(file.join("probe")).unwrap_err().kind();
    assert_eq!(Scratch::new_in(&file).unwrap_err().kind(), expected);
    assert_eq!(fs::read_to_string(file).unwrap(), "sentinel");
}

#[cfg(unix)]
#[test]
fn linked_supplied_parent_keeps_its_authored_spelling() {
    let root = Scratch::new().unwrap();
    let target = root.path().join("target");
    let link = root.path().join("link");
    fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let child = Scratch::new_in(&link).unwrap();
    assert_eq!(child.path().parent().unwrap().as_os_str(), link.as_os_str());
    assert_ne!(
        child.path().as_os_str(),
        fs::canonicalize(child.path()).unwrap().as_os_str()
    );
}

#[cfg(unix)]
#[test]
fn supplied_parent_keeps_non_unicode_path_bytes() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let root = Scratch::new().unwrap();
    let name = OsStr::from_bytes(b"native-\xff");
    if !root.keeps_distinct(&[name]).unwrap() {
        eprintln!(
            "the parent needs a host that stores the name bytes {:?}",
            name.as_bytes()
        );
        return;
    }
    let parent = root.path().join(name);
    fs::create_dir(&parent).unwrap();
    let child = Scratch::new_in(&parent).unwrap();
    assert_eq!(
        child.path().parent().unwrap().as_os_str(),
        parent.as_os_str()
    );
    fs::write(child.path().join("file"), "value").unwrap();
    assert_eq!(
        fs::read_to_string(child.path().join("file")).unwrap(),
        "value"
    );
}

/// A denial is reported only when this process is really refused, and the earlier
/// permissions come back either way. A privileged run takes the `None` arms.
#[cfg(unix)]
#[test]
fn denials_match_what_this_process_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let root = Scratch::new().unwrap();
    let directory = root.path().join("directory");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("entry"), "value").unwrap();
    let file = root.path().join("file");
    fs::write(&file, "value").unwrap();
    let mode = |path: &std::path::Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let (directory_mode, file_mode) = (mode(&directory), mode(&file));
    let created = directory.join("created");
    let checks: [(&std::path::Path, u32, &dyn Fn() -> bool); 4] = [
        (&directory, 0o555, &|| fs::create_dir(&created).is_ok()),
        (&directory, 0o311, &|| fs::read_dir(&directory).is_ok()),
        (&directory, 0o600, &|| {
            fs::metadata(directory.join("entry")).is_ok()
        }),
        (&file, 0o444, &|| {
            fs::OpenOptions::new().append(true).open(&file).is_ok()
        }),
    ];
    for (path, denied_mode, allowed) in checks {
        let earlier = fs::metadata(path).unwrap().permissions();
        fs::set_permissions(path, fs::Permissions::from_mode(denied_mode)).unwrap();
        let allowed = allowed();
        fs::set_permissions(path, earlier).unwrap();
        let _ = fs::remove_dir(&created);
        let guard = maestro_test_directory::deny(path, denied_mode).unwrap();
        assert_eq!(
            guard.is_some(),
            !allowed,
            "{} {denied_mode:o}",
            path.display()
        );
        if guard.is_some() {
            assert_eq!(mode(path), denied_mode);
        }
        drop(guard);
    }
    assert_eq!((mode(&directory), mode(&file)), (directory_mode, file_mode));
}
