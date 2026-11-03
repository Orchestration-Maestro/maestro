//! Ignored-test ownership for native and pinned-browser runs.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// Every ignored test declares its execution owner; browser tests stay in their selected module.
#[test]
fn ignored_tests_keep_known_execution_owners() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let browser = root.join("crates/maestro-tooling/tests/browser.rs");
    let chromium = "#[ignore = \"runs in the pinned Chromium job\"]";
    let controlled = "#[ignore = \"runs only inside a controlled child\"]";
    let mut files = Vec::new();
    rust_sources(&root.join("crates"), &mut files);
    for file in files {
        let source = fs::read_to_string(&file).unwrap();
        for line in source
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("#[ignore"))
        {
            assert!(
                line == chromium || line == controlled,
                "{}: {line}",
                file.display()
            );
            if line == chromium {
                assert_eq!(file, browser, "Chromium tests belong to the browser module");
            }
        }
    }
    let source = fs::read_to_string(&browser).unwrap();
    let tests = source
        .lines()
        .filter(|line| line.trim() == "#[test]")
        .count();
    assert!(tests > 0);
    assert_eq!(
        source
            .lines()
            .filter(|line| line.trim() == chromium)
            .count(),
        tests
    );
    assert!(
        fs::read_to_string(browser.with_file_name("integration.rs"))
            .unwrap()
            .lines()
            .any(|line| line.trim() == "mod browser;")
    );
}

/// Visit authored Rust files without descending into compilation outputs or symbolic links.
fn rust_sources(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        let path = entry.path();
        if kind.is_dir() && entry.file_name() != "target" {
            rust_sources(&path, files);
        } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}
