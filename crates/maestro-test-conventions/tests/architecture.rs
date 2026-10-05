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
    for (name, class) in [
        ("maestro", "dedicated"),
        ("maestro-models", "core"),
        ("maestro-test-conventions", "dedicated"),
        ("maestro-storage", "core"),
    ] {
        workspace.member("models", name, "");
        workspace.list(&[(name, class)]);
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
    workspace.list(&[("maestro-app", "core")]);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    workspace.list(&[("maestro-app", "dedicated")]);
    assert_eq!(
        check_workspace(&workspace.root),
        Err("invalid scoped layer for maestro-app: expected core".into())
    );
    std::fs::write(workspace.root.join("workspace-crates.json"), "[]").unwrap();
    assert_eq!(
        check_workspace(&workspace.root),
        Err("workspace-crates.json must be an object mapping crate names to layers".into())
    );
}

#[test]
fn production_cycles_are_rejected() {
    let workspace = Workspace::new();
    workspace.list(&[("maestro-agent", "core"), ("maestro-session", "core")]);
    workspace.member(
        "session",
        "maestro-session",
        "[dependencies]\nmaestro-agent = { path = \"../agent\" }",
    );
    for kind in [
        "dependencies",
        "build-dependencies",
        "target.'cfg(target_os = \"none\")'.dependencies",
    ] {
        workspace.member(
            "agent",
            "maestro-agent",
            &format!("[{kind}]\nmaestro-session = {{ path = \"../session\" }}"),
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Err(
                "workspace dependency cycle: maestro-agent -> maestro-session -> maestro-agent"
                    .into()
            )
        );
        workspace.member("session", "maestro-session", "");
        assert_eq!(
            check_workspace(&workspace.root),
            Err("forbidden production dependency: maestro-agent -> maestro-session".into())
        );
        workspace.member("agent", "maestro-agent", "");
        workspace.member(
            "session",
            "maestro-session",
            "[dependencies]\nmaestro-agent = { path = \"../agent\" }",
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn leaf_internal_dependencies_are_rejected() {
    for (leaf, class) in [
        ("maestro-models", "core"),
        ("maestro-storage", "core"),
        ("maestro-packages", "core"),
        ("maestro-test-conventions", "dedicated"),
    ] {
        let workspace = Workspace::new();
        workspace.list(&[(leaf, class), ("maestro-app", "core")]);
        workspace.member("app", "maestro-app", "");
        for declaration in [
            "[dependencies]\nmaestro-app = { path = \"../app\" }",
            "[dev-dependencies]\nmaestro-app = { path = \"../app\" }",
            "[build-dependencies]\nmaestro-app = { path = \"../app\" }",
            "[dependencies]\nrenamed = { package = \"maestro-app\", path = \"../app/../app\", optional = true }",
            "[target.'cfg(target_os = \"none\")'.dependencies]\nmaestro-app = { path = \"../app\" }",
        ] {
            workspace.member("leaf", leaf, declaration);
            assert_eq!(
                check_workspace(&workspace.root),
                Err(format!(
                    "{leaf} must not depend on workspace crate maestro-app"
                ))
            );
            workspace.member("leaf", leaf, "");
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        }
    }
}

#[test]
fn core_to_dedicated_dependency_is_rejected() {
    let workspace = Workspace::new();
    workspace.list(&[("maestro-agent", "core"), ("maestro", "dedicated")]);
    for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
        workspace.member("root", "maestro", "");
        workspace.member(
            "agent",
            "maestro-agent",
            &format!("[{kind}]\nmaestro = {{ path = \"../root\" }}"),
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Err("core crate maestro-agent must not depend on dedicated crate maestro".into())
        );
        workspace.member("agent", "maestro-agent", "");
        workspace.member(
            "root",
            "maestro",
            &format!("[{kind}]\nmaestro-agent = {{ path = \"../agent\" }}"),
        );
        let expected = if kind == "dev-dependencies" {
            Err("internal dev dependency requires declared dependency-free test support: maestro -> maestro-agent".into())
        } else {
            Ok(())
        };
        assert_eq!(check_workspace(&workspace.root), expected);
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
    workspace.list(&[("maestro-models", "core"), ("maestro-app", "core")]);
    workspace.member("app", "maestro-app", "");
    workspace.external("external-lib");
    for source in ["version = \"9\"", "path = \"../../external\""] {
        workspace.member(
            "models",
            "maestro-models",
            &format!("[dependencies]\nmaestro-app = {{ package = \"external-lib\", {source} }}"),
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn metadata_command_failure_is_reported_not_accepted() {
    let workspace = Workspace::new();
    std::fs::write(workspace.root.join("Cargo.toml"), "not valid toml").unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.starts_with("cargo metadata failed ("), "{error}");
    assert!(error.contains("Cargo.toml"), "{error}");
    let workspace = Workspace::new();
    workspace.member(
        "models",
        "maestro-models",
        "[dependencies]\nmissing-offline-package = \"999.0.0\"",
    );
    workspace.list(&[("maestro-models", "core")]);
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.starts_with("cargo metadata failed ("), "{error}");
    assert!(error.contains("missing-offline-package"), "{error}");
}

#[test]
fn workspace_package_names_require_direct_member_paths_even_when_patched() {
    let workspace = Workspace::new();
    workspace.list(&[("maestro-session", "core"), ("maestro-storage", "core")]);
    workspace.member("storage", "maestro-storage", "");
    std::fs::write(workspace.root.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n[patch.crates-io]\nmaestro-storage = { path = \"crates/storage\" }\n").unwrap();
    for kind in [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "target.'cfg(target_os = \"none\")'.dependencies",
    ] {
        let mut sources = vec![
            "version = \"0.1\"",
            "git = \"https://example.invalid/storage\"",
        ];
        if kind != "dev-dependencies" {
            sources.extend([
                "version = \"0.1\", optional = true",
                "git = \"https://example.invalid/storage\", optional = true",
            ]);
        }
        for source in sources {
            workspace.member(
                "session",
                "maestro-session",
                &format!("[{kind}]\nrenamed = {{ package = \"maestro-storage\", {source} }}"),
            );
            assert_eq!(check_workspace(&workspace.root), Err("dependency maestro-session -> maestro-storage must use a path to that workspace member".into()));
        }
        workspace.member("session", "maestro-session", &format!("[{kind}]\nrenamed = {{ package = \"maestro-storage\", path = \"../storage/../storage\" }}"));
        let expected = if kind == "dev-dependencies" {
            Err("internal dev dependency requires declared dependency-free test support: maestro-session -> maestro-storage".into())
        } else {
            Ok(())
        };
        assert_eq!(check_workspace(&workspace.root), expected);
    }
}

#[test]
fn unknown_scoped_member_is_rejected() {
    let workspace = Workspace::new();
    workspace.member("unknown", "maestro-unknown", "");
    workspace.list(&[("maestro-unknown", "core")]);
    assert_eq!(
        check_workspace(&workspace.root),
        Err("workspace crate is outside scoped policy: maestro-unknown".into())
    );
}

#[test]
fn forbidden_direct_edges_are_rejected() {
    for (from, to) in [
        ("maestro-agent", "maestro-storage"),
        ("maestro-session", "maestro-tools"),
        ("maestro-cli", "maestro-session"),
        ("maestro-agent", "maestro-app"),
    ] {
        let workspace = Workspace::new();
        workspace.list(&[(from, "core"), (to, "core")]);
        workspace.member("to", to, "");
        for (kind, extra) in [
            ("dependencies", ""),
            ("build-dependencies", ""),
            ("dependencies", ", optional = true"),
            ("target.'cfg(target_os = \"none\")'.dependencies", ""),
        ] {
            workspace.member(
                "from",
                from,
                &format!("[{kind}]\nalias = {{ package = {to:?}, path = \"../to\"{extra} }}"),
            );
            assert_eq!(
                check_workspace(&workspace.root),
                Err(format!("forbidden production dependency: {from} -> {to}"))
            );
        }
    }
}

#[test]
fn permitted_downward_edges_pass_without_absent_crates() {
    let domain = [
        "maestro-models",
        "maestro-storage",
        "maestro-packages",
        "maestro-agent",
        "maestro-credentials",
        "maestro-tools",
        "maestro-session",
        "maestro-settings",
        "maestro-resources",
        "maestro-extensions",
    ];
    let core = [&domain[..], &["maestro-app", "maestro-cli"]].concat();
    let rows: &[(&str, &[&str])] = &[
        ("maestro-agent", &["maestro-models"]),
        ("maestro-credentials", &["maestro-models"]),
        ("maestro-tools", &["maestro-agent", "maestro-models"]),
        (
            "maestro-session",
            &["maestro-agent", "maestro-models", "maestro-storage"],
        ),
        (
            "maestro-settings",
            &["maestro-models", "maestro-agent", "maestro-packages"],
        ),
        (
            "maestro-resources",
            &["maestro-models", "maestro-settings", "maestro-packages"],
        ),
        (
            "maestro-extensions",
            &[
                "maestro-models",
                "maestro-agent",
                "maestro-session",
                "maestro-storage",
            ],
        ),
        ("maestro-app", &domain),
        ("maestro-cli", &["maestro-app"]),
        ("maestro", &core),
    ];
    for (from, targets) in rows {
        let workspace = Workspace::new();
        let mut inventory = vec![(
            *from,
            if *from == "maestro" {
                "dedicated"
            } else {
                "core"
            },
        )];
        for to in *targets {
            inventory.push((to, "core"));
            workspace.member(to, to, "");
        }
        workspace.list(&inventory);
        for (kind, extra) in [
            ("dependencies", ""),
            ("build-dependencies", ""),
            ("dependencies", ", optional = true"),
            ("target.'cfg(target_os = \"none\")'.dependencies", ""),
        ] {
            let mut declarations = format!("[{kind}]\n");
            for (index, to) in targets.iter().enumerate() {
                declarations.push_str(&format!(
                    "renamed{index} = {{ package = {to:?}, path = \"../{to}\"{extra} }}\n"
                ));
            }
            workspace.member("from", from, &declarations);
            assert_eq!(
                check_workspace(&workspace.root),
                Ok(()),
                "{from} -> {targets:?} ({kind}{extra})"
            );
        }
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
        Err("invalid scoped layer for maestro-settings: expected core".into())
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
        ("maestro-tools", "core"),
    ]);
    for (directory, name) in [
        ("models", "maestro-models"),
        ("agent", "maestro-agent"),
        ("packages", "maestro-packages"),
        ("storage", "maestro-storage"),
        ("tools", "maestro-tools"),
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
            ("tools", "maestro-tools"),
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
                            "forbidden production dependency: maestro-settings -> {name}"
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
                    "internal dev dependency requires declared dependency-free test support: maestro-settings -> {name}"
                ))
            );
        }
    }
    workspace.member("settings", "maestro-settings", "");
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn agent_is_registered_as_core() {
    let workspace = Workspace::new();
    workspace.member("agent", "maestro-agent", "");
    assert_eq!(
        check_workspace(&workspace.root),
        Err("workspace crate is not listed in workspace-crates.json: maestro-agent".into())
    );
    workspace.list(&[("maestro-agent", "dedicated")]);
    assert_eq!(
        check_workspace(&workspace.root),
        Err("invalid scoped layer for maestro-agent: expected core".into())
    );
    workspace.list(&[("maestro-agent", "core")]);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inventory: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("workspace-crates.json")).unwrap())
            .unwrap();
    assert_eq!(inventory["maestro-agent"], "core");
}

