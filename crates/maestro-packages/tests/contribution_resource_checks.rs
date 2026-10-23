//! Browser contribution files as the fifth package resource kind.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::{manifest_resource_checks::write, scratch, support};
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions};
use std::{path::Path, rc::Rc};

/// Manifest entries and the conventional directory both resolve, ordered and deduplicated.
#[test]
fn contribution_files_are_discovered_from_manifest_and_directory() {
    let scratch = scratch::Scratch::new().unwrap();
    let root = scratch.path().to_string_lossy().into_owned();
    let listed = Path::new(&root).join("listed");
    write(
        &listed.join("package.json"),
        r#"{"maestro":{"contributions":["views/page.json","views/*.json"]}}"#,
    );
    for file in [
        "views/page.json",
        "views/card.json",
        "views/notes.txt",
        "contributions/hidden.json",
    ] {
        write(&listed.join(file), "{}");
    }
    let conventional = Path::new(&root).join("conventional");
    for file in [
        "contributions/b.json",
        "contributions/a.json",
        "contributions/readme.md",
    ] {
        write(&conventional.join(file), "{}");
    }
    let (_, settings, _) = support::manager(&serde_json::json!({}));
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: maestro_path::join(&[&root, "project"]),
            agent_dir: maestro_path::join(&[&root, "agent"]),
            settings_manager: settings,
        },
        support::Controlled(Rc::new(support::Effects::default())),
    );
    let sources = [
        listed.to_string_lossy().into_owned(),
        conventional.to_string_lossy().into_owned(),
        listed.to_string_lossy().into_owned(),
    ];
    let resolved =
        crate::runtime::run(async move { manager.resolve_extension_sources(&sources, None).await })
            .unwrap();
    let found: Vec<(String, bool)> = resolved
        .contributions
        .iter()
        .map(|resource| {
            let relative = Path::new(&resource.path).strip_prefix(&root).unwrap();
            (
                relative.to_string_lossy().replace('\\', "/"),
                resource.enabled,
            )
        })
        .collect();
    let expected = [
        "listed/views/page.json",
        "listed/views/card.json",
        "conventional/contributions/a.json",
        "conventional/contributions/b.json",
    ];
    assert_eq!(found, expected.map(|path| (path.to_owned(), true)));
    assert!(resolved.themes.is_empty() && resolved.extensions.is_empty());
}
