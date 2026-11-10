#![cfg(test)]

use crate::support::Workspace;
use maestro_test_conventions::check_workspace;

/// Append `profile` to the fixture's root manifest after dropping any existing development profile.
fn with_profile(workspace: &Workspace, profile: &str) {
    let path = workspace.root.path().join("Cargo.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut document: toml::Table = text.parse().unwrap();
    document.remove("profile");
    let mut text = document.to_string();
    text.push_str(profile);
    std::fs::write(path, text).unwrap();
}

#[test]
fn development_profile_requires_optimisation_level_one() {
    let workspace = Workspace::with_text_member();
    for profile in [
        "",
        "[profile.dev]\ndebug = \"line-tables-only\"\n",
        "[profile.dev]\nopt-level = 0\n",
        "[profile.dev]\nopt-level = 2\n",
        "[profile.release]\nopt-level = 1\n",
    ] {
        with_profile(&workspace, profile);
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains("[profile.dev] must set opt-level = 1 to keep"),
            "{profile}: {error}"
        );
    }
    with_profile(&workspace, "[profile.dev]\nopt-level = 1\n");
    check_workspace(workspace.root.path()).unwrap();
}

#[test]
fn development_profile_rejects_optimisation_overrides() {
    let workspace = Workspace::with_text_member();
    for (profile, name) in [
        ("[profile.test]\nopt-level = 0\n", "[profile.test]"),
        (
            "[profile.dev.package.\"*\"]\nopt-level = 0\n",
            "[profile.dev.package.\"*\"]",
        ),
    ] {
        with_profile(
            &workspace,
            &format!("[profile.dev]\nopt-level = 1\n{profile}"),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(
            error.contains(&format!("{name} must not set opt-level")),
            "{profile}: {error}"
        );
    }
    with_profile(
        &workspace,
        "[profile.dev]\nopt-level = 1\n[profile.dev.package.\"*\"]\ndebug = false\n[profile.dev.package.sha2]\nopt-level = 3\n",
    );
    check_workspace(workspace.root.path()).unwrap();
}
