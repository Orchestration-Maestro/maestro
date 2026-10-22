//! The examples folder the default system prompt names.
#![cfg(test)]

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
    assert_eq!(directory_names(root), ["README.md", "extensions", "sdk"]);
    assert_index_lists(root, &directory_names(root));
    for directory in ["extensions", "sdk"] {
        let path = root.join(directory);
        assert_index_lists(&path, &recursive_names(&path, &path));
    }
}

/// Assert an index page names every entry of its folder and only entries that exist.
fn assert_index_lists(folder: &Path, entries: &[String]) {
    let index = fs::read_to_string(folder.join("README.md")).unwrap();
    for name in entries.iter().filter(|name| *name != "README.md") {
        assert!(
            index.contains(name.as_str()),
            "{} index omits {name}",
            folder.display()
        );
    }
    for named in index_entries(&index) {
        assert!(
            folder.join(&named).exists(),
            "{} index names {named}",
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
