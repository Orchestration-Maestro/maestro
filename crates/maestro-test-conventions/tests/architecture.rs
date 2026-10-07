#![cfg(test)]

mod support;

use std::fmt::Write;

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
    for name in [
        "maestro-tui",
        "maestro-tui-crossterm",
        "maestro-test-terminal",
        "maestro-extensions-wasm",
        "maestro-extensions-wasmtime",
        "maestro-chat",
        "maestro-rpc",
        "maestro-web",
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&[name]);
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
    for contents in [None, Some("not JSON")] {
        let list = workspace.root.join("workspace-crates.json");
        if let Some(contents) = contents {
            std::fs::write(&list, contents).unwrap();
        } else {
            std::fs::remove_file(&list).unwrap();
        }
        assert!(
            check_workspace(&workspace.root)
                .unwrap_err()
                .contains("workspace-crates.json")
        );
        workspace.list(&[("maestro-app", "core")]);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
    std::fs::remove_file(workspace.root.join("workspace-crates.json")).unwrap();
    std::fs::create_dir(workspace.root.join("workspace-crates.json")).unwrap();
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("workspace-crates.json")
    );
    std::fs::remove_dir(workspace.root.join("workspace-crates.json")).unwrap();
    workspace.list(&[("maestro-app", "core")]);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn all_crates_require_their_declared_layer() {
    for &(name, _) in support::policy::POLICY {
        let workspace = Workspace::new();
        workspace.foundation(&[name]);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
        let path = workspace.root.join("workspace-crates.json");
        let mut list: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let wrong = if support::class(name) == "core" {
            "dedicated"
        } else {
            "core"
        };
        list[name] = wrong.into();
        std::fs::write(&path, serde_json::to_vec(&list).unwrap()).unwrap();
        assert_eq!(
            check_workspace(&workspace.root),
            Err(format!(
                "invalid scoped layer for {name}: expected {}",
                support::class(name)
            ))
        );
        list[name] = support::class(name).into();
        std::fs::write(&path, serde_json::to_vec(&list).unwrap()).unwrap();
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
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
        ("maestro-resources", "core"),
        ("maestro-settings", "core"),
        ("maestro-tui", "core"),
        ("maestro-extensions-wasm", "core"),
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
            "[target.'cfg(target_os = \"none\")'.build-dependencies]\nmaestro-app = { path = \"../app\" }",
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
    workspace.member("app", "maestro-app", "");
    workspace.list(&[
        ("maestro-agent", "core"),
        ("maestro-app", "core"),
        ("maestro", "dedicated"),
    ]);
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
            Err("forbidden production dependency: maestro -> maestro-agent".into())
        };
        assert_eq!(check_workspace(&workspace.root), expected);
        workspace.member("root", "maestro", "");
        workspace.member("app", "maestro-app", "");
        workspace.list(&[
            ("maestro-agent", "core"),
            ("maestro-app", "core"),
            ("maestro", "dedicated"),
        ]);
        workspace.member(
            "root",
            "maestro",
            &format!("[{kind}]\nmaestro-app = {{ path = \"../app\" }}"),
        );
        let expected = if kind == "dev-dependencies" {
            Err("internal dev dependency requires declared dependency-free test support: maestro -> maestro-app".into())
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

    #[cfg(unix)]
    {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-models"]);
        let rustc = workspace.command("rustc", "printf 'host: x86_64-unknown-linux-gnu\\n'");
        workspace.probe(
            &workspace.root.join("missing-cargo"),
            &rustc,
            "cargo metadata failed:",
        );
        let failure = workspace.command("failure", "echo controlled-stderr >&2; exit 7");
        workspace.probe(&failure, &rustc, "controlled-stderr");
        let invalid = workspace.command("invalid", "printf 'not JSON'");
        workspace.probe(&invalid, &rustc, "invalid cargo metadata:");
    }
}

#[test]
fn workspace_package_names_require_direct_member_paths_even_when_patched() {
    let workspace = Workspace::new();
    workspace.list(&[("maestro-session", "core"), ("maestro-storage", "core")]);
    workspace.member("storage", "maestro-storage", "");
    std::fs::write(workspace.root.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\n[patch.crates-io]\nmaestro-storage = { path = \"crates/storage\" }\n").unwrap();
    for kind in [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "target.'cfg(target_os = \"none\")'.dependencies",
        "target.'cfg(target_os = \"none\")'.build-dependencies",
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
        ("maestro-chat", "maestro-rpc"),
        ("maestro-web", "maestro-models"),
        ("maestro-app", "maestro-chat"),
        ("maestro-app", "maestro-extensions-wasmtime"),
        ("maestro-tools", "maestro-extensions"),
        ("maestro-tools", "maestro-app"),
        ("maestro-extensions-wasmtime", "maestro-models"),
        ("maestro", "maestro-agent"),
        ("maestro-agent", "maestro-app"),
    ] {
        let workspace = Workspace::new();
        workspace.list(&[(from, support::class(from)), (to, support::class(to))]);
        workspace.member("to", to, "");
        for (kind, extra) in [
            ("dependencies", ""),
            ("build-dependencies", ""),
            ("dev-dependencies", ""),
            ("target.'cfg(target_os = \"none\")'.dev-dependencies", ""),
            ("dependencies", ", optional = true"),
            ("target.'cfg(target_os = \"none\")'.dependencies", ""),
            ("target.'cfg(target_os = \"none\")'.build-dependencies", ""),
        ] {
            workspace.member(
                "from",
                from,
                &format!("[{kind}]\nalias = {{ package = {to:?}, path = \"../to\"{extra} }}"),
            );
            assert_eq!(
                check_workspace(&workspace.root),
                Err(if kind.ends_with("dev-dependencies") {
                    format!(
                        "internal dev dependency requires declared dependency-free test support: {from} -> {to}"
                    )
                } else {
                    format!("forbidden production dependency: {from} -> {to}")
                })
            );
        }
        let corrected = Workspace::new();
        corrected.foundation(&[from, to]);
        assert_eq!(check_workspace(&corrected.root), Ok(()));
    }
}

#[test]
fn permitted_downward_edges_pass_without_absent_crates() {
    assert_eq!(support::policy::POLICY.len(), 26);
    assert_eq!(
        support::policy::POLICY
            .iter()
            .map(|row| row.1.len())
            .sum::<usize>(),
        62
    );
    for &(from, targets) in support::policy::POLICY {
        let workspace = Workspace::new();
        workspace.foundation(&[&[from], targets].concat());
        if targets.is_empty() {
            assert_eq!(check_workspace(&workspace.root), Ok(()), "{from}");
        }
        for to in targets {
            if support::exact(from) && to != &targets[0] {
                continue;
            }
            check_permitted_declarations(&workspace, from, to, targets);
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
fn settings_and_resources_remain_dependency_free() {
    for leaf in ["maestro-settings", "maestro-resources"] {
        let workspace = Workspace::new();
        workspace.list(&[
            (leaf, "core"),
            ("maestro-models", "core"),
            ("maestro-agent", "core"),
            ("maestro-packages", "core"),
        ]);
        for target in ["maestro-models", "maestro-agent", "maestro-packages"] {
            workspace.member(target, target, "");
        }
        for target in ["maestro-models", "maestro-agent", "maestro-packages"] {
            for (kind, extra) in DECLARATIONS {
                workspace.member("leaf", leaf, &format!("[{kind}]\nalias = {{ package = {target:?}, path = \"../{target}\"{extra} }}"));
                assert_eq!(
                    check_workspace(&workspace.root),
                    Err(format!(
                        "{leaf} must not depend on workspace crate {target}"
                    ))
                );
                workspace.member("leaf", leaf, "");
                assert_eq!(check_workspace(&workspace.root), Ok(()));
            }
        }
    }
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
                    "maestro-settings must not depend on workspace crate {name}"
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
    if let Some(class) = inventory.get("maestro-agent") {
        assert_eq!(class, "core");
    }
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
        check_agent_production_declarations(&workspace, to);
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

#[test]
fn credentials_is_registered_as_core() {
    let workspace = Workspace::new();
    workspace.member("credentials", "maestro-credentials", "");
    assert_eq!(
        check_workspace(&workspace.root),
        Err("workspace crate is not listed in workspace-crates.json: maestro-credentials".into())
    );
    workspace.list(&[("maestro-credentials", "dedicated")]);
    assert_eq!(
        check_workspace(&workspace.root),
        Err("invalid scoped layer for maestro-credentials: expected core".into())
    );
    workspace.list(&[("maestro-credentials", "core")]);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn credentials_only_allows_models_edge() {
    for (to, class) in [
        ("maestro-models", "core"),
        ("maestro-storage", "core"),
        ("maestro-packages", "core"),
        ("maestro-agent", "core"),
        ("maestro-tools", "core"),
        ("maestro-session", "core"),
        ("maestro-settings", "core"),
        ("maestro-resources", "core"),
        ("maestro-extensions", "core"),
        ("maestro-app", "core"),
        ("maestro-cli", "core"),
        ("maestro", "dedicated"),
        ("maestro-test-conventions", "dedicated"),
    ] {
        let workspace = Workspace::new();
        workspace.list(&[("maestro-credentials", "core"), (to, class)]);
        workspace.member("to", to, "");
        for (kind, extra) in [
            ("dependencies", ""),
            ("dependencies", ", optional = true"),
            ("build-dependencies", ""),
            (
                "target.'cfg(target_os = \"none\")'.dependencies",
                ", optional = true",
            ),
            ("target.'cfg(target_os = \"none\")'.build-dependencies", ""),
        ] {
            workspace.member(
                "credentials",
                "maestro-credentials",
                &format!("[{kind}]\nalias = {{ package = {to:?}, path = \"../to\"{extra} }}"),
            );
            let result = check_workspace(&workspace.root);
            if to == "maestro-models" {
                assert_eq!(result, Ok(()));
            } else if class == "dedicated" {
                assert_eq!(
                    result,
                    Err(format!(
                        "core crate maestro-credentials must not depend on dedicated crate {to}"
                    ))
                );
            } else {
                assert_eq!(
                    result,
                    Err(format!(
                        "forbidden production dependency: maestro-credentials -> {to}"
                    ))
                );
            }
        }
    }
}

#[test]
fn credentials_rejects_internal_dev_dependencies() {
    for (to, class) in [
        ("maestro-models", "core"),
        ("maestro-storage", "core"),
        ("maestro-packages", "core"),
        ("maestro-agent", "core"),
        ("maestro-tools", "core"),
        ("maestro-session", "core"),
        ("maestro-settings", "core"),
        ("maestro-resources", "core"),
        ("maestro-extensions", "core"),
        ("maestro-app", "core"),
        ("maestro-cli", "core"),
        ("maestro", "dedicated"),
        ("maestro-test-conventions", "dedicated"),
    ] {
        let workspace = Workspace::new();
        workspace.list(&[("maestro-credentials", "core"), (to, class)]);
        workspace.member("to", to, "");
        for kind in [
            "dev-dependencies",
            "target.'cfg(target_os = \"none\")'.dev-dependencies",
        ] {
            workspace.member(
                "credentials",
                "maestro-credentials",
                &format!("[{kind}]\nalias = {{ package = {to:?}, path = \"../to\" }}"),
            );
            let expected = if class == "dedicated" {
                format!("core crate maestro-credentials must not depend on dedicated crate {to}")
            } else {
                format!(
                    "internal dev dependency requires declared dependency-free test support: maestro-credentials -> {to}"
                )
            };
            assert_eq!(check_workspace(&workspace.root), Err(expected));
        }
    }
}

const DECLARATIONS: &[(&str, &str)] = &[
    ("dependencies", ""),
    ("build-dependencies", ""),
    ("dependencies", ", optional = true"),
    ("target.'cfg(target_os = \"none\")'.dependencies", ""),
    ("target.'cfg(target_os = \"none\")'.build-dependencies", ""),
];

#[test]
fn packages_use_settings_and_resources() {
    let workspace = Workspace::new();
    workspace.list(&[
        ("maestro-packages", "core"),
        ("maestro-settings", "core"),
        ("maestro-resources", "core"),
        ("maestro-models", "core"),
    ]);
    for target in ["settings", "resources", "models"] {
        workspace.member(target, &format!("maestro-{target}"), "");
    }
    for targets in [
        vec!["settings"],
        vec!["resources"],
        vec!["settings", "resources"],
    ] {
        for (kind, extra) in DECLARATIONS {
            let mut dependencies = format!("[{kind}]\n");
            for target in &targets {
                writeln!(
                    dependencies,
                    "{target} = {{ package = \"maestro-{target}\", path = \"../{target}\"{extra} }}"
                )
                .unwrap();
            }
            workspace.member("packages", "maestro-packages", &dependencies);
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        }
    }
    workspace.member(
        "packages",
        "maestro-packages",
        "[dependencies]\nmaestro-models = { path = \"../models\" }",
    );
    assert_eq!(
        check_workspace(&workspace.root),
        Err("forbidden production dependency: maestro-packages -> maestro-models".into())
    );
    workspace.member("packages", "maestro-packages", "");
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    for leaf in ["settings", "resources"] {
        workspace.member(
            leaf,
            &format!("maestro-{leaf}"),
            "[dependencies]\nmaestro-packages = { path = \"../packages\" }",
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Err(format!(
                "maestro-{leaf} must not depend on workspace crate maestro-packages"
            ))
        );
        workspace.member(leaf, &format!("maestro-{leaf}"), "");
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn sparse_inventory_does_not_create_product_crates() {
    let workspace = Workspace::new();
    workspace.member("models", "maestro-models", "");
    workspace.list(&[
        ("maestro-models", "core"),
        ("maestro-tools", "core"),
        ("maestro-chat", "core"),
    ]);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    assert!(!workspace.root.join("crates/maestro-tools").exists());
    assert!(!workspace.root.join("crates/maestro-chat").exists());
    assert!(!workspace.root.join("wit").exists());
}

#[test]
fn frontends_require_complete_direct_dependencies() {
    for frontend in ["maestro-cli", "maestro-chat", "maestro-rpc", "maestro-web"] {
        let required = support::policy::POLICY
            .iter()
            .find(|row| row.0 == frontend)
            .unwrap()
            .1;
        let workspace = Workspace::new();
        workspace.foundation(&[&[frontend], required].concat());
        for omitted in required {
            check_frontend_omission(&workspace, frontend, required, omitted);
        }
        let mut mixed = String::new();
        for (index, target) in required.iter().enumerate() {
            let (kind, extra) = DECLARATIONS[if index < 2 { index } else { index + 1 }];
            writeln!(
                mixed,
                "[{kind}]\nedge{index} = {{ package = {target:?}, path = \"../{target}\"{extra} }}"
            )
            .unwrap();
        }
        workspace.member(frontend, frontend, &mixed);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
        workspace.member(frontend, frontend, "");
        assert!(
            check_workspace(&workspace.root)
                .unwrap_err()
                .contains("missing required direct dependency")
        );
    }
}

#[test]
fn terminal_adapter_and_harness_require_toolkit_only() {
    for owner in ["maestro-tui-crossterm", "maestro-test-terminal"] {
        let workspace = Workspace::new();
        workspace.foundation(&[owner, "maestro-models"]);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
        workspace.member(owner, owner, "");
        assert_eq!(
            check_workspace(&workspace.root),
            Err(format!(
                "{owner}: missing required direct dependency maestro-tui"
            ))
        );
        for (kind, extra) in DECLARATIONS {
            workspace.member(owner, owner, &format!("[{kind}]\nmaestro-tui = {{ path = \"../maestro-tui\" }}\nmodels = {{ package = \"maestro-models\", path = \"../maestro-models\"{extra} }}"));
            assert_eq!(
                check_workspace(&workspace.root),
                Err(format!(
                    "forbidden production dependency: {owner} -> maestro-models"
                ))
            );
            workspace.member(
                owner,
                owner,
                &format!("[{kind}]\nmaestro-tui = {{ path = \"../maestro-tui\"{extra} }}"),
            );
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        }
    }
    for &(from, _) in support::policy::POLICY {
        if from == "maestro-test-terminal" {
            continue;
        }
        let workspace = Workspace::new();
        workspace.foundation(&[from, "maestro-test-terminal"]);
        for (kind, extra) in DECLARATIONS
            .iter()
            .copied()
            .chain([("dev-dependencies", "")])
        {
            workspace.member(from, from, &format!("[{kind}]\nharness = {{ package = \"maestro-test-terminal\", path = \"../maestro-test-terminal\"{extra} }}"));
            let error = check_workspace(&workspace.root).unwrap_err();
            assert!(
                error.contains(from)
                    && error.contains("maestro-test-terminal")
                    && !error.contains("missing required"),
                "{error}"
            );
        }
        workspace.foundation(&[from, "maestro-test-terminal"]);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn internal_dev_edges_are_rejected_for_every_member() {
    for &(from, _) in support::policy::POLICY {
        for to in [
            "maestro-models",
            "maestro-test-conventions",
            "maestro-test-terminal",
        ] {
            if from == to {
                continue;
            }
            let workspace = Workspace::new();
            workspace.foundation(&[from, to]);
            for kind in [
                "dev-dependencies",
                "target.'cfg(target_os = \"none\")'.dev-dependencies",
            ] {
                workspace.member(
                    from,
                    from,
                    &format!("[{kind}]\nrenamed = {{ package = {to:?}, path = \"../{to}\" }}"),
                );
                let error = check_workspace(&workspace.root).unwrap_err();
                assert!(
                    error.contains(from)
                        && error.contains(to)
                        && !error.contains("missing required"),
                    "{error}"
                );
            }
            workspace.foundation(&[from, to]);
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        }
    }
}

#[cfg(unix)]
#[test]
fn metadata_graph_failures_cross_the_public_interface() {
    use serde_json::json;
    if let Some(root) = std::env::var_os("MAESTRO_CONVENTIONS_PROBE_ROOT") {
        let result = check_workspace(std::path::Path::new(&root));
        let expected = std::env::var("MAESTRO_CONVENTIONS_EXPECT").unwrap();
        if expected == "ok" {
            assert_eq!(result, Ok(()));
        } else {
            let error = result.unwrap_err();
            assert!(error.contains(&expected), "{error}");
        }
        return;
    }
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-agent", "maestro-models", "maestro-storage"]);
    let valid = json!({"workspace_members": ["agent-id", "models-id", "storage-id"], "packages": [
        {"id": "agent-id", "name": "maestro-agent", "manifest_path": workspace.root.join("crates/maestro-agent/Cargo.toml"), "dependencies": []},
        {"id": "models-id", "name": "maestro-models", "manifest_path": workspace.root.join("crates/maestro-models/Cargo.toml"), "dependencies": []},
        {"id": "storage-id", "name": "maestro-storage", "manifest_path": workspace.root.join("crates/maestro-storage/Cargo.toml"), "dependencies": []}
    ], "resolve": {"nodes": [
        {"id": "agent-id", "deps": [{"name": "alias", "pkg": "models-id", "dep_kinds": [{"kind": null, "target": null}]}]},
        {"id": "models-id", "deps": []}, {"id": "storage-id", "deps": []}
    ]}});
    workspace.metadata_probe(&valid, &valid, "ok");
    invalid_metadata_identities(&workspace, &valid);
    invalid_resolved_metadata(&workspace, &valid);
    complete_resolved_metadata();
}

#[cfg(unix)]
fn invalid_resolved_metadata(workspace: &Workspace, valid: &serde_json::Value) {
    use serde_json::{Value, json};
    for (pointer, replacement) in [
        ("/resolve", Value::Null),
        ("/resolve/nodes", Value::Null),
        ("/resolve/nodes", json!([])),
        ("/resolve/nodes/0/id", json!("unknown")),
        ("/resolve/nodes/0/deps/0/pkg", json!("unknown")),
        ("/resolve/nodes/0/deps/0/name", Value::Null),
        ("/resolve/nodes/0/deps/0/dep_kinds", Value::Null),
        ("/resolve/nodes/0/deps/0/dep_kinds", json!([])),
        ("/resolve/nodes/0/deps/0/dep_kinds/0/kind", json!("invalid")),
        ("/resolve/nodes/0/deps/0/dep_kinds/0/target", json!(1)),
    ] {
        let mut value = valid.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        workspace.metadata_probe(valid, &value, "invalid cargo metadata");
    }
    for (parent, key) in [
        ("", "resolve"),
        ("/resolve", "nodes"),
        ("/resolve/nodes/0/deps/0", "name"),
        ("/resolve/nodes/0/deps/0", "dep_kinds"),
        ("/resolve/nodes/0/deps/0/dep_kinds/0", "kind"),
        ("/resolve/nodes/0/deps/0/dep_kinds/0", "target"),
    ] {
        let mut value = valid.clone();
        value
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        workspace.metadata_probe(valid, &value, "invalid cargo metadata");
    }
    invalid_resolved_edges(workspace, valid);
}

#[cfg(unix)]
fn invalid_resolved_edges(workspace: &Workspace, valid: &serde_json::Value) {
    use serde_json::json;
    let mut value = valid.clone();
    value["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(valid["resolve"]["nodes"][0].clone());
    workspace.metadata_probe(valid, &value, "invalid cargo metadata");
    let mut value = valid.clone();
    value["resolve"]["nodes"].as_array_mut().unwrap().pop();
    workspace.metadata_probe(valid, &value, "invalid cargo metadata");
    let mut value = valid.clone();
    value["resolve"]["nodes"][0]["deps"][0]["pkg"] = json!("storage-id");
    workspace.metadata_probe(
        valid,
        &value,
        "forbidden production dependency: maestro-agent -> maestro-storage",
    );
    let mut value = valid.clone();
    value["packages"][1]["name"] = json!("maestro-unknown");
    workspace.metadata_probe(valid, &value, "invalid cargo metadata");
    workspace.metadata_probe(valid, valid, "ok");
}

#[cfg(unix)]
fn complete_resolved_metadata() {
    use serde_json::json;
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-cli"]);
    let names = [
        "maestro-cli",
        "maestro-app",
        "maestro-tui",
        "maestro-tui-crossterm",
        "maestro-theme",
    ];
    let mut full = json!({
        "workspace_members": names,
        "packages": names.iter().map(|name| json!({
            "id": name, "name": name, "manifest_path": workspace.root.join(format!("crates/{name}/Cargo.toml")), "dependencies": []
        })).collect::<Vec<_>>(),
        "resolve": {"nodes": names.iter().map(|name| {
            let targets = if support::exact(name) { support::policy::POLICY.iter().find(|row| row.0 == *name).unwrap().1 } else { &[] };
            json!({"id": name, "deps": targets.iter().map(|target| json!({"name": "alias", "pkg": target, "dep_kinds": [{"kind": null, "target": null}]})).collect::<Vec<_>>()})
        }).collect::<Vec<_>>()}
    });
    // The declared graph alone is incomplete; the resolved direct edges complete it.
    workspace.metadata_probe(&full, &full, "ok");
    for omitted in 0..4 {
        let mut value = full.clone();
        value["resolve"]["nodes"][0]["deps"]
            .as_array_mut()
            .unwrap()
            .remove(omitted);
        workspace.metadata_probe(&full, &value, "missing required direct dependency");
    }
    let mut declared = full.clone();
    declared["packages"][0]["dependencies"] = json!([{
        "name": "maestro-app", "kind": "build", "optional": true,
        "target": "cfg(target_os = \"none\")", "path": workspace.root.join("crates/maestro-app")
    }]);
    full["resolve"]["nodes"][0]["deps"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    workspace.metadata_probe(&declared, &full, "ok");
}

#[test]
fn extension_domain_accepts_resource_edges() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions", "maestro-resources"]);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            "maestro-extensions",
            "maestro-extensions",
            &format!("[{kind}]\nalias = {{ package = \"maestro-resources\", path = \"../maestro-resources\"{extra} }}"),
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()), "{kind}{extra}");
    }
}

#[test]
fn resource_leaf_rejects_extension_edges() {
    for (kind, extra) in DECLARATIONS.iter().copied().chain([
        ("dev-dependencies", ""),
        ("target.'cfg(target_os = \"none\")'.dev-dependencies", ""),
    ]) {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-extensions", "maestro-resources"]);
        workspace.member(
            "maestro-resources",
            "maestro-resources",
            &format!("[{kind}]\nalias = {{ package = \"maestro-extensions\", path = \"../maestro-extensions\"{extra} }}"),
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Err("maestro-resources must not depend on workspace crate maestro-extensions".into()),
            "{kind}{extra}"
        );
        workspace.member("maestro-resources", "maestro-resources", "");
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn extension_resource_dev_edges_stay_forbidden() {
    for kind in [
        "dev-dependencies",
        "target.'cfg(target_os = \"none\")'.dev-dependencies",
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-extensions", "maestro-resources"]);
        workspace.member(
            "maestro-extensions",
            "maestro-extensions",
            &format!("[{kind}]\nalias = {{ package = \"maestro-resources\", path = \"../maestro-resources\" }}"),
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Err("internal dev dependency requires declared dependency-free test support: maestro-extensions -> maestro-resources".into()),
            "{kind}"
        );
        workspace.member("maestro-extensions", "maestro-extensions", "");
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn documented_foundation_graph_matches_policy() {
    use std::collections::{BTreeMap, BTreeSet};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let specification = std::fs::read_to_string(root.join("docs/specs/maestro-port.md")).unwrap();
    let architecture = std::fs::read_to_string(root.join("docs/architecture.md")).unwrap();
    let table = specification
        .split_once("| Crate | One job | Allowed internal dependencies | Layer |\n")
        .unwrap()
        .1
        .split_once("\n\n")
        .unwrap()
        .0;
    let mut documented = BTreeMap::new();
    for row in table.lines().skip(1) {
        let columns: Vec<_> = row.split('|').map(str::trim).collect();
        assert_eq!(columns.len(), 6, "{row}");
        let name = columns[1].trim_matches('`');
        let dependencies: BTreeSet<_> = if columns[3] == "—" {
            BTreeSet::new()
        } else {
            columns[3]
                .split(", ")
                .map(|dependency| dependency.trim_matches('`'))
                .collect()
        };
        assert!(
            documented.insert(name, dependencies).is_none(),
            "duplicate crate: {name}"
        );
    }
    let expected: BTreeMap<_, BTreeSet<_>> = support::policy::POLICY
        .iter()
        .map(|&(name, dependencies)| (name, dependencies.iter().copied().collect()))
        .collect();
    assert_eq!(documented, expected);
    assert_eq!(documented.len(), 26);
    assert_eq!(documented.values().map(BTreeSet::len).sum::<usize>(), 62);
    assert_eq!(documented.values().filter(|row| row.is_empty()).count(), 8);
    assert!(documented["maestro-resources"].is_empty());
    assert!(documented["maestro-extensions"].contains("maestro-resources"));
    assert!(specification.lines().any(|line| line == "| `maestro-extensions` | govern extension semantics | `maestro-models`, `maestro-agent`, `maestro-session`, `maestro-catalog`, `maestro-tools`, `maestro-theme`, `maestro-tui`, `maestro-resources` | 3 |"));
    assert!(specification.split("\n\n").any(|paragraph| paragraph == "The graph contains 26 crates and 62 permitted internal dependency edges; eight crates remain leaves."));
    assert!(architecture.split("\n\n").any(|paragraph| paragraph == "The extension domain may depend directly on resources for shared source information. Resources remain a leaf; this permission does not allow the reverse dependency or internal dev-dependencies."));
}

fn check_permitted_declarations(workspace: &Workspace, from: &str, to: &str, targets: &[&str]) {
    for (kind, extra) in DECLARATIONS {
        let selected = if support::exact(from) {
            targets
        } else {
            std::slice::from_ref(&to)
        };
        let mut declarations = format!("[{kind}]\n");
        for (index, target) in selected.iter().enumerate() {
            writeln!(
                declarations,
                "renamed{index} = {{ package = {target:?}, path = \"../{target}\"{extra} }}"
            )
            .unwrap();
        }
        workspace.member(from, from, &declarations);
        assert_eq!(
            check_workspace(&workspace.root),
            Ok(()),
            "{from} -> {to} ({kind}{extra})"
        );
    }
}

fn check_agent_production_declarations(workspace: &Workspace, to: &str) {
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
                &format!("[{kind}]\n{name} = {{ package = {to:?}, path = \"../target\"{extra} }}"),
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

fn check_frontend_omission(
    workspace: &Workspace,
    frontend: &str,
    required: &[&str],
    omitted: &str,
) {
    for (kind, extra) in DECLARATIONS {
        let mut declarations = format!("[{kind}]\n");
        for (index, target) in required
            .iter()
            .enumerate()
            .filter(|(_, target)| **target != omitted)
        {
            writeln!(
                declarations,
                "alias{index} = {{ package = {target:?}, path = \"../{target}\"{extra} }}"
            )
            .unwrap();
        }
        workspace.member(frontend, frontend, &declarations);
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(
            error.contains(frontend)
                && error.contains(omitted)
                && error.contains("missing required direct dependency"),
            "{error}"
        );
        writeln!(
            declarations,
            "restored = {{ package = {omitted:?}, path = \"../{omitted}\"{extra} }}"
        )
        .unwrap();
        workspace.member(frontend, frontend, &declarations);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[cfg(unix)]
fn invalid_metadata_identities(workspace: &Workspace, valid: &serde_json::Value) {
    use serde_json::{Value, json};
    for pointer in ["/workspace_members", "/packages/0/id", "/packages/0/name"] {
        let mut value = valid.clone();
        *value.pointer_mut(pointer).unwrap() = Value::Null;
        workspace.metadata_probe(&value, valid, "invalid cargo metadata");
        workspace.metadata_probe(valid, &value, "invalid cargo metadata");
    }
    for members in [
        json!(["agent-id", "agent-id"]),
        json!(["unknown"]),
        json!([null]),
    ] {
        let mut value = valid.clone();
        value["workspace_members"] = members;
        workspace.metadata_probe(&value, valid, "invalid cargo metadata");
        workspace.metadata_probe(valid, &value, "invalid cargo metadata");
    }
    for packages in [
        json!([]),
        json!([valid["packages"][0].clone(), valid["packages"][0].clone()]),
    ] {
        let mut value = valid.clone();
        value["packages"] = packages;
        workspace.metadata_probe(&value, valid, "invalid cargo metadata");
        workspace.metadata_probe(valid, &value, "invalid cargo metadata");
    }
    let mut duplicate_name = valid.clone();
    duplicate_name["packages"][1]["name"] = json!("maestro-agent");
    workspace.metadata_probe(&duplicate_name, valid, "invalid cargo metadata");
    workspace.metadata_probe(valid, &duplicate_name, "invalid cargo metadata");
}
