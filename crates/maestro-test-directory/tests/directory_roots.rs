//! Public directory ownership qualification.
#![cfg(not(target_arch = "wasm32"))]
use maestro_test_directory::Scratch;
use std::fs;

#[test]
fn chosen_root_is_canonical_and_keeps_written_files() {
    let root = Scratch::new().unwrap();
    assert_eq!(
        root.path().as_os_str(),
        fs::canonicalize(root.path()).unwrap().as_os_str()
    );
    fs::write(root.path().join("kept"), "owned").unwrap();
    let file = root.path().join("kept");
    assert_eq!(
        file.as_os_str(),
        fs::canonicalize(&file).unwrap().as_os_str()
    );
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
    let parent = root.path().join(OsStr::from_bytes(b"native-\xff"));
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
