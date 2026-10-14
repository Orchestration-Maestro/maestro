//! Integration-test packaging through the workspace checker.
#![cfg(test)]

mod support;

use std::io::Write;

use maestro_test_conventions::check_workspace;
use support::Workspace;

fn workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-models"]);
    let root = workspace.root.join("crates/maestro-models");
    let manifest = root.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        text.replace("[package]", "[package]\nautotests = false"),
    )
    .unwrap();
    writeln!(
        std::fs::OpenOptions::new()
            .append(true)
            .open(manifest)
            .unwrap(),
        "\n[[test]]\nname = \"integration\"\npath = \"tests/integration.rs\""
    )
    .unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(root.join("tests/integration.rs"), "").unwrap();
    workspace
}

#[test]
fn unlisted_test_file_is_rejected() {
    let workspace = workspace();
    let tests = workspace.root.join("crates/maestro-models/tests");
    std::fs::write(tests.join("added.rs"), "").unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.contains("added.rs"), "{error}");
    std::fs::write(tests.join("integration.rs"), "mod added;\n").unwrap();
    check_workspace(&workspace.root).unwrap();
}

fn rewrite(workspace: &Workspace, from: &str, to: &str) {
    let path = workspace.root.join("crates/maestro-models/Cargo.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from));
    std::fs::write(path, text.replace(from, to)).unwrap();
}

#[test]
fn listed_test_files_share_one_target() {
    let workspace = workspace();
    let tests = workspace.root.join("crates/maestro-models/tests");
    std::fs::create_dir_all(tests.join("support")).unwrap();
    std::fs::write(tests.join("support/helper.rs"), "").unwrap();
    std::fs::write(tests.join("case.rs"), "#![cfg(unix)]\n").unwrap();
    std::fs::write(tests.join("integration.rs"), "mod case;\n").unwrap();
    check_workspace(&workspace.root).unwrap();
}

#[test]
fn automatic_test_discovery_is_rejected() {
    for replacement in ["", "autotests = true"] {
        let workspace = workspace();
        rewrite(&workspace, "autotests = false", replacement);
        assert!(
            check_workspace(&workspace.root)
                .unwrap_err()
                .contains("autotests")
        );
    }
    check_workspace(&workspace().root).unwrap();
}

#[test]
fn missing_or_extra_test_targets_are_rejected() {
    let target = "[[test]]\nname = \"integration\"\npath = \"tests/integration.rs\"";
    for replacement in [
        String::new(),
        format!("{target}\n[[test]]\nname = \"extra\"\npath = \"tests/integration.rs\""),
    ] {
        let workspace = workspace();
        rewrite(&workspace, target, &replacement);
        assert!(check_workspace(&workspace.root).is_err());
    }
    check_workspace(&workspace().root).unwrap();
}

#[test]
fn nonstandard_test_root_is_rejected() {
    for (from, to) in [
        ("name = \"integration\"", "name = \"other\""),
        ("tests/integration.rs", "tests/other.rs"),
    ] {
        let workspace = workspace();
        rewrite(&workspace, from, to);
        assert!(check_workspace(&workspace.root).is_err());
    }
    for contents in [None, Some("mod {")] {
        let workspace = workspace();
        let root = workspace
            .root
            .join("crates/maestro-models/tests/integration.rs");
        std::fs::write(root.with_file_name("case.rs"), "").unwrap();
        if let Some(contents) = contents {
            std::fs::write(&root, contents).unwrap();
        } else {
            std::fs::remove_file(&root).unwrap();
        }
        assert!(check_workspace(&workspace.root).is_err());
    }
    check_workspace(&workspace().root).unwrap();
}

#[test]
fn nonexecuting_test_target_is_rejected() {
    for option in [
        "harness = false",
        "test = false",
        "required-features = [\"gated\"]",
    ] {
        let workspace = workspace();
        rewrite(
            &workspace,
            "path = \"tests/integration.rs\"",
            &format!("path = \"tests/integration.rs\"\n{option}"),
        );
        assert!(check_workspace(&workspace.root).is_err());
    }
    check_workspace(&workspace().root).unwrap();
}

#[test]
fn test_module_decoys_are_rejected() {
    for contents in [
        "// mod case;",
        "const TEXT: &str = \"mod case;\";",
        "mod case {}",
        "#[cfg(unix)] mod case;",
        "#[path = \"other.rs\"] mod case;",
        "mod case; mod case;",
    ] {
        let workspace = workspace();
        let tests = workspace.root.join("crates/maestro-models/tests");
        std::fs::write(tests.join("case.rs"), "").unwrap();
        std::fs::write(tests.join("integration.rs"), contents).unwrap();
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(error.contains("case.rs"), "{error}");
    }
}

#[test]
fn unmigrated_test_layout_is_unchanged() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-models", "maestro-text"]);
    let tests = workspace.root.join("crates/maestro-text/tests");
    std::fs::create_dir_all(&tests).unwrap();
    std::fs::write(tests.join("standalone.rs"), "").unwrap();
    check_workspace(&workspace.root).unwrap();
}
