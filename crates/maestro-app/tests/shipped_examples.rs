//! The examples folder the default system prompt names.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::configuration::config;
use std::fs;
use std::path::Path;

/// Resolve the folder through the running process's package directory.
#[test]
fn examples_folder_matches_the_shipped_files() {
    let package = config::get_package_dir().unwrap();
    assert!(
        Path::new(&package).join("Cargo.toml").is_file(),
        "{package}"
    );
    assert!(
        Path::new(&package).join("crates/maestro-app").is_dir(),
        "{package}"
    );
    let folder = config::get_examples_path().unwrap();
    let root = Path::new(&folder);
    assert!(root.is_dir(), "{folder}");
    let entries = directory_names(root);
    assert_eq!(entries, ["README.md", "extensions", "sdk"]);
    assert_index_lists(root, &entries);
    for directory in ["extensions", "sdk"] {
        let path = root.join(directory);
        assert_index_lists(&path, &recursive_names(&path, &path));
    }
}

/// Assert an index page lists every entry of its folder and names only paths that exist.
fn assert_index_lists(folder: &Path, entries: &[String]) {
    let index = fs::read_to_string(folder.join("README.md")).unwrap();
    let listed = index_entries(&index);
    for name in entries.iter().filter(|name| *name != "README.md") {
        let target = if folder.join(name).is_dir() {
            format!("{name}/")
        } else {
            name.clone()
        };
        assert!(
            listed.contains(&target),
            "{} index omits {target}",
            folder.display()
        );
    }
    for target in listed {
        assert!(
            folder.join(&target).exists(),
            "{} index names {target}",
            folder.display()
        );
    }
}

/// List the files an index page points at, through a relative link or a table row.
fn index_entries(index: &str) -> Vec<String> {
    let mut entries: Vec<String> = index
        .split("](")
        .skip(1)
        .filter_map(|part| part.split_once(')'))
        .map(|(target, _)| target)
        .filter(|target| !target.contains("://"))
        .map(str::to_owned)
        .collect();
    entries.extend(
        index
            .lines()
            .filter(|line| line.starts_with('|'))
            .filter_map(|row| row.split('`').nth(1).map(str::to_owned)),
    );
    entries
}

/// Both rejection branches run against disposable files, without changing shipped inputs.
#[test]
fn shipped_examples_index_rejects_unlisted_and_missing_files() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    fs::write(root.join("hello.rs"), "").unwrap();
    fs::write(root.join("README.md"), "# Examples\n").unwrap();
    let unlisted = std::panic::catch_unwind(|| assert_index_lists(root, &["hello.rs".to_owned()]));
    assert!(unlisted.is_err(), "an unlisted shipped file must fail");
    fs::write(root.join("README.md"), "| `missing.rs` |\n").unwrap();
    let missing = std::panic::catch_unwind(|| assert_index_lists(root, &[]));
    assert!(missing.is_err(), "a listed missing file must fail");
    fs::write(root.join("README.md"), "| `hello.rs` |\n").unwrap();
    assert_index_lists(root, &["hello.rs".to_owned()]);
}

/// List one directory's entry names in name order.
fn directory_names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// List file names below a directory, relative to its root, in name order.
fn recursive_names(root: &Path, directory: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            names.extend(recursive_names(root, &path));
        } else {
            names.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    names.sort();
    names
}
