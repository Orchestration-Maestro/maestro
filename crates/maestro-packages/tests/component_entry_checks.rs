//! Author-built component discovery through the existing package owner.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::component_support;
use crate::scratch;
use crate::support;
use component_support::ExpectedPaths;
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions};
use serde::Deserialize;
use serde_json::Value;
use std::{cell::RefCell, collections::HashSet, fs, rc::Rc};
/// One complete recorded discovery input and result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    input: Input,
    expected: ExpectedPaths,
}
/// Filesystem and settings operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    operation: String,
    files: Vec<File>,
    configured: Option<Value>,
    filtered: Option<Value>,
}
/// A file to create before discovery.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    path: String,
    text: String,
}
/// Replay each selected case once against native files and controlled acquisition.
fn cases(names: &[&str]) {
    let text = include_str!("fixtures/component_entries.json");
    let all: Vec<Case> = serde_json::from_str(text).unwrap();
    let raw: Vec<Value> = serde_json::from_str(text).unwrap();
    let mut ids = HashSet::new();
    let mut inputs = HashSet::new();
    for value in raw {
        assert!(ids.insert(value["id"].to_string()));
        assert!(inputs.insert(value["input"].to_string()));
    }
    let runtime = crate::runtime::runtime();
    for name in names {
        // Automatic discovery reads the home; give each case an empty one of its own.
        let home = |base: &str| vec![("HOME", Some(format!("{base}/home")))];
        if crate::resolution_support::case_in_child(name, home).is_none() {
            continue;
        }
        let case = all.iter().find(|c| c.id == *name).unwrap();
        let dir = scratch::Scratch::new().unwrap();
        let root = dir.path().to_str().unwrap();
        let user = dir.path().join("user");
        setup(case, &dir, &user);
        let settings = Rc::new(RefCell::new(maestro_settings::SettingsManager::create(
            root,
            user.to_str().unwrap(),
            ".maestro",
        )));
        let effects = Rc::new(support::Effects::default());
        let manager = DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: root.into(),
                agent_dir: user.to_str().unwrap().into(),
                settings_manager: Rc::clone(&settings),
            },
            support::Controlled(Rc::clone(&effects)),
        );
        let actual = runtime.block_on(async {
            match case.input.operation.as_str() {
                "explicit" => manager
                    .resolve_extension_sources(&[root.into()], None)
                    .await
                    .unwrap(),
                "configured" | "automatic" | "filtered" => manager.resolve(None).await.unwrap(),
                _ => panic!("unknown operation"),
            }
        });
        case.expected.check(actual, root);
        assert!(effects.calls.borrow().is_empty());
        assert_eq!(effects.spawns.get(), 0);
    }
}
/// Conventional files, indexes, pruning and nonrecursive child entry selection.
#[test]
fn component_files_follow_conventional_entry_rules() {
    cases(&[
        "discovery-top-level",
        "discovery-root-index",
        "discovery-child-index",
        "discovery-mixed",
        "discovery-pruned",
        "discovery-helper-only",
    ]);
}
/// Literal manifests precede indexes while malformed or empty fields fall back.
#[test]
fn component_manifests_keep_literal_selection_and_fallbacks() {
    cases(&[
        "discovery-manifest-wins",
        "discovery-child-manifest",
        "discovery-manifest-empty",
        "discovery-manifest-missing",
        "discovery-manifest-null",
        "discovery-manifest-wrong-shape",
        "discovery-manifest-invalid",
        "discovery-manifest-key-case",
    ]);
}
/// Configured, automatic, expanded and filtered routes share entry discovery.
#[test]
fn component_entries_use_shared_configured_and_manifest_routes() {
    cases(&[
        "discovery-configured-directory",
        "discovery-automatic-scopes",
        "discovery-manifest-glob-directory",
        "discovery-filtered-package",
    ]);
}

/// Create only the recorded filesystem and settings operands.
fn setup(case: &Case, dir: &scratch::Scratch, user: &std::path::Path) {
    let root = dir.path().to_str().unwrap();
    fs::create_dir_all(user).unwrap();
    fs::create_dir_all(dir.path().join("extensions")).unwrap();
    fs::write(dir.path().join(".git"), "gitdir: unused").unwrap();
    for file in &case.input.files {
        let path = dir.path().join(&file.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        if file.text == "@author_component" {
            fs::copy("tests/fixtures/author_component.wasm", path).unwrap();
        } else {
            fs::write(path, &file.text).unwrap();
        }
    }
    for (path, value) in [
        (
            dir.path().join(".maestro/settings.json"),
            &case.input.configured,
        ),
        (user.join("settings.json"), &case.input.filtered),
    ] {
        if let Some(value) = value {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            // The root is written inside JSON strings, where a Windows `\` needs escaping.
            let root = root.replace('\\', "\\\\");
            fs::write(path, value.to_string().replace("$ROOT", &root)).unwrap();
        }
    }
}