#[test]
fn agent_only_allows_models_production_edge() {
    for to in [
        "maestro-models",
        "maestro-storage",
        "maestro-packages",
        "maestro-credentials",
        "maestro-tools",
        "maestro-session",
        "maestro-settings",
        "maestro-resources",
        "maestro-extensions",
        "maestro-app",
        "maestro-cli",
        "maestro",
        "maestro-test-conventions",
    ] {
        let workspace = Workspace::new();
        workspace.list(&[
            ("maestro-agent", "core"),
            (
                to,
                if matches!(to, "maestro" | "maestro-test-conventions") {
                    "dedicated"
                } else {
                    "core"
                },
            ),
        ]);
        workspace.member("target", to, "");
        for renamed in [false, true] {
            for (kind, extra) in [
                ("dependencies", ""),
                ("build-dependencies", ""),
                ("dependencies", ", optional = true"),
                ("target.'cfg(target_os = \"none\")'.dependencies", ""),
                ("target.'cfg(target_os = \"none\")'.build-dependencies", ""),
            ] {
                let name = if renamed { "alias" } else { to };
                workspace.member(
                    "agent",
                    "maestro-agent",
                    &format!(
                        "[{kind}]\n{name} = {{ package = {to:?}, path = \"../target\"{extra} }}"
                    ),
                );
                assert_eq!(
                    check_workspace(&workspace.root),
                    if to == "maestro-models" {
                        Ok(())
                    } else if matches!(to, "maestro" | "maestro-test-conventions") {
                        Err(format!(
                            "core crate maestro-agent must not depend on dedicated crate {to}"
                        ))
                    } else {
                        Err(format!(
                            "forbidden production dependency: maestro-agent -> {to}"
                        ))
                    },
                    "{to} {kind} {renamed} {extra}"
                );
            }
        }
    }
}

#[test]
fn agent_rejects_internal_dev_dependencies() {
    let workspace = Workspace::new();
    workspace.list(&[("maestro-agent", "core"), ("maestro-models", "core")]);
    workspace.member("models", "maestro-models", "");
    for renamed in [false, true] {
        for kind in [
            "dev-dependencies",
            "target.'cfg(target_os = \"none\")'.dev-dependencies",
        ] {
            let name = if renamed { "alias" } else { "maestro-models" };
            workspace.member(
                "agent",
                "maestro-agent",
                &format!(
                    "[{kind}]\n{name} = {{ package = \"maestro-models\", path = \"../models\" }}"
                ),
            );
            assert_eq!(check_workspace(&workspace.root), Err("internal dev dependency requires declared dependency-free test support: maestro-agent -> maestro-models".into()));
        }
    }
    workspace.member("agent", "maestro-agent", "");
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}
