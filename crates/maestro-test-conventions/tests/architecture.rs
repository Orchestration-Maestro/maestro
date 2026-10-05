mod support;

use maestro_test_conventions::check_workspace;
use support::Workspace;

#[test]
fn invalid_crate_name_is_rejected_and_corrected_name_passes() {
    let workspace = Workspace::new();
    for name in [
        "other-models",
        "maestro-Models",
        "maestro-models_extra",
        "maestro-1models",
        "maestro-models-2role",
        "maestro-models-too-many",
        "maestro-models-",
    ] {
        workspace.member("models", name, "");
        workspace.list(&[(name, "core")]);
        assert_eq!(
            check_workspace(&workspace.root),
            Err(format!("invalid workspace crate name: {name}"))
        );
    }
    for name in [
        "maestro",
        "maestro-models",
        "maestro-test-conventions",
        "maestro-mcp-client",
    ] {
        workspace.member("models", name, "");
        workspace.list(&[(name, "core")]);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn unlisted_member_is_rejected_and_listing_it_passes() {
    let workspace = Workspace::new();
    workspace.member("models", "maestro-models", "");
    assert_eq!(
        check_workspace(&workspace.root),
        Err("workspace crate is not listed in workspace-crates.json: maestro-models".into())
    );
    workspace.list(&[("maestro-models", "core")]);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn crate_list_requires_core_or_dedicated_layers() {
    let workspace = Workspace::new();
    workspace.member("app", "maestro-app", "");
    workspace.list(&[("maestro-app", "unknown")]);
    assert_eq!(
        check_workspace(&workspace.root),
        Err("invalid layer for maestro-app: expected core or dedicated".into())
    );
    for layer in ["core", "dedicated"] {
        workspace.list(&[("maestro-app", layer)]);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
    std::fs::write(workspace.root.join("workspace-crates.json"), "[]").unwrap();
    assert_eq!(
        check_workspace(&workspace.root),
        Err("workspace-crates.json must be an object mapping crate names to layers".into())
    );
}

#[test]
fn dependency_cycle_is_rejected_and_breaking_it_passes() {
    let workspace = Workspace::new();
    workspace.list(&[
        ("maestro-alpha", "core"),
        ("maestro-beta", "core"),
        ("maestro-gamma", "core"),
    ]);
    workspace.member(
        "alpha",
        "maestro-alpha",
        "[dependencies]\nrenamed = { package = \"maestro-beta\", path = \"../beta\" }",
    );
    workspace.member(
        "beta",
        "maestro-beta",
        "[dependencies]\nmaestro-gamma = { path = \"../gamma\" }",
    );
    for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
        workspace.member(
            "gamma",
            "maestro-gamma",
            &format!("[{kind}]\nmaestro-alpha = {{ path = \"../alpha\" }}"),
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Err("workspace dependency cycle: maestro-alpha -> maestro-beta -> maestro-gamma -> maestro-alpha".into())
        );
        workspace.member("gamma", "maestro-gamma", "");
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn models_internal_dependencies_are_rejected_and_removing_them_passes() {
    let workspace = Workspace::new();
    workspace.list(&[("maestro-models", "core"), ("maestro-app", "core")]);
    workspace.member("app", "maestro-app", "");
    for declaration in [
        "[dependencies]\nmaestro-app = { path = \"../app\" }",
        "[dev-dependencies]\nmaestro-app = { path = \"../app\" }",
        "[build-dependencies]\nmaestro-app = { path = \"../app\" }",
        "[dependencies]\nrenamed = { package = \"maestro-app\", path = \"../app/../app\", optional = true }",
        "[target.'cfg(target_os = \"none\")'.dependencies]\nmaestro-app = { path = \"../app\" }",
    ] {
        workspace.member("models", "maestro-models", declaration);
        assert_eq!(
            check_workspace(&workspace.root),
            Err("maestro-models must not depend on workspace crate maestro-app".into())
        );
        workspace.member("models", "maestro-models", "");
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn core_to_dedicated_dependency_is_rejected_and_reversing_it_passes() {
    let workspace = Workspace::new();
    workspace.list(&[("maestro-agent", "core"), ("maestro-jobs", "dedicated")]);
    for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
        workspace.member("jobs", "maestro-jobs", "");
        workspace.member(
            "agent",
            "maestro-agent",
            &format!("[{kind}]\nmaestro-jobs = {{ path = \"../jobs\" }}"),
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Err("core crate maestro-agent must not depend on dedicated crate maestro-jobs".into())
        );
        workspace.member("agent", "maestro-agent", "");
        workspace.member(
            "jobs",
            "maestro-jobs",
            &format!("[{kind}]\nmaestro-agent = {{ path = \"../agent\" }}"),
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn real_workspace_obeys_crate_rules() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    check_workspace(root).unwrap();
}

#[test]
fn external_dependency_aliases_are_not_workspace_edges() {
    let workspace = Workspace::new();
    workspace.list(&[("maestro-models", "core"), ("maestro-app", "dedicated")]);
    workspace.member("app", "maestro-app", "");
    workspace.member(
        "models",
        "maestro-models",
        "[dependencies]\nmaestro-app = { package = \"external-lib\", version = \"9\" }",
    );
    assert_eq!(check_workspace(&workspace.root), Ok(()));

    let external = workspace.root.join("external");
    std::fs::create_dir_all(external.join("src")).unwrap();
    std::fs::write(external.join("src/lib.rs"), "").unwrap();
    std::fs::write(
        external.join("Cargo.toml"),
        "[workspace]\n[package]\nname = \"external-lib\"\nversion = \"9.0.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(
        workspace.root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"external\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    workspace.member(
        "models",
        "maestro-models",
        "[dependencies]\nmaestro-app = { package = \"external-lib\", path = \"../../external\" }",
    );
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn metadata_command_failure_is_reported_not_accepted() {
    let workspace = Workspace::new();
    std::fs::write(workspace.root.join("Cargo.toml"), "not valid toml").unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.starts_with("cargo metadata failed ("), "{error}");
    assert!(error.contains("Cargo.toml"), "{error}");
}

#[test]
fn workspace_package_names_require_direct_member_paths_even_when_patched() {
    let workspace = Workspace::new();
    workspace.list(&[("maestro-agent", "core"), ("maestro-app", "core")]);
    workspace.member("app", "maestro-app", "");
    std::fs::write(
        workspace.root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n[patch.crates-io]\nmaestro-app = { path = \"crates/app\" }\n",
    ).unwrap();
    for kind in [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "target.'cfg(target_os = \"none\")'.dependencies",
    ] {
        for source in ["version = \"0.1\"", "git = \"https://example.invalid/app\""] {
            workspace.member(
                "agent",
                "maestro-agent",
                &format!("[{kind}]\nrenamed = {{ package = \"maestro-app\", {source} }}"),
            );
            assert_eq!(
                check_workspace(&workspace.root),
                Err("dependency maestro-agent -> maestro-app must use a path to that workspace member".into())
            );
        }
        workspace.member(
            "agent",
            "maestro-agent",
            &format!("[{kind}]\nrenamed = {{ package = \"maestro-app\", path = \"../app\" }}"),
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn settings_is_registered_as_core() {
    let workspace = Workspace::new();
    workspace.member("settings", "maestro-settings", "");
    assert_eq!(
        check_workspace(&workspace.root),
        Err("workspace crate is not listed in workspace-crates.json: maestro-settings".into())
    );
    workspace.list(&[("maestro-settings", "dedicated")]);
    assert_eq!(
        check_workspace(&workspace.root),
        Err("maestro-settings must be registered as core".into())
    );
    workspace.list(&[("maestro-settings", "core")]);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn settings_only_allows_declared_domain_edges() {
    let workspace = Workspace::new();
    workspace.list(&[
        ("maestro-settings", "core"),
        ("maestro-models", "core"),
        ("maestro-agent", "core"),
        ("maestro-packages", "core"),
        ("maestro-storage", "core"),
        ("maestro-jobs", "dedicated"),
    ]);
    for (directory, name) in [
        ("models", "maestro-models"),
        ("agent", "maestro-agent"),
        ("packages", "maestro-packages"),
        ("storage", "maestro-storage"),
        ("jobs", "maestro-jobs"),
    ] {
        workspace.member(directory, name, "");
    }
    for kind in [
        "dependencies",
        "build-dependencies",
        "target.'cfg(target_os = \"none\")'.dependencies",
        "target.'cfg(target_os = \"none\")'.build-dependencies",
    ] {
        for (directory, name) in [
            ("models", "maestro-models"),
            ("agent", "maestro-agent"),
            ("packages", "maestro-packages"),
            ("storage", "maestro-storage"),
            ("jobs", "maestro-jobs"),
        ] {
            for renamed in [false, true] {
                let key = if renamed { "alias" } else { name };
                workspace.member("settings","maestro-settings",&format!("[{kind}]\n{key} = {{ package = \"{name}\", path = \"../{directory}/../{directory}\", optional = true }}"));
                let result = check_workspace(&workspace.root);
                if matches!(directory, "models" | "agent" | "packages") {
                    assert_eq!(result, Ok(()));
                } else {
                    assert_eq!(
                        result,
                        Err(format!(
                            "maestro-settings must not depend on workspace crate {name}"
                        ))
                    );
                }
            }
        }
    }
    workspace.member("settings", "maestro-settings", "");
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn settings_rejects_internal_dev_dependencies() {
    let workspace = Workspace::new();
    workspace.list(&[
        ("maestro-settings", "core"),
        ("maestro-models", "core"),
        ("maestro-agent", "core"),
        ("maestro-packages", "core"),
    ]);
    for (directory, name) in [
        ("models", "maestro-models"),
        ("agent", "maestro-agent"),
        ("packages", "maestro-packages"),
    ] {
        workspace.member(directory, name, "");
    }
    for kind in [
        "dev-dependencies",
        "target.'cfg(target_os = \"none\")'.dev-dependencies",
    ] {
        for (directory, name) in [
            ("models", "maestro-models"),
            ("agent", "maestro-agent"),
            ("packages", "maestro-packages"),
        ] {
            workspace.member(
                "settings",
                "maestro-settings",
                &format!("[{kind}]\nalias = {{ package = \"{name}\", path = \"../{directory}\" }}"),
            );
            assert_eq!(
                check_workspace(&workspace.root),
                Err(format!(
                    "maestro-settings must not have internal dev dependency {name}"
                ))
            );
        }
    }
    workspace.member("settings", "maestro-settings", "");
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}
