//! Nextest scheduling through the repository configuration.

use std::{fs, path::Path};

use crate::toolchain_support;

/// Measured slow tests start first without changing partition membership or hiding failures.
#[test]
fn measured_priorities_order_tests_without_changing_shards() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"maestro-tooling\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n",
    )
    .unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(
        root.path().join("src/lib.rs"),
        "#[test] fn a_short() {}\nmod browser_author_build {\n\
         #[test] fn browser_author_check_preserves_adjacent_locks() {}\n\
         #[test] fn browser_author_bootstraps_from_empty_build_state() {}\n\
         #[test] fn browser_check_repairs_formatting() { assert!(false); }\n}\n",
    )
    .unwrap();
    let baseline = root.path().join("baseline.toml");
    fs::write(&baseline, "[profile.default]\n").unwrap();
    let configured = root.path().join("configured.toml");
    let mut config: toml::Value =
        toml::from_str(include_str!("../../../.config/nextest.toml")).unwrap();
    config.as_table_mut().unwrap().remove("scripts");
    let profile = config["profile"]["default"].as_table_mut().unwrap();
    profile.remove("scripts");
    profile
        .get_mut("overrides")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .retain(|rule| {
            let filter = rule["filter"].as_str().unwrap();
            filter.contains("package(maestro-tooling)")
                || filter.contains("package(=maestro-tooling)")
        });
    fs::write(&configured, toml::to_string(&config).unwrap()).unwrap();
    let before = run_nextest(root.path(), &baseline, None);
    let after = run_nextest(root.path(), &configured, None);
    assert_eq!(
        after,
        [
            "browser_author_build::browser_check_repairs_formatting",
            "browser_author_build::browser_author_bootstraps_from_empty_build_state",
            "browser_author_build::browser_author_check_preserves_adjacent_locks",
            "a_short",
        ]
    );
    assert_eq!(before[0], "a_short");
    let mut union = Vec::new();
    for shard in 1..=16 {
        let partition = format!("count:{shard}/16");
        let mut before = run_nextest(root.path(), &baseline, Some(&partition));
        let mut after = run_nextest(root.path(), &configured, Some(&partition));
        before.sort();
        after.sort();
        assert_eq!(before, after);
        union.extend(after);
    }
    union.sort();
    let mut expected = after;
    expected.sort();
    assert_eq!(union, expected);
}

/// Run one-worker nextest and retain completion order, including the intentional failure.
fn run_nextest(root: &Path, config: &Path, partition: Option<&str>) -> Vec<String> {
    let mut command = toolchain_support::command("cargo");
    command
        .current_dir(root)
        .args([
            "nextest",
            "run",
            "--offline",
            "--color",
            "never",
            "--test-threads",
            "1",
            "--no-fail-fast",
            "--no-tests",
            "pass",
            "--final-status-level",
            "none",
            "--config-file",
        ])
        .arg(config);
    if let Some(partition) = partition {
        command.args(["--partition", partition]);
    }
    let output = command.output().unwrap();
    let text = String::from_utf8(output.stderr).unwrap();
    let failed = text
        .lines()
        .any(|line| line.trim_start().starts_with("FAIL "));
    if partition.is_none() {
        assert!(failed, "{text}");
    }
    assert_eq!(
        output.status.code(),
        Some(if failed { 100 } else { 0 }),
        "{text}"
    );
    let completed: Vec<_> = text
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if matches!(fields.first(), Some(&"PASS" | &"FAIL")) {
                fields.last().map(|name| (*name).to_owned())
            } else {
                None
            }
        })
        .collect();
    if partition.is_none() {
        assert!(!completed.is_empty(), "{text}");
    }
    completed
}
