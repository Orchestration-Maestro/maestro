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
    let mut entries = directory_names(root);
    entries.sort();
    assert_eq!(entries, ["README.md", "extensions", "sdk"]);
    let index = fs::read_to_string(root.join("README.md")).unwrap();
    for name in ["extensions/", "sdk/"] {
        assert!(index.contains(name), "index omits {name}");
    }
    for (directory, shipped) in [
        ("extensions", &["README.md", "author_component.rs"][..]),
        ("sdk", &["README.md"][..]),
    ] {
        let path = root.join(directory);
        let mut files = recursive_names(&path, &path);
        files.sort();
        assert_eq!(files, shipped, "{directory} holds unshipped files");
        let index = fs::read_to_string(path.join("README.md")).unwrap();
        for file in files.iter().filter(|file| *file != "README.md") {
            assert!(index.contains(file), "{directory} index omits {file}");
        }
    }
}

/// List one directory's entry names.
fn directory_names(directory: &Path) -> Vec<String> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

/// List file names below a directory, relative to its root.
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
    names
}
