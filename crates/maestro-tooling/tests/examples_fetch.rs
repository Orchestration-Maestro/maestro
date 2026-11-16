//! The `examples` development command fetches the package repository's example tree.
#![cfg(test)]

use maestro_test_directory::Scratch;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::ExitCode,
};

use crate::tooling_commands::git;

/// Run `examples` in `checkout` against `source`, optionally at `reference`.
fn fetch(checkout: &Path, source: &Path, reference: Option<&str>) -> io::Result<ExitCode> {
    let mut args = vec![OsString::from("examples")];
    args.extend(reference.map(OsString::from));
    args.extend([OsString::from("--source"), source.as_os_str().to_owned()]);
    maestro_tooling::run(&args, Path::new("unused-cargo"), checkout)
}

/// Replace the source's working files with `files`, then commit them.
fn publish(source: &Path, files: &[(&str, &str)]) {
    let examples = source.join("examples");
    if examples.exists() {
        fs::remove_dir_all(&examples).unwrap();
    }
    for (name, content) in files {
        let path = source.join("examples").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fs::write(source.join("README.md"), "not an example").unwrap();
    git(source, &["add", "-A"]);
    git(
        source,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "fixture",
        ],
    );
}

/// A source repository holding the first version of the tree.
fn source() -> Scratch {
    let source = Scratch::new().unwrap();
    git(source.path(), &["init", "-q"]);
    publish(
        source.path(),
        &[
            ("README.md", "index"),
            ("extensions/hello.rs", "hello"),
            ("extensions/old.rs", "old"),
            ("sdk/nested/deep.rs", "deep"),
        ],
    );
    source
}

/// Every file under `root`, relative and slash separated, with its text.
fn tree(root: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, directory: &Path, files: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path: PathBuf = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, files);
            } else {
                let name = path.strip_prefix(base).unwrap();
                let name = name.to_string_lossy().replace('\\', "/");
                files.insert(name, fs::read_to_string(&path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

/// The names beside the checkout's `examples` folder.
fn siblings(checkout: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(checkout)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A first fetch writes a tree equal to the source's `examples/` and nothing else.
#[test]
fn examples_command_writes_the_source_tree() {
    let source = source();
    let checkout = Scratch::new().unwrap();

    assert_eq!(
        fetch(checkout.path(), source.path(), None).unwrap(),
        ExitCode::SUCCESS
    );

    assert_eq!(
        tree(&checkout.path().join("examples")),
        tree(&source.path().join("examples"))
    );
    assert_eq!(siblings(checkout.path()), ["examples"]);
}

/// A rerun after an upstream change adds, changes and removes files to match.
#[test]
fn examples_command_rerun_follows_upstream_changes() {
    let source = source();
    let checkout = Scratch::new().unwrap();
    fetch(checkout.path(), source.path(), None).unwrap();
    publish(
        source.path(),
        &[
            ("README.md", "new index"),
            ("extensions/hello.rs", "hello"),
            ("extensions/added.rs", "added"),
        ],
    );

    fetch(checkout.path(), source.path(), None).unwrap();

    let fetched = tree(&checkout.path().join("examples"));
    assert_eq!(fetched, tree(&source.path().join("examples")));
    assert!(!fetched.contains_key("extensions/old.rs"));
    assert_eq!(siblings(checkout.path()), ["examples"]);
}

/// A reference selects that revision instead of the default branch.
#[test]
fn examples_command_fetches_the_named_reference() {
    let source = source();
    git(source.path(), &["tag", "v1"]);
    publish(source.path(), &[("README.md", "later")]);
    let checkout = Scratch::new().unwrap();

    fetch(checkout.path(), source.path(), Some("v1")).unwrap();

    assert_eq!(
        tree(&checkout.path().join("examples"))["extensions/old.rs"],
        "old"
    );
}

/// A fetch git refuses reports git's error and keeps the previous tree.
#[test]
fn examples_command_failure_keeps_the_previous_tree() {
    let source = source();
    let checkout = Scratch::new().unwrap();
    fetch(checkout.path(), source.path(), None).unwrap();
    let before = tree(&checkout.path().join("examples"));

    let error = fetch(checkout.path(), source.path(), Some("no-such-ref")).unwrap_err();

    assert!(error.to_string().contains("no-such-ref"), "{error}");
    assert_eq!(tree(&checkout.path().join("examples")), before);
    assert_eq!(siblings(checkout.path()), ["examples"]);
}

/// A source without an `examples/` folder fails and keeps the previous tree.
#[test]
fn examples_command_requires_the_examples_folder() {
    let source = source();
    let checkout = Scratch::new().unwrap();
    fetch(checkout.path(), source.path(), None).unwrap();
    let before = tree(&checkout.path().join("examples"));
    fs::remove_dir_all(source.path().join("examples")).unwrap();
    git(source.path(), &["add", "-A"]);
    git(
        source.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "remove",
        ],
    );

    let error = fetch(checkout.path(), source.path(), None).unwrap_err();

    assert!(error.to_string().contains("examples"), "{error}");
    assert_eq!(tree(&checkout.path().join("examples")), before);
}

/// A reference or source that looks like an option is refused before `git` runs.
#[test]
fn examples_command_refuses_option_like_values() {
    let source = source();
    let checkout = Scratch::new().unwrap();

    let reference = fetch(checkout.path(), source.path(), Some("--upload-pack=x")).unwrap_err();
    let args = [OsString::from("examples"), "--source".into(), "-x".into()];
    let location =
        maestro_tooling::run(&args, Path::new("unused-cargo"), checkout.path()).unwrap_err();

    assert!(reference.to_string().contains("usage"), "{reference}");
    assert!(location.to_string().contains("usage"), "{location}");
    assert!(siblings(checkout.path()).is_empty());
}
