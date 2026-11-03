//! Content-only package resolution through native discovery and skill loading.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::{scratch::Scratch, support};
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions};
use maestro_resources::{LoadSkillsOptions, load_skills};
use std::{path::Path, rc::Rc};

/// Resolve the fixture package as a source, then load the skills it resolved.
#[test]
fn content_package_resolves_only_its_skills() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/content-package");
    let scratch = Scratch::new().unwrap();
    let root = scratch.path().to_str().unwrap();

    let paths = resolve_paths(&package, root);
    let loaded = load_skills(LoadSkillsOptions {
        cwd: root,
        home: root,
        agent_dir: root,
        config_dir_name: ".maestro",
        skill_paths: &paths,
        include_defaults: false,
    })
    .unwrap();

    assert_eq!(paths.len(), 1);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    assert_eq!(loaded.skills.len(), 1);
    assert_eq!(loaded.skills[0].name, "fixture-skill");
}

/// Resolve only local skill resources while observing acquisition and spawning.
fn resolve_paths(package: &Path, root: &str) -> Vec<String> {
    let effects = Rc::new(support::Effects::default());
    let settings = Rc::new(std::cell::RefCell::new(
        maestro_settings::SettingsManager::create(root, root, ".maestro"),
    ));
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: root.into(),
            agent_dir: root.into(),
            settings_manager: settings,
        },
        support::Controlled(Rc::clone(&effects)),
    );
    let resolved = crate::runtime::run(
        manager.resolve_extension_sources(&[package.to_str().unwrap().into()], None),
    )
    .unwrap();
    assert!(resolved.extensions.is_empty());
    assert!(resolved.prompts.is_empty());
    assert!(resolved.themes.is_empty());
    assert!(!resolved.skills.is_empty(), "package must resolve skills");
    assert!(resolved.skills.iter().all(|s| s.enabled));
    assert!(effects.calls.borrow().is_empty());
    assert_eq!(effects.spawns.get(), 0);
    resolved
        .skills
        .into_iter()
        .filter(|s| s.enabled)
        .map(|s| s.path)
        .collect()
}
