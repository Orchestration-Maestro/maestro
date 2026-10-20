//! Installed pin selection ignores unread manifest values.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::scratch;
use crate::support;

use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions, SourceScope};
use std::{fs, rc::Rc};

/// Matching online pins retain resources without reacquisition despite deep unread members.
#[test]
fn matching_online_pin_ignores_deep_unread_member() {
    check_matching_pin(false, "unread");
}

/// Matching offline pins retain resources despite deep unread members.
#[test]
fn matching_offline_pin_ignores_deep_unread_member() {
    check_matching_pin(true, "unread");
}

/// A deep earlier duplicate version is discarded before selecting the matching string.
#[test]
fn matching_pin_selects_last_version_after_deep_duplicate() {
    check_matching_pin(false, "version");
    check_matching_pin(true, "version");
}

/// Resolves installed resources and observes the completed command boundary.
fn check_matching_pin(offline: bool, member: &str) {
    let scratch = scratch::Scratch::new().unwrap();
    let cwd = scratch.0.to_str().unwrap();
    let target = scratch.0.join(".maestro/npm/node_modules/pkg");
    fs::create_dir_all(&target).unwrap();
    let deep = format!("{}0{}", "[".repeat(128), "]".repeat(128));
    fs::write(
        target.join("package.json"),
        format!("{{\"{member}\":{deep},\"version\":\"2.0.0\",\"maestro\":{{\"extensions\":[\"entry.ts\"]}}}}"),
    )
    .unwrap();
    fs::write(target.join("entry.ts"), "installed resource").unwrap();
    let (_, settings, mut effects) = support::manager(&serde_json::json!({}));
    Rc::get_mut(&mut effects).unwrap().native_exists = true;
    *effects.offline.borrow_mut() = offline.then(|| "1".into());
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: cwd.into(),
            agent_dir: scratch.0.join("user").to_str().unwrap().into(),
            settings_manager: settings,
        },
        support::Controlled(Rc::clone(&effects)),
    );
    let result = crate::runtime::run(
        manager.resolve_extension_sources(&["npm:pkg@2.0.0".into()], Some(SourceScope::Project)),
    )
    .unwrap();
    assert_eq!(
        result.extensions.len(),
        1,
        "matching pin retains installed resources"
    );
    assert_eq!(
        result.extensions[0].path,
        target.join("entry.ts").to_str().unwrap()
    );
    assert!(
        effects.calls.borrow().is_empty(),
        "matching pin must not reacquire"
    );
}
