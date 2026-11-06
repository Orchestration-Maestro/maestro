//! The private index path a pinned checkout's comparison hands to Git.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::{runtime, scratch, support};
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions};
use std::{fs, path::Path};

/// The commit the pin and the fake checkout both report.
const COMMIT: &str = "1234567890abcdef1234567890abcdef12345678";

#[test]
fn pin_inspection_hands_git_the_ordinary_staging_index_path() {
    let scratch = scratch::Scratch::new().unwrap();
    let agent = scratch.path().join("agent");
    let checkout = agent
        .join("git")
        .join("example.test")
        .join("team")
        .join("repo");
    fs::create_dir_all(checkout.join(".git")).unwrap();
    let target = checkout.to_string_lossy().into_owned();
    let (_, settings, effects) = support::manager(&serde_json::json!({}));
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: agent.to_string_lossy().into_owned(),
            agent_dir: agent.to_string_lossy().into_owned(),
            settings_manager: settings,
        },
        support::Controlled(effects.clone()),
    );
    // Each captured probe answers one question the inspection asks this checkout.
    let responses = ["", ".git", ".git", "", COMMIT, COMMIT, "", "", "", ""];
    *effects.capture_outputs.borrow_mut() = responses.map(|text| Ok(text.to_owned())).into();
    let source = format!("https://example.test/team/repo@{COMMIT}");

    let status = runtime::run(manager.verify_pin(&source, None)).unwrap();

    assert!(status.is_clean());
    let index = staged_index(&effects);
    let stage_root = format!(
        "{}{}.maestro-stage-",
        maestro_path::dirname(&target),
        maestro_path::SEP
    );
    assert!(
        Path::new(&index).is_absolute()
            && index.starts_with(&stage_root)
            && index.ends_with(&format!("{}index", maestro_path::SEP)),
        "Git must receive the staging index beside the checkout, spelled as the checkout is: {index}"
    );
}

/// The index path recorded with the commit comparison.
fn staged_index(effects: &support::Effects) -> String {
    let calls = effects.calls.borrow();
    let operands = effects.operands.borrow();
    let environment = calls
        .iter()
        .zip(operands.iter())
        .find_map(|((_, arguments), (_, _, environment))| {
            arguments
                .first()
                .is_some_and(|verb| verb == "read-tree")
                .then_some(environment)
        })
        .expect("inspection compares the commit through a private index");
    environment
        .iter()
        .find_map(|(name, value)| (name == "GIT_INDEX_FILE").then(|| value.clone()))
        .expect("the comparison receives GIT_INDEX_FILE")
}
