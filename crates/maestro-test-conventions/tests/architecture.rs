#![cfg(test)]

use crate::support;

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
            check_workspace(workspace.root.path()),
            Err(format!("invalid workspace crate name: {name}"))
        );
    }
    for (name, class) in [
        ("maestro", "dedicated"),
        ("maestro-models", "core"),
        ("maestro-test-conventions", "dedicated"),
        ("maestro-tooling", "dedicated"),
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&[name]);
        assert_eq!(support::class(name), class);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
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
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[test]
fn unlisted_member_is_rejected_and_listing_it_passes() {
    for &(name, _) in support::policy::POLICY {
        let workspace = Workspace::new();
        workspace.foundation(&[name]);
        let path = workspace.root.path().join("workspace-crates.json");
        let mut list: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        list.as_object_mut().unwrap().shift_remove(name);
        std::fs::write(&path, serde_json::to_vec(&list).unwrap()).unwrap();
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(format!(
                "workspace crate is not listed in workspace-crates.json: {name}"
            ))
        );
        workspace.foundation(&[name]);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[test]
fn crate_list_requires_core_or_dedicated_layers() {
    all_crates_require_their_declared_layer();
    let workspace = Workspace::new();
    workspace.member("app", "maestro-app", "");
    workspace.list(&[("maestro-app", "unknown")]);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("invalid layer for maestro-app: expected core or dedicated".into())
    );
    workspace.list(&[("maestro-app", "core")]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    workspace.list(&[("maestro-app", "dedicated")]);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("invalid scoped layer for maestro-app: expected core".into())
    );
    std::fs::write(workspace.root.path().join("workspace-crates.json"), "[]").unwrap();
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("workspace-crates.json must be an object mapping crate names to layers".into())
    );
    for contents in [None, Some("not JSON")] {
        let list = workspace.root.path().join("workspace-crates.json");
        if let Some(contents) = contents {
            std::fs::write(&list, contents).unwrap();
        } else {
            std::fs::remove_file(&list).unwrap();
        }
        assert!(
            check_workspace(workspace.root.path())
                .unwrap_err()
                .contains("workspace-crates.json")
        );
        workspace.list(&[("maestro-app", "core")]);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
    std::fs::remove_file(workspace.root.path().join("workspace-crates.json")).unwrap();
    std::fs::create_dir(workspace.root.path().join("workspace-crates.json")).unwrap();
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("workspace-crates.json")
    );
    std::fs::remove_dir(workspace.root.path().join("workspace-crates.json")).unwrap();
    workspace.list(&[("maestro-app", "core")]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

fn all_crates_require_their_declared_layer() {
    let names: Vec<_> = support::policy::POLICY.iter().map(|row| row.0).collect();
    let workspace = Workspace::new();
    workspace.foundation(&names);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    let path = workspace.root.path().join("workspace-crates.json");
    let mut list: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for name in names {
        let wrong = if support::class(name) == "core" {
            "dedicated"
        } else {
            "core"
        };
        list[name] = wrong.into();
        std::fs::write(&path, serde_json::to_vec(&list).unwrap()).unwrap();
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(format!(
                "invalid scoped layer for {name}: expected {}",
                support::class(name)
            ))
        );
        list[name] = support::class(name).into();
    }
    std::fs::write(&path, serde_json::to_vec(&list).unwrap()).unwrap();
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
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
            check_workspace(workspace.root.path()),
            Err(
                "workspace dependency cycle: maestro-agent -> maestro-session -> maestro-agent"
                    .into()
            )
        );
        workspace.member("session", "maestro-session", "");
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("forbidden production dependency: maestro-agent -> maestro-session".into())
        );
        workspace.member("agent", "maestro-agent", "");
        workspace.member(
            "session",
            "maestro-session",
            "[dependencies]\nmaestro-agent = { path = \"../agent\" }",
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

fn leaf_internal_dependencies_are_rejected() {
    for (leaf, class) in [
        ("maestro-path", "core"),
        ("maestro-cancellation", "core"),
        ("maestro-request", "core"),
        ("maestro-watch", "core"),
        ("maestro-lock", "core"),
        ("maestro-process", "core"),
        ("maestro-pattern", "core"),
        ("maestro-text", "core"),
        ("maestro-json", "core"),
        ("maestro-entry", "core"),
        ("maestro-tool", "core"),
        ("maestro-brand", "core"),
        ("maestro-models", "core"),
        ("maestro-resources", "core"),
        ("maestro-settings", "core"),
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
                check_workspace(workspace.root.path()),
                Err(if is_leaf(leaf) {
                    format!("{leaf} must not depend on workspace crate maestro-app")
                } else if declaration.starts_with("[dev-dependencies]") {
                    format!(
                        "internal dev dependency requires declared dependency-free test support: {leaf} -> maestro-app"
                    )
                } else {
                    format!("forbidden production dependency: {leaf} -> maestro-app")
                })
            );
        }
        workspace.member("leaf", leaf, "");
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

fn core_to_dedicated_dependency_is_rejected() {
    let workspace = Workspace::new();
    workspace.member("maestro-app", "maestro-app", "");
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
            check_workspace(workspace.root.path()),
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
        assert_eq!(check_workspace(workspace.root.path()), expected);
        workspace.member("root", "maestro", "");
        workspace.member("maestro-app", "maestro-app", "");
        workspace.list(&[
            ("maestro-agent", "core"),
            ("maestro-app", "core"),
            ("maestro", "dedicated"),
        ]);
        workspace.foundation(&["maestro-multiplexer"]);
        workspace.list(&[
            ("maestro-agent", "core"),
            ("maestro-app", "core"),
            ("maestro", "dedicated"),
            ("maestro-multiplexer", "core"),
        ]);
        let wiring_kind = if kind == "build-dependencies" {
            "dependencies"
        } else {
            kind
        };
        workspace.member("root", "maestro", &format!(
            "[{wiring_kind}]\nmaestro-app = {{ path = \"../maestro-app\" }}\nmaestro-multiplexer = {{ path = \"../maestro-multiplexer\" }}"
        ));
        let expected = if kind == "dev-dependencies" {
            Err("internal dev dependency requires declared dependency-free test support: maestro -> maestro-app".into())
        } else {
            Ok(())
        };
        assert_eq!(check_workspace(workspace.root.path()), expected);
    }
}

#[test]
fn root_can_wire_native_presentation_and_process_adapters() {
    let workspace = Workspace::new();
    workspace.foundation(&[
        "maestro",
        "maestro-theme",
        "maestro-tui-crossterm",
        "maestro-process",
    ]);
    let manifest = workspace.root.path().join("crates/maestro/Cargo.toml");
    let original = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(&manifest, format!("{original}\nmaestro-theme = {{ path = \"../maestro-theme\" }}\nmaestro-tui = {{ path = \"../maestro-tui\" }}\nmaestro-tui-crossterm = {{ path = \"../maestro-tui-crossterm\" }}\nmaestro-process = {{ path = \"../maestro-process\" }}\n")).unwrap();
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
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
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[test]
fn metadata_command_failure_is_reported_not_accepted() {
    let workspace = Workspace::new();
    std::fs::write(workspace.root.path().join("Cargo.toml"), "not valid toml").unwrap();
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(error.starts_with("cargo metadata failed ("), "{error}");
    assert!(error.contains("Cargo.toml"), "{error}");
    let workspace = Workspace::new();
    workspace.member(
        "models",
        "maestro-models",
        "[dependencies]\nmissing-offline-package = \"999.0.0\"",
    );
    workspace.list(&[("maestro-models", "core")]);
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(error.starts_with("cargo metadata failed ("), "{error}");
    assert!(error.contains("missing-offline-package"), "{error}");

    #[cfg(unix)]
    {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-models"]);
        let rustc = workspace.command("rustc", "printf 'host: x86_64-unknown-linux-gnu\\n'");
        workspace.probe(
            &workspace.root.path().join("missing-cargo"),
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
    workspace.list(&[("maestro-session", "core"), ("maestro-agent", "core")]);
    workspace.member("agent", "maestro-agent", "");
    let manifest = workspace.root.path().join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        format!("{text}[patch.crates-io]\nmaestro-agent = {{ path = \"crates/agent\" }}\n"),
    )
    .unwrap();
    for kind in [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "target.'cfg(target_os = \"none\")'.dependencies",
        "target.'cfg(target_os = \"none\")'.build-dependencies",
    ] {
        let mut sources = vec![
            "version = \"0.1\"",
            "git = \"https://example.invalid/agent\"",
        ];
        if kind != "dev-dependencies" {
            sources.extend([
                "version = \"0.1\", optional = true",
                "git = \"https://example.invalid/agent\", optional = true",
            ]);
        }
        for source in sources {
            workspace.member(
                "session",
                "maestro-session",
                &format!("[{kind}]\nrenamed = {{ package = \"maestro-agent\", {source} }}"),
            );
            assert_eq!(check_workspace(workspace.root.path()), Err("dependency maestro-session -> maestro-agent must use a path to that workspace member".into()));
        }
        workspace.member("session", "maestro-session", &format!("[{kind}]\nrenamed = {{ package = \"maestro-agent\", path = \"../agent/../agent\" }}"));
        let expected = if kind == "dev-dependencies" {
            Err("internal dev dependency requires declared dependency-free test support: maestro-session -> maestro-agent".into())
        } else {
            Ok(())
        };
        assert_eq!(check_workspace(workspace.root.path()), expected);
    }
}

#[test]
fn unknown_scoped_member_is_rejected() {
    let workspace = Workspace::new();
    workspace.member("unknown", "maestro-unknown", "");
    workspace.list(&[("maestro-unknown", "core")]);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("workspace crate is outside scoped policy: maestro-unknown".into())
    );
}

#[test]
fn forbidden_direct_edges_are_rejected() {
    forbidden_policy_matrix();
    for (from, to) in [
        ("maestro-agent", "maestro-settings"),
        ("maestro-session", "maestro-tools"),
        ("maestro-cli", "maestro-session"),
        ("maestro-chat", "maestro-rpc"),
        ("maestro-web", "maestro-models"),
        ("maestro-app", "maestro-chat"),
        ("maestro-app", "maestro-extensions-wasmtime"),
        ("maestro-tools", "maestro-extensions"),
        ("maestro-tools", "maestro-app"),
        ("maestro", "maestro-agent"),
        ("maestro-agent", "maestro-app"),
        ("maestro-app", "maestro-pattern"),
        ("maestro-process", "maestro-tools"),
        ("maestro-process", "maestro-packages"),
        ("maestro-process", "maestro-tui"),
        ("maestro-models", "maestro-process"),
        ("maestro-cli", "maestro-process"),
        ("maestro-chat", "maestro-process"),
        ("maestro-rpc", "maestro-process"),
        ("maestro-web", "maestro-process"),
        ("maestro-text", "maestro-tools"),
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
                check_workspace(workspace.root.path()),
                Err(if is_leaf(from) {
                    format!("{from} must not depend on workspace crate {to}")
                } else if kind.ends_with("dev-dependencies") {
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
        assert_eq!(check_workspace(corrected.root.path()), Ok(()));
    }
}

#[test]
fn export_reuses_text_and_json_without_frontend_edge() {
    for owner in ["maestro-text", "maestro-json"] {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-export", owner]);
        check_permitted_declarations(&workspace, "maestro-export", &[owner]);
    }
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-export", "maestro-web"]);
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            "maestro-export",
            "maestro-export",
            &format!("[{kind}]\nfrontend = {{ package = \"maestro-web\", path = \"../maestro-web\"{extra} }}"),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("forbidden production dependency: maestro-export -> maestro-web".into()),
            "{kind}{extra}"
        );
    }
}

#[test]
fn catalog_pattern_edge_is_scoped() {
    for from in ["maestro-catalog", "maestro-resources", "maestro-packages"] {
        let workspace = Workspace::new();
        workspace.foundation(&[from, "maestro-pattern"]);
        check_permitted_declarations(&workspace, from, &["maestro-pattern"]);
    }
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-catalog", "maestro-pattern"]);
    workspace.member(
        "maestro-catalog",
        "maestro-catalog",
        "[dev-dependencies]\nalias = { package = \"maestro-pattern\", path = \"../maestro-pattern\" }",
    );
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("internal dev dependency requires declared dependency-free test support: maestro-catalog -> maestro-pattern".into())
    );
    workspace.member("maestro-catalog", "maestro-catalog", "");
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            "maestro-pattern",
            "maestro-pattern",
            &format!("[{kind}]\nalias = {{ package = \"maestro-catalog\", path = \"../maestro-catalog\"{extra} }}"),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("forbidden production dependency: maestro-pattern -> maestro-catalog".into())
        );
    }
}

#[test]
fn listener_leaf_edges_are_scoped() {
    for from in [
        "maestro-models",
        "maestro-tui",
        "maestro-agent",
        "maestro-app",
        "maestro-extensions",
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&[from, "maestro-listeners"]);
        check_permitted_declarations(&workspace, from, &["maestro-listeners"]);
    }
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-session", "maestro-listeners"]);
    workspace.member(
        "maestro-session",
        "maestro-session",
        "[dependencies]\nlisteners = { package = \"maestro-listeners\", path = \"../maestro-listeners\" }",
    );
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("forbidden production dependency: maestro-session -> maestro-listeners".into())
    );
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-listeners", "maestro-text"]);
    workspace.member(
        "maestro-listeners",
        "maestro-listeners",
        "[dependencies]\ntext = { package = \"maestro-text\", path = \"../maestro-text\" }",
    );
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("maestro-listeners must not depend on workspace crate maestro-text".into())
    );
}

#[test]
fn process_capture_consumers_may_declare_the_owner() {
    for from in [
        "maestro-app",
        "maestro-tools",
        "maestro-packages",
        "maestro-tui",
        "maestro-credentials",
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&[from, "maestro-process"]);
        check_permitted_declarations(&workspace, from, &["maestro-process"]);
        workspace.member(
            from,
            from,
            "[target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]\nnative = { package = \"maestro-process\", path = \"../maestro-process\" }",
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{from}");
        workspace.member(
            from,
            from,
            "[dev-dependencies]\nalias = { package = \"maestro-process\", path = \"../maestro-process\" }",
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(format!(
                "internal dev dependency requires declared dependency-free test support: {from} -> maestro-process"
            ))
        );
    }
}

#[test]
fn process_owner_cannot_depend_on_cancellation() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-process", "maestro-cancellation"]);
    for (kind, extra) in DECLARATIONS.iter().chain(&[
        ("dev-dependencies", ""),
        ("target.'cfg(target_os = \"none\")'.dev-dependencies", ""),
    ]) {
        workspace.member(
            "maestro-process",
            "maestro-process",
            &format!("[{kind}]\nalias = {{ package = \"maestro-cancellation\", path = \"../maestro-cancellation\"{extra} }}"),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(if kind.ends_with("dev-dependencies") {
                "internal dev dependency requires declared dependency-free test support: maestro-process -> maestro-cancellation".into()
            } else {
                "forbidden production dependency: maestro-process -> maestro-cancellation".into()
            }),
            "{kind}{extra}"
        );
    }
    workspace.member("maestro-process", "maestro-process", "");
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn session_shared_records_edge_is_scoped() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-session", "maestro-request"]);
    check_permitted_declarations(&workspace, "maestro-session", &["maestro-request"]);
    workspace.member(
        "maestro-session",
        "maestro-session",
        "[dev-dependencies]\nalias = { package = \"maestro-request\", path = \"../maestro-request\" }",
    );
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("internal dev dependency requires declared dependency-free test support: maestro-session -> maestro-request".into())
    );
    workspace.member("maestro-session", "maestro-session", "");
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            "maestro-request",
            "maestro-request",
            &format!("[{kind}]\nalias = {{ package = \"maestro-session\", path = \"../maestro-session\"{extra} }}"),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("forbidden production dependency: maestro-request -> maestro-session".into())
        );
    }
}

#[test]
fn credentials_shared_records_edge_is_scoped() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-credentials", "maestro-request"]);
    check_permitted_declarations(&workspace, "maestro-credentials", &["maestro-request"]);
}

#[test]
fn theme_shared_records_edge_is_scoped() {
    let allowed = Workspace::new();
    allowed.foundation(&["maestro-theme", "maestro-request"]);
    check_permitted_declarations(&allowed, "maestro-theme", &["maestro-request"]);
    for (from, to) in [
        ("maestro-tui", "maestro-request"),
        ("maestro-request", "maestro-theme"),
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&[from, to]);
        for (kind, extra) in DECLARATIONS {
            workspace.member(
                from,
                from,
                &format!("[{kind}]\nalias = {{ package = {to:?}, path = \"../{to}\"{extra} }}"),
            );
            let error = check_workspace(workspace.root.path()).unwrap_err();
            assert!(
                error.contains(from) && error.contains(to),
                "{from} -> {to} ({kind}{extra}): {error}"
            );
        }
        workspace.member(
            from,
            from,
            &format!("[dev-dependencies]\nalias = {{ package = {to:?}, path = \"../{to}\" }}"),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains(from) && error.contains(to), "{error}");
    }
}

#[test]
fn documentation_checks_may_reuse_the_markdown_owner() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-test-conventions", "maestro-markdown"]);
    workspace.member(
        "maestro-test-conventions",
        "maestro-test-conventions",
        "[dependencies]\nmaestro-markdown = { path = \"../maestro-markdown\" }",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn permitted_downward_edges_pass_without_absent_crates() {
    support_declarations_match_owner(FIXTURES, FIXTURE_CALLERS);
    assert!(documented_foundation_graph_matches_policy(
        FOUNDATION_SPECIFICATION
    ));
    for &(from, targets) in support::policy::POLICY {
        let workspace = Workspace::new();
        workspace.foundation(&[&[from], targets].concat());
        if targets.is_empty() {
            assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{from}");
        } else {
            check_permitted_declarations(&workspace, from, targets);
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

/// Crates the specification excludes from the foundation utility.
fn utility_excluded() -> Vec<&'static str> {
    specification_names("The guest authoring crate", "are excluded from this")
}

/// Direct dependencies a crate must declare.
fn required_targets(name: &str) -> &'static [&'static str] {
    support::required(name)
}

/// Manifest declaring the required targets and the foundation utility.
fn dependencies_with_utility(required: &[&str], kind: &str, extra: &str) -> String {
    let mut declarations = String::from("[dependencies]\n");
    for target in required {
        writeln!(
            declarations,
            "{} = {{ package = {target:?}, path = \"../{target}\" }}",
            target.replace('-', "_")
        )
        .unwrap();
    }
    if kind != "dependencies" {
        writeln!(declarations, "[{kind}]").unwrap();
    }
    writeln!(
        declarations,
        "utility = {{ package = \"maestro-path\", path = \"../maestro-path\"{extra} }}"
    )
    .unwrap();
    declarations
}

#[test]
fn path_utility_edges_preserve_native_layer_rules() {
    let rows = support::policy::POLICY;
    let excluded = utility_excluded();
    let consumers: Vec<_> = rows
        .iter()
        .map(|row| row.0)
        .filter(|name| *name != "maestro-path" && !excluded.contains(name))
        .collect();
    native_consumers_may_declare_the_utility(&consumers);
    for from in consumers {
        native_consumer_may_not_dev_declare_the_utility(from);
    }
    for from in excluded {
        excluded_crate_may_not_declare_the_utility(from);
    }
    path_utility_has_no_outbound_edges();
    path_utility_does_not_replace_the_application();
}

/// Whether the table row of a crate lists no internal dependencies.
fn is_leaf(name: &str) -> bool {
    support::policy::POLICY
        .iter()
        .find(|row| row.0 == name)
        .unwrap()
        .1
        .is_empty()
}

/// Accept every production declaration form of the utility from all consumers at once.
fn native_consumers_may_declare_the_utility(consumers: &[&str]) {
    let workspace = Workspace::new();
    workspace.foundation(&[consumers, &["maestro-path"]].concat());
    for (kind, extra) in DECLARATIONS {
        for from in consumers {
            workspace.member(
                from,
                from,
                &dependencies_with_utility(required_targets(from), kind, extra),
            );
        }
        assert_eq!(
            check_workspace(workspace.root.path()),
            Ok(()),
            "consumers -> maestro-path ({kind}{extra})"
        );
    }
}

/// Reject a dev declaration of the utility from a native consumer.
fn native_consumer_may_not_dev_declare_the_utility(from: &str) {
    let workspace = Workspace::new();
    workspace.foundation(&[from, "maestro-path"]);
    workspace.member(
        from,
        from,
        &dependencies_with_utility(required_targets(from), "dev-dependencies", ""),
    );
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(if is_leaf(from) {
            format!("{from} must not depend on workspace crate maestro-path")
        } else {
            format!(
                "internal dev dependency requires declared dependency-free test support: {from} -> maestro-path"
            )
        })
    );
}

/// Reject every declaration form of the utility from a crate outside the permission.
fn excluded_crate_may_not_declare_the_utility(from: &str) {
    let workspace = Workspace::new();
    workspace.foundation(&[from, "maestro-path"]);
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            from,
            from,
            &dependencies_with_utility(required_targets(from), kind, extra),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(if is_leaf(from) {
                format!("{from} must not depend on workspace crate maestro-path")
            } else {
                format!("forbidden production dependency: {from} -> maestro-path")
            })
        );
    }
    workspace.foundation(&[from, "maestro-path"]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

/// Reject every dependency kind from the utility to another workspace crate.
fn path_utility_has_no_outbound_edges() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-path", "maestro-models"]);
    for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
        workspace.member(
            "maestro-path",
            "maestro-path",
            &format!(
                "[{kind}]\nmodels = {{ package = \"maestro-models\", path = \"../maestro-models\" }}"
            ),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("maestro-path must not depend on workspace crate maestro-models".into())
        );
    }
}

/// Verify that the optional utility edge cannot replace the required application edge.
fn path_utility_does_not_replace_the_application() {
    for frontend in [
        "maestro-cli",
        "maestro-chat",
        "maestro-rpc",
        "maestro-web",
        "maestro-multiplexer",
    ] {
        let required = required_targets(frontend);
        let workspace = Workspace::new();
        workspace.foundation(&[&[frontend, "maestro-path"], required].concat());
        workspace.member(
            frontend,
            frontend,
            &dependencies_with_utility(required, "dependencies", ""),
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
        for omitted in required {
            let remaining: Vec<_> = required
                .iter()
                .copied()
                .filter(|target| target != omitted)
                .collect();
            workspace.member(
                frontend,
                frontend,
                &dependencies_with_utility(&remaining, "dependencies", ""),
            );
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "{frontend}: missing required direct dependency {omitted}"
                ))
            );
        }
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
        ("maestro-watch", "core"),
        ("maestro-lock", "core"),
        ("maestro-pattern", "core"),
        ("maestro-process", "core"),
        ("maestro-text", "core"),
        ("maestro-json", "core"),
        ("maestro-entry", "core"),
        ("maestro-tool", "core"),
        ("maestro-brand", "core"),
    ]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    for absent in ["text", "json", "entry", "tool", "brand"] {
        assert!(
            !workspace
                .root
                .path()
                .join(format!("crates/maestro-{absent}"))
                .exists()
        );
    }
    assert!(
        !workspace
            .root
            .path()
            .join("crates/maestro-process")
            .exists()
    );
    assert!(!workspace.root.path().join("crates/maestro-tools").exists());
    workspace.member("tools", "maestro-tools", "");
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    assert!(!workspace.root.path().join("crates/maestro-text").exists());
    assert!(!workspace.root.path().join("crates/maestro-chat").exists());
    assert!(!workspace.root.path().join("crates/maestro-watch").exists());
    assert!(!workspace.root.path().join("crates/maestro-lock").exists());
    assert!(
        !workspace
            .root
            .path()
            .join("crates/maestro-pattern")
            .exists()
    );
    assert!(!workspace.root.path().join("wit").exists());
}

#[test]
fn frontends_require_application_without_unused_allowed_edges() {
    for frontend in [
        "maestro-cli",
        "maestro-chat",
        "maestro-rpc",
        "maestro-web",
        "maestro-multiplexer",
    ] {
        let required = support::policy::POLICY
            .iter()
            .find(|row| row.0 == frontend)
            .unwrap()
            .1;
        let workspace = Workspace::new();
        workspace.foundation(&[&[frontend, "maestro-path"], required].concat());
        check_frontend_omission(&workspace, frontend, &["maestro-app"], "maestro-app");
        for omitted in required.iter().filter(|edge| **edge != "maestro-app") {
            let remaining: Vec<_> = required
                .iter()
                .copied()
                .filter(|edge| edge != omitted)
                .collect();
            workspace.member(
                frontend,
                frontend,
                &dependencies_with_utility(&remaining, "dependencies", ""),
            );
            assert_eq!(check_workspace(workspace.root.path()), Ok(()));
        }
        let mut mixed = String::new();
        for (index, target) in required.iter().enumerate() {
            let kind = DECLARATIONS[[0, 1, 3, 4][index % 4]].0;
            writeln!(
                mixed,
                "[{kind}.edge{index}]\npackage = {target:?}\npath = \"../{target}\""
            )
            .unwrap();
        }
        workspace.member(frontend, frontend, &mixed);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
        workspace.member(frontend, frontend, "");
        assert!(
            check_workspace(workspace.root.path())
                .unwrap_err()
                .contains("missing required direct dependency")
        );
    }
}

#[test]
fn terminal_adapter_and_harness_require_toolkit() {
    for owner in ["maestro-tui-crossterm", "maestro-test-terminal"] {
        let text = if owner == "maestro-tui-crossterm" {
            "maestro-text = { path = \"../maestro-text\" }\n"
        } else {
            ""
        };
        let workspace = Workspace::new();
        workspace.foundation(&[owner, "maestro-models", "maestro-text"]);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
        workspace.member(owner, owner, "");
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(format!(
                "{owner}: missing required direct dependency maestro-tui"
            ))
        );
        for (kind, extra) in DECLARATIONS {
            workspace.member(owner, owner, &format!("[{kind}]\nmaestro-tui = {{ path = \"../maestro-tui\" }}\n{text}models = {{ package = \"maestro-models\", path = \"../maestro-models\"{extra} }}"));
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "forbidden production dependency: {owner} -> maestro-models"
                ))
            );
            workspace.member(
                owner,
                owner,
                &format!("[{kind}]\nmaestro-tui = {{ path = \"../maestro-tui\"{extra} }}\n{text}"),
            );
            assert_eq!(check_workspace(workspace.root.path()), Ok(()));
        }
    }
    for &(from, _) in support::policy::POLICY {
        if from == "maestro-test-terminal" || TERMINAL_CALLERS.contains(&from) {
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
            let error = check_workspace(workspace.root.path()).unwrap_err();
            assert!(
                error.contains(from)
                    && error.contains("maestro-test-terminal")
                    && !error.contains("missing required"),
                "{error}"
            );
        }
    }
    let workspace = Workspace::new();
    let names: Vec<_> = support::policy::POLICY.iter().map(|row| row.0).collect();
    workspace.foundation(&names);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn unapproved_internal_dev_edges_are_rejected_for_every_member() {
    #[cfg(unix)]
    policy_edge_matrix(true);
    #[cfg(not(unix))]
    {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-agent", "maestro-models"]);
        workspace.member(
            "maestro-agent",
            "maestro-agent",
            "[dev-dependencies]\nmaestro-models = { path = \"../maestro-models\" }",
        );
        assert!(
            check_workspace(workspace.root.path())
                .unwrap_err()
                .contains("internal dev dependency")
        );
        workspace.member("maestro-agent", "maestro-agent", "");
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[cfg(unix)]
#[test]
fn metadata_fixtures_run_while_scenario_data_is_open() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-models"]);
    std::fs::write(
        workspace.root.path().join("resolved.json"),
        serde_json::to_vec(&workspace.metadata()).unwrap(),
    )
    .unwrap();
    let cargo = workspace.command("cargo", "cat resolved.json");
    let rustc = workspace.command("rustc", "printf 'host: x86_64-unknown-linux-gnu\\n'");
    let writer = std::fs::OpenOptions::new()
        .write(true)
        .open(&cargo)
        .unwrap();
    workspace.probe(&cargo, &rustc, "ok");
    drop(writer);
}

#[cfg(unix)]
#[test]
fn metadata_queries_leave_compiler_wrappers_to_builds() {
    const PROBE: &str = "MAESTRO_CONVENTIONS_WRAPPER_PROBE";
    if std::env::var_os(PROBE).is_some() {
        let wrapper = std::env::var_os("RUSTC_WRAPPER");
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-models"]);
        let result = check_workspace(workspace.root.path());
        assert_eq!(std::env::var_os("RUSTC_WRAPPER"), wrapper);
        assert_eq!(result, Ok(()));
        return;
    }
    let workspace = Workspace::new();
    let failing =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/metadata-command.sh");
    let missing = workspace.root.path().join("missing-wrapper");
    for wrapper in [None, Some(failing), Some(missing)] {
        let mut command = maestro_test_process::TestProcess::current(
            "metadata_queries_leave_compiler_wrappers_to_builds",
        )
        .unwrap()
        .nocapture()
        .env(PROBE, "1")
        .env_remove("RUSTC_WRAPPER");
        if let Some(wrapper) = wrapper {
            command = command.env("RUSTC_WRAPPER", wrapper);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "wrapper probe failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(unix)]
#[test]
fn metadata_graph_failures_cross_the_public_interface() {
    if let Some(root) = std::env::var_os("MAESTRO_CONVENTIONS_PROBE_ROOT") {
        if let Some(matrix) = std::env::var_os("MAESTRO_CONVENTIONS_POLICY_MATRIX") {
            return policy_edge_cases(std::path::Path::new(&root), matrix == "dev");
        }
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
    workspace.foundation(&["maestro-agent", "maestro-models", "maestro-settings"]);
    workspace.member(
        "maestro-agent",
        "maestro-agent",
        "[dependencies]\nmaestro-models = { path = \"../maestro-models\" }",
    );
    let valid = workspace.metadata();
    workspace.metadata_probe(&valid, &valid, "ok");
    invalid_metadata_identities(&workspace, &valid);
    invalid_resolved_metadata(&workspace, &valid);
    complete_resolved_metadata();
    command_order_and_host_failures(&workspace, &valid);
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
    ] {
        let mut value = valid.clone();
        value
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .shift_remove(key);
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
    value["resolve"]["nodes"][0]["deps"][0]["pkg"] = valid["packages"][2]["id"].clone();
    workspace.metadata_probe(
        valid,
        &value,
        "forbidden production dependency: maestro-agent -> maestro-settings",
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
    let mut full = workspace.metadata();
    for package in full["packages"].as_array_mut().unwrap() {
        package["dependencies"] = json!([]);
    }
    full["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .sort_by_key(|node| {
            let id = node["id"].as_str().unwrap();
            !id.contains("/maestro-cli#")
        });
    // The declared graph alone is incomplete; the resolved direct edges complete it.
    workspace.metadata_probe(&full, &full, "ok");
    let app = full["resolve"]["nodes"][0]["deps"]
        .as_array()
        .unwrap()
        .iter()
        .position(|edge| edge["name"] == "maestro_app")
        .unwrap();
    let mut missing_app = full.clone();
    missing_app["resolve"]["nodes"][0]["deps"]
        .as_array_mut()
        .unwrap()
        .remove(app);
    workspace.metadata_probe(&full, &missing_app, "missing required direct dependency");
    let mut declared = full.clone();
    let actual = workspace.metadata();
    let package = actual["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "maestro-cli")
        .unwrap();
    let mut dependency = package["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "maestro-app")
        .unwrap()
        .clone();
    dependency["kind"] = json!("build");
    dependency["optional"] = json!(true);
    dependency["target"] = json!("cfg(target_os = \"none\")");
    let cli = declared["packages"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["name"] == "maestro-cli")
        .unwrap();
    cli["dependencies"] = json!([dependency]);
    full["resolve"]["nodes"][0]["deps"]
        .as_array_mut()
        .unwrap()
        .remove(app);
    workspace.metadata_probe(&declared, &full, "ok");
}

/// Documented dependencies checked against the permitted policy.
const FOUNDATION_SPECIFICATION: &str = include_str!("../../../docs/specs/maestro-port.md");

/// Backticked names in the foundation specification between the first `start` and the next `end`.
fn specification_names(start: &str, end: &str) -> Vec<&'static str> {
    FOUNDATION_SPECIFICATION
        .split_once(start)
        .unwrap()
        .1
        .split_once(end)
        .unwrap()
        .0
        .split('`')
        .skip(1)
        .step_by(2)
        .collect()
}

#[test]
fn documented_foundation_graph_rejects_missing_and_extra_edges() {
    let row = FOUNDATION_SPECIFICATION
        .lines()
        .find(|line| line.starts_with("| `maestro-theme` |"))
        .unwrap();
    for changed in [
        row.replace("`maestro-watch`, ", ""),
        row.replace("`maestro-tui`,", "`maestro-lock`, `maestro-tui`,"),
    ] {
        assert_ne!(row, changed);
        let specification = FOUNDATION_SPECIFICATION.replace(row, &changed);
        assert!(!documented_foundation_graph_matches_policy(&specification));
    }
}

/// Compare every documented crate and edge with the policy table.
fn documented_foundation_graph_matches_policy(specification: &str) -> bool {
    use std::collections::{BTreeMap, BTreeSet};
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
        .filter(|row| row.0 != "maestro-path")
        .map(|&(name, dependencies)| (name, dependencies.iter().copied().collect()))
        .collect();
    documented == expected
}

/// Declares the targets together in every form; the checker judges and names every edge on its own.
///
/// One manifest declares every form under the same names, which Cargo accepts across tables; the
/// optional form needs a second manifest, because Cargo rejects one package under two names in a
/// table.
fn check_permitted_declarations(workspace: &Workspace, from: &str, targets: &[&str]) {
    let selected: std::collections::BTreeSet<_> = targets
        .iter()
        .chain(support::required(from))
        .copied()
        .collect();
    let renamed: Vec<_> = selected
        .iter()
        .enumerate()
        .filter(|(_, target)| from != "maestro" || !required_targets(from).contains(target))
        .collect();
    for optional in [false, true] {
        let mut declarations = String::new();
        for (kind, extra) in DECLARATIONS
            .iter()
            .filter(|(_, extra)| extra.is_empty() != optional)
        {
            writeln!(declarations, "[{kind}]").unwrap();
            for (index, target) in &renamed {
                writeln!(
                    declarations,
                    "renamed{index} = {{ package = {target:?}, path = \"../{target}\"{extra} }}"
                )
                .unwrap();
            }
            if *kind == "dependencies" {
                append_required_root_wiring(&mut declarations, from, kind);
            }
        }
        workspace.member(from, from, &declarations);
        assert_eq!(
            check_workspace(workspace.root.path()),
            Ok(()),
            "{from} -> {targets:?} (optional: {optional})"
        );
    }
}

/// Keep mandatory root wiring normal while varying other allowed declarations.
fn append_required_root_wiring(declarations: &mut String, from: &str, kind: &str) {
    if from != "maestro" {
        return;
    }
    if kind != "dependencies" {
        declarations.push_str("[dependencies]\n");
    }
    for target in required_targets(from) {
        writeln!(declarations, "{target} = {{ path = \"../{target}\" }}").unwrap();
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
        let error = check_workspace(workspace.root.path()).unwrap_err();
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
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
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
        json!([""]),
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
    for pointer in ["/packages/0/id", "/packages/0/name"] {
        let mut value = valid.clone();
        *value.pointer_mut(pointer).unwrap() = json!("");
        workspace.metadata_probe(&value, valid, "invalid cargo metadata");
        workspace.metadata_probe(valid, &value, "invalid cargo metadata");
    }
    let mut duplicate_directory = valid.clone();
    duplicate_directory["packages"][1]["manifest_path"] =
        valid["packages"][0]["manifest_path"].clone();
    workspace.metadata_probe(
        &duplicate_directory,
        valid,
        "invalid cargo metadata: duplicate member directory",
    );
    let mut duplicate_name = valid.clone();
    duplicate_name["packages"][1]["name"] = json!("maestro-agent");
    workspace.metadata_probe(&duplicate_name, valid, "invalid cargo metadata");
    workspace.metadata_probe(valid, &duplicate_name, "invalid cargo metadata");
}

#[test]
fn test_dependency_cycles_are_rejected() {
    support_edges_keep_cycle_checks(FIXTURES, "maestro-request");
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-agent", "maestro-models"]);
    workspace.member(
        "maestro-agent",
        "maestro-agent",
        "[dependencies]\nmaestro-models = { path = \"../maestro-models\" }",
    );
    workspace.member(
        "maestro-models",
        "maestro-models",
        "[dev-dependencies]\nmaestro-agent = { path = \"../maestro-agent\" }",
    );
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("test dependency cycle: maestro-agent -> maestro-models -> maestro-agent".into())
    );
    workspace.member("maestro-models", "maestro-models", "");
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn maestro_conventions_rejects_foreign_target_reverse_edges() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-agent", "maestro-session"]);
    for (kind, extra) in [
        ("target.'cfg(target_os = \"none\")'.dependencies", ""),
        ("target.'cfg(target_os = \"none\")'.build-dependencies", ""),
        ("dependencies", ", optional = true"),
    ] {
        workspace.member("maestro-agent", "maestro-agent", &format!("[{kind}]\nalias = {{ package = \"maestro-session\", path = \"../maestro-session\"{extra} }}"));
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("forbidden production dependency: maestro-agent -> maestro-session".into())
        );
        workspace.member("maestro-agent", "maestro-agent", "");
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

fn forbidden_policy_matrix() {
    leaf_internal_dependencies_are_rejected();
    core_to_dedicated_dependency_is_rejected();
    #[cfg(unix)]
    policy_edge_matrix(false);
}

#[cfg(unix)]
fn policy_edge_matrix(dev: bool) {
    let workspace = Workspace::new();
    let names: Vec<_> = support::policy::POLICY.iter().map(|row| row.0).collect();
    workspace.foundation(&names);
    workspace.policy_probe(&workspace.metadata(), dev);
}

/// Reject each unpermitted policy edge, in the probe child serving the valid metadata.
#[cfg(unix)]
fn policy_edge_cases(root: &std::path::Path, dev: bool) {
    use serde_json::json;
    let valid: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("resolved.json")).unwrap()).unwrap();
    let template = valid["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "maestro-cli")
        .unwrap()["dependencies"][0]
        .clone();
    let excluded = utility_excluded();
    for &(from, targets) in support::policy::POLICY {
        for &(to, _) in support::policy::POLICY {
            let utility_edge = to == "maestro-path" && !excluded.contains(&from);
            let support_edge = support::policy::test_support_callers(to).is_some();
            let approved = dev && support::policy::permitted_development(from, to);
            if from == to
                || support_edge
                || approved
                || (!dev && (targets.contains(&to) || utility_edge))
            {
                continue;
            }
            let mut declared = valid.clone();
            for package in declared["packages"].as_array_mut().unwrap() {
                package["dependencies"] = json!([]);
            }
            let package = declared["packages"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|p| p["name"] == from)
                .unwrap();
            let mut dependency = template.clone();
            dependency["name"] = json!(to);
            dependency["rename"] = json!("alias");
            dependency["path"] = json!(root.join("crates").join(to));
            dependency["kind"] = if dev { json!("dev") } else { json!(null) };
            package["dependencies"] = json!([dependency]);
            let expected = if targets.is_empty() {
                format!("{from} must not depend on workspace crate {to}")
            } else if support::class(from) == "core" && support::class(to) == "dedicated" {
                format!("core crate {from} must not depend on dedicated crate {to}")
            } else if dev {
                format!(
                    "internal dev dependency requires declared dependency-free test support: {from} -> {to}"
                )
            } else {
                format!("forbidden production dependency: {from} -> {to}")
            };
            let result = check_declared(root, &declared);
            assert!(
                matches!(&result, Err(error) if error.contains(&expected)),
                "{from} -> {to}: {result:?}"
            );
        }
    }
    assert_eq!(check_declared(root, &valid), Ok(()));
}

/// Serve `declared` to the probe's declared pass, then check the probe workspace.
#[cfg(unix)]
fn check_declared(root: &std::path::Path, declared: &serde_json::Value) -> Result<(), String> {
    std::fs::write(
        root.join("declared.json"),
        serde_json::to_vec(declared).unwrap(),
    )
    .unwrap();
    check_workspace(root)
}

#[cfg(unix)]
fn command_order_and_host_failures(workspace: &Workspace, valid: &serde_json::Value) {
    workspace.metadata_probe(valid, valid, "ok");
    let cargo = workspace.command(
        "ordered-cargo",
        "printf 'cargo %s\\n' \"$*\" >> order; cat resolved.json",
    );
    let rustc = workspace.command(
        "ordered-rustc",
        &format!(
            "printf 'rustc %s\\n' \"$*\" >> {log:?}; printf 'host: x86_64-unknown-linux-gnu\\n'",
            log = workspace.root.path().join("order")
        ),
    );
    workspace.probe(&cargo, &rustc, "ok");
    let log = std::fs::read_to_string(workspace.root.path().join("order")).unwrap();
    let lines: Vec<_> = log.lines().collect();
    assert_eq!(lines.len(), 3, "{log}");
    assert!(
        lines[0].contains("--no-deps") && lines[0].contains("--offline"),
        "{log}"
    );
    assert_eq!(lines[1], "rustc -vV");
    assert!(
        lines[2].contains("--filter-platform x86_64-unknown-linux-gnu")
            && lines[2].contains("--offline"),
        "{log}"
    );
    for line in [lines[0], lines[2]] {
        assert!(
            line.contains("--format-version 1")
                && !line.contains("--all-features")
                && !line.contains("--no-default-features"),
            "{log}"
        );
    }
    compiler_host_failures(workspace, &cargo);
}

#[cfg(unix)]
fn compiler_host_failures(workspace: &Workspace, cargo: &std::path::Path) {
    workspace.probe(
        cargo,
        &workspace.root.path().join("missing-rustc"),
        "rustc -vV failed:",
    );
    for (body, expected) in [
        (
            "echo compiler-failure >&2; exit 9",
            "rustc -vV failed (exit status: 9): compiler-failure",
        ),
        (
            "printf 'release: 1.98.0\\n'",
            "rustc -vV failed: missing or empty host line",
        ),
        (
            "printf 'host:    \\n'",
            "rustc -vV failed: missing or empty host line",
        ),
    ] {
        let rustc = workspace.command("failed-rustc", body);
        workspace.probe(cargo, &rustc, expected);
        workspace.probe(
            cargo,
            &workspace.command("valid-rustc", "printf 'host: x86_64-unknown-linux-gnu\\n'"),
            "ok",
        );
    }
}

#[test]
fn terminal_adapter_text_edge_is_scoped() {
    const ADAPTER: &str = "maestro-tui-crossterm";
    let toolkit = "maestro-tui = { path = \"../maestro-tui\" }\n";
    let plain_text = "maestro-text = { path = \"../maestro-text\" }\n";
    let workspace = Workspace::new();
    workspace.foundation(&[ADAPTER, "maestro-text", "maestro-path"]);
    workspace.member(ADAPTER, ADAPTER, &format!("[dependencies]\n{toolkit}"));
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "{ADAPTER}: missing required direct dependency maestro-text"
        ))
    );
    workspace.member(
        ADAPTER,
        ADAPTER,
        &format!("[dependencies]\n{toolkit}{plain_text}"),
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            ADAPTER,
            ADAPTER,
            &format!("[{kind}]\n{toolkit}renamed = {{ package = \"maestro-text\", path = \"../maestro-text\"{extra} }}"),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Ok(()),
            "{kind}{extra}"
        );
    }
    let missing = format!("{ADAPTER}: missing required direct dependency maestro-tui");
    let sparse = Workspace::new();
    sparse.member("maestro-text", "maestro-text", "");
    sparse.member("maestro-path", "maestro-path", "");
    sparse.list(&[
        (ADAPTER, "core"),
        ("maestro-text", "core"),
        ("maestro-path", "core"),
    ]);
    for only in [
        "[dependencies]\nmaestro-text = { path = \"../maestro-text\" }",
        "[dependencies]\nmaestro-path = { path = \"../maestro-path\" }",
    ] {
        sparse.member(ADAPTER, ADAPTER, only);
        assert_eq!(
            check_workspace(sparse.root.path()),
            Err(missing.clone()),
            "{only}"
        );
        workspace.member(ADAPTER, ADAPTER, only);
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(missing.clone()),
            "{only}"
        );
    }
    adapter_text_may_not_be_dev_only(&workspace, toolkit);
    harness_may_not_declare_text(toolkit);
}

/// Reject the text owner as an adapter dev-dependency.
fn adapter_text_may_not_be_dev_only(workspace: &Workspace, toolkit: &str) {
    const ADAPTER: &str = "maestro-tui-crossterm";
    workspace.member(
        ADAPTER,
        ADAPTER,
        &format!("[dependencies]\n{toolkit}[dev-dependencies]\nmaestro-text = {{ path = \"../maestro-text\" }}"),
    );
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "internal dev dependency requires declared dependency-free test support: {ADAPTER} -> maestro-text"
        ))
    );
}

/// Reject the text owner from the terminal scenario harness.
fn harness_may_not_declare_text(toolkit: &str) {
    let harness = Workspace::new();
    harness.foundation(&["maestro-test-terminal", "maestro-text"]);
    harness.member(
        "maestro-test-terminal",
        "maestro-test-terminal",
        &format!("[dependencies]\n{toolkit}maestro-text = {{ path = \"../maestro-text\" }}"),
    );
    assert_eq!(
        check_workspace(harness.root.path()),
        Err("forbidden production dependency: maestro-test-terminal -> maestro-text".into())
    );
}

#[test]
fn terminal_example_brand_edge_is_development_only() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tui-crossterm", "maestro-brand"]);
    let required = "[dependencies]\nmaestro-tui = { path = \"../maestro-tui\" }\nmaestro-text = { path = \"../maestro-text\" }\n";
    for kind in [
        "dev-dependencies",
        "target.'cfg(unix)'.dev-dependencies",
        "target.'cfg(target_os = \"none\")'.dev-dependencies",
    ] {
        workspace.member("maestro-tui-crossterm", "maestro-tui-crossterm", &format!("{required}[{kind}]\nbrand = {{ package = \"maestro-brand\", path = \"../maestro-brand\" }}"));
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{kind}");
    }
    for (kind, extra) in DECLARATIONS {
        workspace.member("maestro-tui-crossterm", "maestro-tui-crossterm", &format!("[{kind}]\nmaestro-tui = {{ path = \"../maestro-tui\" }}\nmaestro-text = {{ path = \"../maestro-text\" }}\nbrand = {{ package = \"maestro-brand\", path = \"../maestro-brand\"{extra} }}"));
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("forbidden production dependency: maestro-tui-crossterm -> maestro-brand".into()),
            "{kind}{extra}"
        );
    }
}

#[test]
fn native_component_fixture_edges_are_development_only() {
    for dependency in [
        "maestro-catalog",
        "maestro-credentials",
        "maestro-models",
        "maestro-session",
        "maestro-theme",
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-extensions-wasmtime", dependency]);
        workspace.member("maestro-extensions-wasmtime", "maestro-extensions-wasmtime", &format!(
            "[target.'cfg(not(target_arch = \"wasm32\"))'.dev-dependencies]\nalias = {{ package = {dependency:?}, path = \"../{dependency}\" }}"
        ));
        assert_eq!(
            check_workspace(workspace.root.path()),
            Ok(()),
            "{dependency}"
        );
        for (kind, extra) in DECLARATIONS {
            workspace.member("maestro-extensions-wasmtime", "maestro-extensions-wasmtime", &format!(
                "[{kind}]\nalias = {{ package = {dependency:?}, path = \"../{dependency}\"{extra} }}"
            ));
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "forbidden production dependency: maestro-extensions-wasmtime -> {dependency}"
                )),
                "{dependency} {kind}{extra}"
            );
        }
    }
}

#[test]
fn extension_credentials_edge_is_development_only() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions", "maestro-credentials"]);
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            "maestro-extensions",
            "maestro-extensions",
            &format!(
                "[{kind}]\nmaestro-credentials = {{ path = \"../maestro-credentials\"{extra} }}"
            ),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(
                "forbidden production dependency: maestro-extensions -> maestro-credentials".into()
            ),
            "{kind}{extra}"
        );
    }
}

#[test]
fn rpc_child_rendering_edges_are_development_only() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-rpc", "maestro-tui-crossterm"]);
    let application = "maestro-app = { path = \"../maestro-app\" }";
    for target in ["maestro-tui-crossterm"] {
        for (kind, extra) in DECLARATIONS {
            let declared = format!("{target} = {{ path = \"../{target}\"{extra} }}");
            let manifest = if *kind == "dependencies" {
                format!("[dependencies]\n{application}\n{declared}")
            } else {
                format!("[dependencies]\n{application}\n[{kind}]\n{declared}")
            };
            workspace.member("maestro-rpc", "maestro-rpc", &manifest);
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "forbidden production dependency: maestro-rpc -> {target}"
                )),
                "{kind}{extra} {target}"
            );
        }
    }
}

#[test]
fn chat_scripted_provider_edge_is_development_only() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat", "maestro-models"]);
    let application = "maestro-app = { path = \"../maestro-app\" }";
    let models = "models = { package = \"maestro-models\", path = \"../maestro-models\"";
    workspace.member(
        "maestro-chat",
        "maestro-chat",
        &format!(
            "[dependencies]\n{application}\n[target.'cfg(not(target_arch = \"wasm32\"))'.dev-dependencies]\n{models} }}"
        ),
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            "maestro-chat",
            "maestro-chat",
            &format!("[{kind}]\n{application}\n{models}{extra} }}"),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("forbidden production dependency: maestro-chat -> maestro-models".into()),
            "{kind}{extra}"
        );
    }
}

#[test]
fn terminal_example_brand_edge_keeps_required_runtime_dependencies() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tui-crossterm", "maestro-brand"]);
    for (retained, missing) in [
        ("maestro-text", "maestro-tui"),
        ("maestro-tui", "maestro-text"),
    ] {
        workspace.member("maestro-tui-crossterm", "maestro-tui-crossterm", &format!("[dependencies]\n{retained} = {{ path = \"../{retained}\" }}\n[dev-dependencies]\nmaestro-brand = {{ path = \"../maestro-brand\" }}"));
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(format!(
                "maestro-tui-crossterm: missing required direct dependency {missing}"
            ))
        );
    }
}

#[cfg(unix)]
#[test]
fn terminal_example_brand_edge_checks_host_resolved_dependency_kind() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-tui-crossterm", "maestro-brand"]);
    workspace.member("maestro-tui-crossterm", "maestro-tui-crossterm", "[dependencies]\nmaestro-tui = { path = \"../maestro-tui\" }\nmaestro-text = { path = \"../maestro-text\" }\n[dev-dependencies]\nmaestro-brand = { path = \"../maestro-brand\" }");
    let valid = workspace.metadata();
    workspace.metadata_probe(&valid, &valid, "ok");
    for kind in [serde_json::Value::Null, serde_json::json!("build")] {
        let mut resolved = valid.clone();
        let node = resolved["resolve"]["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| {
                node["id"]
                    .as_str()
                    .unwrap()
                    .contains("maestro-tui-crossterm")
            })
            .unwrap();
        let edge = node["deps"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|edge| edge["name"] == "maestro_brand")
            .unwrap();
        edge["dep_kinds"][0]["kind"] = kind;
        workspace.metadata_probe(
            &valid,
            &resolved,
            "forbidden production dependency: maestro-tui-crossterm -> maestro-brand",
        );
    }
}

#[test]
fn complete_permitted_graph_is_acyclic() {
    let names: Vec<_> = support::policy::POLICY.iter().map(|row| row.0).collect();
    let workspace = Workspace::new();
    workspace.foundation(&names);
    let excluded = utility_excluded();
    for (kind, extra) in DECLARATIONS {
        for &(from, targets) in support::policy::POLICY {
            let mut declarations = format!("[{kind}]\n");
            let utility =
                (from != "maestro-path" && !excluded.contains(&from)).then_some("maestro-path");
            for (index, target) in targets
                .iter()
                .copied()
                .chain(utility)
                .filter(|target| from != "maestro" || !required_targets(from).contains(target))
                .enumerate()
            {
                writeln!(
                    declarations,
                    "edge{index} = {{ package = {target:?}, path = \"../{target}\"{extra} }}"
                )
                .unwrap();
            }
            append_required_root_wiring(&mut declarations, from, kind);
            workspace.member(from, from, &declarations);
        }
        assert_eq!(
            check_workspace(workspace.root.path()),
            Ok(()),
            "{kind}{extra}"
        );
    }
}

/// The feature unifier's package name.
const UNIFIER: &str = "maestro-dependencies";

/// The only target table that may declare the unifier.
const NATIVE: &str = "target.'cfg(not(target_arch = \"wasm32\"))'.dependencies";

/// Manifest declaring the unifier through one section and extra entry fields.
fn unifier_declaration(required: &[&str], section: &str, alias: &str, extra: &str) -> String {
    let mut text = String::from("[dependencies]\n");
    for target in required {
        writeln!(text, "{target} = {{ path = \"../{target}\" }}").unwrap();
    }
    if section != "dependencies" {
        writeln!(text, "[{section}]").unwrap();
    }
    writeln!(
        text,
        "{alias} = {{ package = {UNIFIER:?}, path = \"../{UNIFIER}\"{extra} }}"
    )
    .unwrap();
    text
}

#[test]
fn feature_unifier_is_a_native_build_dependency_leaf() {
    for from in [
        "maestro-path",
        "maestro-models",
        "maestro-app",
        "maestro-tooling",
        "maestro-test-conventions",
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&[from, UNIFIER]);
        let required = required_targets(from);
        for alias in [UNIFIER, "unified"] {
            workspace.member(
                from,
                from,
                &unifier_declaration(required, NATIVE, alias, ""),
            );
            assert_eq!(
                check_workspace(workspace.root.path()),
                Ok(()),
                "{from} {alias}"
            );
        }
        for (section, extra) in [
            ("dependencies", ""),
            ("target.'cfg(target_arch = \"wasm32\")'.dependencies", ""),
            (NATIVE, ", optional = true"),
            ("build-dependencies", ""),
            ("dev-dependencies", ""),
            ("target.'cfg(unix)'.dependencies", ""),
        ] {
            workspace.member(
                from,
                from,
                &unifier_declaration(required, section, "unified", extra),
            );
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "{from}: {UNIFIER} must be a native, normal, non-optional dependency"
                )),
                "{from} [{section}]{extra}"
            );
        }
    }
    unifier_has_no_outbound_edges();
    unifier_class_is_dedicated();
}

/// Reject every dependency kind from the unifier to any workspace crate, utility included.
fn unifier_has_no_outbound_edges() {
    let workspace = Workspace::new();
    workspace.foundation(&[UNIFIER, "maestro-path", "maestro-models"]);
    for target in ["maestro-path", "maestro-models"] {
        for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
            workspace.member(
                UNIFIER,
                UNIFIER,
                &format!("[{kind}]\nx = {{ package = {target:?}, path = \"../{target}\" }}"),
            );
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "{UNIFIER} must not depend on workspace crate {target}"
                ))
            );
        }
    }
}

/// Require the dedicated classification and keep core crates out of the dedicated exception.
fn unifier_class_is_dedicated() {
    let workspace = Workspace::new();
    workspace.foundation(&[UNIFIER]);
    workspace.list(&[(UNIFIER, "core")]);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "invalid scoped layer for {UNIFIER}: expected dedicated"
        ))
    );
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-models", "maestro-tooling", UNIFIER]);
    workspace.member(
        "maestro-models",
        "maestro-models",
        "[dependencies]\ntooling = { package = \"maestro-tooling\", path = \"../maestro-tooling\" }",
    );
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("core crate maestro-models must not depend on dedicated crate maestro-tooling".into())
    );
}

#[test]
fn feature_unifier_preserves_required_domain_edges() {
    for from in [
        "maestro-cli",
        "maestro-chat",
        "maestro-rpc",
        "maestro-web",
        "maestro-tui-crossterm",
        "maestro-test-terminal",
    ] {
        let required = required_targets(from);
        let workspace = Workspace::new();
        workspace.foundation(&[&[from, UNIFIER], required].concat());
        workspace.member(
            from,
            from,
            &unifier_declaration(required, NATIVE, "unified", ""),
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{from}");
        for omitted in required {
            let remaining: Vec<_> = required.iter().copied().filter(|t| t != omitted).collect();
            workspace.member(
                from,
                from,
                &unifier_declaration(&remaining, NATIVE, "unified", ""),
            );
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "{from}: missing required direct dependency {omitted}"
                ))
            );
        }
    }
}

const OWNER: &str = "maestro-test-process";
const NATIVE_DEV: &str = "target.'cfg(not(target_arch = \"wasm32\"))'.dev-dependencies";

/// Callers the specification lets declare the test-process owner, in its order.
fn owner_callers() -> Vec<&'static str> {
    std::iter::once("maestro")
        .chain(specification_names(
            "from `maestro`",
            "; a\ncaller declares it only",
        ))
        .collect()
}

/// Install `caller` with its required dependencies followed by `declaration` and check the graph.
fn caller_declares(workspace: &Workspace, caller: &str, declaration: &str) -> Result<(), String> {
    let mut manifest = String::new();
    let required = required_targets(caller);
    if !required.is_empty() {
        manifest.push_str("[dependencies]\n");
        for target in required {
            writeln!(manifest, "{target} = {{ path = \"../{target}\" }}").unwrap();
        }
    }
    let declaration = if manifest.is_empty() {
        declaration
    } else {
        declaration
            .strip_prefix("[dependencies]\n")
            .unwrap_or(declaration)
    };
    manifest.push_str(declaration);
    workspace.member(caller, caller, &manifest);
    check_workspace(workspace.root.path()).map(drop)
}

/// A dependency entry that reaches the owner through a renamed alias.
fn owner_entry(extra: &str) -> String {
    format!("support = {{ package = {OWNER:?}, path = \"../{OWNER}\"{extra} }}\n")
}

#[test]
fn test_process_edges_are_native_development_only() {
    support_declarations_match_owner(OWNER, &owner_callers());
}

#[test]
fn test_process_owner_stays_dependency_free() {
    support_owner_has_no_domain_dependencies(OWNER);
}

#[test]
fn test_process_permission_is_not_a_name_pattern() {
    support_permissions_are_exact(OWNER, &owner_callers());
}

#[test]
fn test_process_edges_keep_cycle_checks() {
    support_edges_keep_cycle_checks(OWNER, "maestro-agent");
}

#[test]
fn test_process_policy_keeps_existing_domain_edges() {
    let workspace = Workspace::new();
    workspace.foundation(&[
        "maestro-tui-crossterm",
        "maestro-brand",
        "maestro-models",
        OWNER,
    ]);
    let required = "maestro-tui = { path = \"../maestro-tui\" }\nmaestro-text = { path = \"../maestro-text\" }\n";
    for (extra, accepted) in [
        (
            "[dev-dependencies]\nb = { package = \"maestro-brand\", path = \"../maestro-brand\" }\n",
            true,
        ),
        (
            "[dev-dependencies]\nb = { package = \"maestro-models\", path = \"../maestro-models\" }\n",
            false,
        ),
    ] {
        workspace.member(
            "maestro-tui-crossterm",
            "maestro-tui-crossterm",
            &format!(
                "[dependencies]\n{required}{extra}[{NATIVE_DEV}]\n{}",
                owner_entry("")
            ),
        );
        assert_eq!(check_workspace(workspace.root.path()).is_ok(), accepted);
    }
}

#[test]
fn test_process_inventory_matches_foundation_contract() {
    support_inventory_matches_contract(
        OWNER,
        support::policy::TEST_PROCESS_CALLERS,
        &owner_callers(),
    );
}

#[test]
fn workspace_daemon_owners_are_core() {
    for name in [
        "maestro-protocol",
        "maestro-ipc",
        "maestro-terminal",
        "maestro-workspace",
        "maestro-detection",
        "maestro-daemon",
        "maestro-multiplexer",
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&[name]);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{name}");
        let path = workspace.root.path().join("workspace-crates.json");
        let mut inventory: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        inventory[name] = "dedicated".into();
        std::fs::write(&path, serde_json::to_vec_pretty(&inventory).unwrap()).unwrap();
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(format!("invalid scoped layer for {name}: expected core"))
        );
    }
}

#[test]
fn daemon_requires_the_terminal_engine() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-daemon"]);
    workspace.member("maestro-daemon", "maestro-daemon", "");
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("maestro-daemon: missing required direct dependency maestro-terminal".into())
    );
    workspace.foundation(&["maestro-daemon"]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn workspace_root_rejects_optional_wiring() {
    check_root_dependency_form("dependencies", ", optional = true");
}

#[test]
fn workspace_root_rejects_build_only_wiring() {
    check_root_dependency_form("build-dependencies", "");
}

#[test]
fn workspace_root_rejects_inactive_target_wiring() {
    check_root_dependency_form("target.'cfg(target_os = \"none\")'.dependencies", "");
}

/// Reject each runtime dependency's invalid form while retaining its neighbours.
fn check_root_dependency_form(kind: &str, extra: &str) {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro"]);
    for target in required_targets("maestro") {
        let mut declarations = String::from("[dependencies]\n");
        for other in required_targets("maestro")
            .iter()
            .filter(|other| *other != target)
        {
            writeln!(declarations, "{other} = {{ path = \"../{other}\" }}").unwrap();
        }
        if kind != "dependencies" {
            writeln!(declarations, "[{kind}]").unwrap();
        }
        writeln!(
            declarations,
            "renamed = {{ package = {target:?}, path = \"../{target}\"{extra} }}"
        )
        .unwrap();
        workspace.member("maestro", "maestro", &declarations);
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(format!(
                "maestro: missing required direct dependency {target}"
            )),
            "{target} ({kind}{extra})"
        );
    }
    workspace.foundation(&["maestro"]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn workspace_root_accepts_native_target_wiring() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro"]);
    let mut declarations =
        String::from("[target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]\n");
    for target in required_targets("maestro") {
        writeln!(declarations, "{target} = {{ path = \"../{target}\" }}").unwrap();
    }
    workspace.member("maestro", "maestro", &declarations);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn workspace_root_requires_application_wiring() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro"]);
    workspace.member("maestro", "maestro", "");
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("maestro: missing required direct dependency maestro-app".into())
    );
    workspace.member(
        "maestro",
        "maestro",
        "[dependencies]\nmaestro-app = { path = \"../maestro-app\" }",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn detection_consumes_foreground_process_facts() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-process"]);
    workspace.member(
        "maestro-detection",
        "maestro-detection",
        "[dependencies]\nmaestro-process = { path = \"../maestro-process\" }",
    );
    workspace.list(&[("maestro-process", "core"), ("maestro-detection", "core")]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn detection_text_test_edge_is_native() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-detection", "maestro-text"]);
    workspace.member("maestro-detection", "maestro-detection", "[target.'cfg(not(target_arch = \"wasm32\"))'.dev-dependencies]\nmaestro-text = { path = \"../maestro-text\" }");
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    workspace.member(
        "maestro-detection",
        "maestro-detection",
        "[dev-dependencies]\nmaestro-text = { path = \"../maestro-text\" }",
    );
    assert_eq!(check_workspace(workspace.root.path()), Err("maestro-detection: maestro-text test support must be a native, non-optional development dependency".into()));
}

#[test]
fn chat_models_test_edge_is_native() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat", "maestro-models"]);
    let production = "[dependencies]\nmaestro-app = { path = \"../maestro-app\" }\n";
    let rejected = "maestro-chat: maestro-models test support must be a native, non-optional development dependency";
    for declarations in [
        "[dev-dependencies]\nmaestro-models = { path = \"../maestro-models\" }",
        "[target.'cfg(target_arch = \"wasm32\")'.dev-dependencies]\nmaestro-models = { path = \"../maestro-models\" }",
    ] {
        workspace.member(
            "maestro-chat",
            "maestro-chat",
            &format!("{production}{declarations}"),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(rejected.into()),
            "{declarations}"
        );
    }
    workspace.member("maestro-chat", "maestro-chat", &format!("{production}[target.'cfg(not(target_arch = \"wasm32\"))'.dev-dependencies]\nmaestro-models = {{ path = \"../maestro-models\" }}"));
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn extension_and_rpc_test_edges_are_native() {
    for (owner, target) in [
        ("maestro-extensions", "maestro-credentials"),
        ("maestro-rpc", "maestro-theme"),
        ("maestro-rpc", "maestro-tui-crossterm"),
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&[owner, target]);
        let manifest = |table: &str| {
            let mut manifest = String::from("[dependencies]\n");
            for required in required_targets(owner) {
                writeln!(manifest, "{required} = {{ path = \"../{required}\" }}").unwrap();
            }
            writeln!(
                manifest,
                "[{table}]\n{target} = {{ path = \"../{target}\" }}"
            )
            .unwrap();
            manifest
        };
        workspace.member(owner, owner, &manifest(WITNESS_DEV));
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{target}");
        for table in [
            "dev-dependencies",
            "target.'cfg(target_arch = \"wasm32\")'.dev-dependencies",
        ] {
            workspace.member(owner, owner, &manifest(table));
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "{owner}: {target} test support must be a native, non-optional development dependency"
                )),
                "{target} in {table}"
            );
        }
    }
}

#[test]
fn session_lock_edge_is_scoped() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-session", "maestro-lock"]);
    check_permitted_declarations(&workspace, "maestro-session", &["maestro-lock"]);

    let workspace = Workspace::new();
    workspace.foundation(&["maestro-models", "maestro-lock"]);
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            "maestro-models",
            "maestro-models",
            &format!("[{kind}]\nalias = {{ package = \"maestro-lock\", path = \"../maestro-lock\"{extra} }}"),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("forbidden production dependency: maestro-models -> maestro-lock".into()),
            "{kind}{extra}"
        );
    }
}

#[test]
fn detection_catalog_uses_atomic_replacement() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-lock"]);
    workspace.member(
        "maestro-detection",
        "maestro-detection",
        r#"[dependencies]
maestro-lock = { path = "../maestro-lock" }"#,
    );
    workspace.list(&[("maestro-lock", "core"), ("maestro-detection", "core")]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

/// The portable fixture owner.
const FIXTURES: &str = "maestro-test-fixtures";

/// Independently declared fixture consumers.
const FIXTURE_CALLERS: &[&str] = &[
    "maestro-agent",
    "maestro-models",
    "maestro-request",
    "maestro-catalog",
    "maestro-chat",
    "maestro-credentials",
    "maestro-tools",
    "maestro-export",
];

/// The disposable-directory owner.
const DIRECTORY: &str = "maestro-test-directory";

/// The scoped test tracing owner.
const TRACING: &str = "maestro-test-tracing";

/// The recording-terminal and manual-runtime owner.
const TERMINAL: &str = "maestro-test-terminal";

/// Independently declared terminal scenario consumers.
const TERMINAL_CALLERS: &[&str] = &["maestro-cli"];

/// Independently declared native tracing consumers.
const TRACING_CALLERS: &[&str] = &[
    "maestro-daemon",
    "maestro-models",
    "maestro-process",
    "maestro-terminal",
    "maestro-workspace",
];

/// Independently declared native directory consumers.
const DIRECTORY_CALLERS: &[&str] = &[
    "maestro",
    "maestro-app",
    "maestro-catalog",
    "maestro-chat",
    "maestro-cli",
    "maestro-credentials",
    "maestro-daemon",
    "maestro-detection",
    "maestro-export",
    "maestro-extensions",
    "maestro-extensions-wasmtime",
    "maestro-ipc",
    "maestro-listing",
    "maestro-lock",
    "maestro-models",
    "maestro-packages",
    "maestro-process",
    "maestro-resources",
    "maestro-rpc",
    "maestro-session",
    "maestro-settings",
    "maestro-terminal",
    "maestro-test-conventions",
    "maestro-test-process",
    "maestro-theme",
    "maestro-tooling",
    "maestro-tools",
    "maestro-tui",
    "maestro-tui-crossterm",
    "maestro-watch",
    "maestro-workspace",
];

#[test]
fn directory_support_edges_are_native_development_only() {
    support_declarations_match_owner(DIRECTORY, DIRECTORY_CALLERS);
}

#[test]
fn tracing_support_edges_are_native_development_only() {
    support_declarations_match_owner(TRACING, TRACING_CALLERS);
}

#[test]
fn terminal_support_edges_are_native_development_only() {
    support_declarations_match_owner(TERMINAL, TERMINAL_CALLERS);
}

/// Check the permitted declaration forms for each support owner and its callers.
fn support_declarations_match_owner(owner: &str, callers: &[&str]) {
    for &caller in callers {
        let workspace = Workspace::new();
        workspace.foundation(&[caller, owner]);
        for (kind, extra, accepted) in [
            (NATIVE_DEV, "", true),
            ("dev-dependencies", "", false),
            ("dependencies", "", false),
            ("build-dependencies", "", false),
            ("dependencies", ", optional = true", false),
            (
                "target.'cfg(target_arch = \"wasm32\")'.dev-dependencies",
                "",
                false,
            ),
            (
                "target.'cfg(target_os = \"none\")'.dev-dependencies",
                "",
                false,
            ),
        ] {
            let declaration = format!(
                "[{kind}]\nsupport = {{ package = {owner:?}, path = \"../{owner}\"{extra} }}\n"
            );
            let result = caller_declares(&workspace, caller, &declaration);
            let expected = if owner == FIXTURES && !kind.ends_with("dev-dependencies") {
                Err(format!(
                    "{owner} is reachable only as a development dependency of its declared callers: {caller} -> {owner}"
                ))
            } else if owner == FIXTURES || accepted {
                Ok(())
            } else {
                Err(format!(
                    "{caller}: {owner} must be a native, non-optional development dependency"
                ))
            };
            assert_eq!(result, expected, "{caller} {kind}{extra}");
        }
    }
}

#[test]
fn directory_support_owner_has_no_domain_dependencies() {
    support_owner_has_no_domain_dependencies(FIXTURES);
    support_owner_has_no_domain_dependencies(DIRECTORY);
}

#[test]
fn tracing_support_owner_has_no_domain_dependencies() {
    support_owner_has_no_domain_dependencies(TRACING);
}

/// `owner` rejects the text, path and tooling crates in every dependency table and accepts
/// the dependency unifier.
fn support_owner_has_no_domain_dependencies(owner: &str) {
    let targets = ["maestro-text", "maestro-path", "maestro-tooling"];
    let workspace = Workspace::new();
    workspace.foundation(&[owner, targets[0], targets[1], targets[2], UNIFIER]);
    for target in targets {
        for table in [
            "dependencies",
            "dev-dependencies",
            "build-dependencies",
            NATIVE_DEV,
        ] {
            workspace.member(
                owner,
                owner,
                &format!("[{table}]\nx = {{ package = {target:?}, path = \"../{target}\" }}"),
            );
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "{owner} must not depend on workspace crate {target}"
                )),
                "{table}"
            );
        }
    }
    workspace.member(
        owner,
        owner,
        &unifier_declaration(&[], NATIVE, "unified", ""),
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn directory_support_permissions_are_exact() {
    support_permissions_are_exact(FIXTURES, FIXTURE_CALLERS);
    support_permissions_are_exact(DIRECTORY, DIRECTORY_CALLERS);
}

#[test]
fn tracing_support_permissions_are_exact() {
    support_permissions_are_exact(TRACING, TRACING_CALLERS);
}

/// Only `owner`'s declared callers may declare it; no crate-name pattern grants access.
fn support_permissions_are_exact(owner: &str, callers: &[&str]) {
    let native =
        format!("[{NATIVE_DEV}]\nsupport = {{ package = {owner:?}, path = \"../{owner}\" }}");
    for &(caller, _) in support::policy::POLICY {
        if callers.contains(&caller) || caller == owner {
            continue;
        }
        let workspace = Workspace::new();
        workspace.foundation(&[caller, owner]);
        assert_eq!(
            caller_declares(&workspace, caller, &native),
            Err(format!(
                "{owner} is reachable only as a {}development dependency of its declared callers: {caller} -> {owner}",
                if owner == FIXTURES { "" } else { "native " }
            ))
        );
    }
    assert!(support::policy::test_support_callers("maestro-test-other").is_none());
    let workspace = Workspace::new();
    workspace.foundation(&[callers[0], owner]);
    assert_eq!(caller_declares(&workspace, callers[0], &native), Ok(()));
    workspace.member("maestro-test-other", "maestro-test-other", &native);
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("maestro-test-other")
    );
}

#[test]
fn directory_support_edges_keep_cycle_checks() {
    support_edges_keep_cycle_checks(DIRECTORY, "maestro-models");
    let (owner, helper) = ("maestro-models", "maestro-agent");
    let workspace = Workspace::new();
    workspace.foundation(&[owner, helper]);
    workspace.member(
        helper,
        helper,
        "[dependencies]\nmaestro-models = { path = \"../maestro-models\" }",
    );
    let declaration =
        format!("[dev-dependencies]\nsupport = {{ package = {helper:?}, path = \"../{helper}\" }}");
    assert_eq!(
        caller_declares(&workspace, owner, &declaration),
        Err(format!(
            "test dependency cycle: {helper} -> {owner} -> {helper}"
        ))
    );
}

#[test]
fn tracing_support_edges_keep_cycle_checks() {
    support_edges_keep_cycle_checks(TRACING, "maestro-daemon");
}

#[test]
fn terminal_support_edges_keep_cycle_checks() {
    support_edges_keep_cycle_checks(TERMINAL, "maestro-cli");
}

/// A native development edge from `caller` to `owner` still meets the test-graph cycle check.
fn support_edges_keep_cycle_checks(owner: &str, caller: &str) {
    let workspace = Workspace::new();
    workspace.foundation(&[caller, owner]);
    workspace.member(
        owner,
        owner,
        &format!("[dev-dependencies]\nback = {{ package = {caller:?}, path = \"../{caller}\" }}"),
    );
    let native =
        format!("[{NATIVE_DEV}]\nsupport = {{ package = {owner:?}, path = \"../{owner}\" }}");
    assert_eq!(
        caller_declares(&workspace, caller, &native),
        Err(format!(
            "test dependency cycle: {caller} -> {owner} -> {caller}"
        ))
    );
}

#[test]
fn directory_support_inventory_matches_contract() {
    assert_eq!(
        FIXTURE_CALLERS,
        support::policy::test_support_callers(FIXTURES).unwrap()
    );
    let documented = specification_names("Fixture development callers are", "; declarations");
    support_inventory_matches_contract(FIXTURES, FIXTURE_CALLERS, &documented);
    assert_eq!(DIRECTORY_CALLERS, support::policy::TEST_DIRECTORY_CALLERS);
    let documented = std::iter::once("maestro")
        .chain(specification_names(
            "`cfg(not(target_arch = \"wasm32\"))` from `maestro`,",
            "; a caller\ndeclares it only",
        ))
        .collect::<Vec<_>>();
    support_inventory_matches_contract(DIRECTORY, DIRECTORY_CALLERS, &documented);
}

#[test]
fn terminal_support_inventory_matches_contract() {
    assert_eq!(
        support::policy::test_support_callers(TERMINAL),
        Some(TERMINAL_CALLERS)
    );
    assert_eq!(TERMINAL_CALLERS, support::policy::TEST_TERMINAL_CALLERS);
    let documented = std::iter::once("maestro-cli")
        .chain(specification_names(
            "from\n`maestro-cli`",
            "; a caller\ndeclares it only when it uses it",
        ))
        .collect::<Vec<_>>();
    support_inventory_matches_contract(TERMINAL, TERMINAL_CALLERS, &documented);
}

#[test]
fn tracing_support_inventory_matches_contract() {
    assert_eq!(TRACING_CALLERS, support::policy::TEST_TRACING_CALLERS);
    let documented = std::iter::once("maestro-daemon")
        .chain(specification_names(
            "from\n`maestro-daemon`",
            "; a caller\ndeclares it only when it uses it",
        ))
        .collect::<Vec<_>>();
    support_inventory_matches_contract(TRACING, TRACING_CALLERS, &documented);
}

/// `owner` is dedicated, outside the utility permission, listed, and documented with `callers`.
fn support_inventory_matches_contract(owner: &str, callers: &[&str], documented: &[&str]) {
    assert_eq!(support::class(owner), "dedicated");
    assert!(utility_excluded().contains(&owner));
    assert!(!support::policy::uses_utility(owner));
    assert_eq!(documented, callers);
    assert!(documented_foundation_graph_matches_policy(
        FOUNDATION_SPECIFICATION
    ));
    let workspace = Workspace::new();
    workspace.foundation(&[owner]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    workspace.list(&[(owner, "core")]);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "invalid scoped layer for {owner}: expected dedicated"
        ))
    );
    workspace.list(&[]);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "workspace crate is not listed in workspace-crates.json: {owner}"
        ))
    );
}

#[test]
fn directory_support_preserves_process_and_domain_policies() {
    for caller in owner_callers() {
        let workspace = Workspace::new();
        workspace.foundation(&[caller, OWNER, DIRECTORY]);
        assert_eq!(
            caller_declares(
                &workspace,
                caller,
                &format!("[{NATIVE_DEV}]\n{}", owner_entry(""))
            ),
            Ok(())
        );
    }
    test_process_policy_keeps_existing_domain_edges();
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-models", "maestro-app", DIRECTORY]);
    assert_eq!(
        caller_declares(
            &workspace,
            "maestro-models",
            "[dependencies]\napp = { package = \"maestro-app\", path = \"../maestro-app\" }"
        ),
        Err("forbidden production dependency: maestro-models -> maestro-app".into())
    );
}

/// A function holding the explicit-instruction-path pattern, as its tracked path and its name.
type PatternOwner = (String, String);

/// Calls one function body makes, by the names the explicit-path pattern uses.
#[derive(Default)]
struct PatternCalls {
    /// The body expands a leading tilde.
    expands: bool,
    /// The body checks whether the expansion is absolute.
    checks_absolute: bool,
    /// The body resolves the result.
    resolves: bool,
}

impl PatternCalls {
    /// Read the calls `block` makes, ignoring nested functions.
    fn of(block: &syn::Block) -> Self {
        let mut calls = Self::default();
        syn::visit::visit_block(&mut calls, block);
        calls
    }

    /// Note a call by the last segment of its path.
    fn note(&mut self, path: &syn::ExprPath) {
        let Some(segment) = path.path.segments.last() else {
            return;
        };
        match segment.ident.to_string().as_str() {
            "expand_tilde_text" => self.expands = true,
            "is_absolute" => self.checks_absolute = true,
            "resolve" => self.resolves = true,
            _ => {}
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for PatternCalls {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            self.note(path);
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_item(&mut self, _node: &'ast syn::Item) {}

    fn visit_impl_item(&mut self, _node: &'ast syn::ImplItem) {}
}

/// Records every function of one source whose body holds the whole pattern.
struct PatternVisitor<'a> {
    /// Tracked path or control label used in the comparison.
    label: &'a str,
    /// Owners found so far.
    owners: &'a mut Vec<PatternOwner>,
}

impl PatternVisitor<'_> {
    /// Record `block` when it expands a tilde, checks absoluteness and resolves.
    fn record(&mut self, name: &syn::Ident, block: &syn::Block) {
        let calls = PatternCalls::of(block);
        if calls.expands && calls.checks_absolute && calls.resolves {
            self.owners.push((self.label.to_owned(), name.to_string()));
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for PatternVisitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.record(&node.sig.ident, &node.block);
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.record(&node.sig.ident, &node.block);
        syn::visit::visit_impl_item_fn(self, node);
    }
}

/// Every function of `source` that holds the explicit-path pattern, labeled with `label`.
fn pattern_owners(label: &str, source: &str) -> Vec<PatternOwner> {
    let Ok(file) = syn::parse_file(source) else {
        return Vec::new();
    };
    let mut owners = Vec::new();
    syn::visit::visit_file(
        &mut PatternVisitor {
            label,
            owners: &mut owners,
        },
        &file,
    );
    owners
}

/// The arguments a delegating loader forwards to the shared owner with.
const FORWARDED_ARGUMENTS: [&str; 3] = ["path", "options.cwd", "options.home"];

/// The spelling of one call argument, with its field accesses joined by `.`.
fn argument_spelling(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Path(path) => Some(
            path.path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("."),
        ),
        syn::Expr::Field(field) => {
            let member = match &field.member {
                syn::Member::Named(name) => name.to_string(),
                syn::Member::Unnamed(index) => index.index.to_string(),
            };
            Some(format!("{}.{member}", argument_spelling(&field.base)?))
        }
        _ => None,
    }
}

/// The path and text of every tracked Rust source below `root`.
///
/// Every tracked file is enumerated and the Rust sources are read, since only a parsed function
/// body can delegate.
fn tracked_rust_sources(root: &std::path::Path) -> Vec<(String, String)> {
    let listed = std::process::Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(listed.status.success());
    listed
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .filter_map(|path| std::str::from_utf8(path).ok())
        .filter(|path| {
            std::path::Path::new(path)
                .extension()
                .is_some_and(|kind| kind == "rs")
        })
        .map(|path| {
            (
                path.to_owned(),
                std::fs::read_to_string(root.join(path)).unwrap(),
            )
        })
        .collect()
}

/// Every function in the tracked `sources` that holds the explicit-path pattern.
///
/// A source that never names the expansion call cannot call it.
fn pattern_owners_in(sources: &[(String, String)]) -> Vec<PatternOwner> {
    let mut owners: Vec<PatternOwner> = sources
        .iter()
        .filter(|(_, source)| source.contains("expand_tilde_text"))
        .flat_map(|(path, source)| pattern_owners(path, source))
        .collect();
    owners.sort();
    owners
}

/// Whether `function` in `source` is exactly one tail expression that returns the shared owner's
/// result, called with the authored path, the caller's working directory and its home spelling.
fn delegates_directly(source: &str, function: &str) -> bool {
    let Ok(file) = syn::parse_file(source) else {
        return false;
    };
    let Some(body) = file.items.iter().find_map(|item| match item {
        syn::Item::Fn(item) if item.sig.ident == function => Some(&item.block),
        _ => None,
    }) else {
        return false;
    };
    let [syn::Stmt::Expr(syn::Expr::Call(call), None)] = body.stmts.as_slice() else {
        return false;
    };
    let syn::Expr::Path(target) = &*call.func else {
        return false;
    };
    target
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "resolve_explicit_path")
        && call
            .args
            .iter()
            .filter_map(argument_spelling)
            .eq(FORWARDED_ARGUMENTS)
}

/// Assert that a directly delegating loader is accepted and that a body returning the path,
/// discarding the shared result or forwarding its operands in the wrong order is rejected.
fn assert_forwarding_controls() {
    for (label, source, function) in [
        ("skills delegate", DELEGATE_BODY, "normalize_skill_path"),
        (
            "prompts delegate",
            PROMPT_DELEGATE_BODY,
            "resolve_prompt_path",
        ),
    ] {
        assert!(delegates_directly(source, function), "{label}");
    }
    for (label, source, function) in [
        ("skills bypass", SKILL_BYPASS_BODY, "normalize_skill_path"),
        ("prompts bypass", PROMPT_BYPASS_BODY, "resolve_prompt_path"),
        (
            "skills discarded result",
            SKILL_DISCARDED_RESULT_BODY,
            "normalize_skill_path",
        ),
        (
            "prompts discarded result",
            PROMPT_DISCARDED_RESULT_BODY,
            "resolve_prompt_path",
        ),
        (
            "swapped operands",
            SWAPPED_OPERANDS_BODY,
            "normalize_skill_path",
        ),
    ] {
        assert!(!delegates_directly(source, function), "{label}");
    }
}

/// Assert that each instruction loader's body is the direct delegation to the shared owner.
fn assert_tracked_delegation(sources: &[(String, String)]) {
    for (path, function) in [
        (
            "crates/maestro-resources/src/prompt_templates.rs",
            "resolve_prompt_path",
        ),
        (
            "crates/maestro-resources/src/skills/mod.rs",
            "normalize_skill_path",
        ),
    ] {
        let source = sources
            .iter()
            .find(|(tracked, _)| tracked == path)
            .map(|(_, source)| source.as_str());
        assert!(
            source.is_some_and(|source| delegates_directly(source, function)),
            "{path}: {function}"
        );
    }
}

/// A shared owner holding the expansion, the absoluteness check and the resolution.
const OWNER_BODY: &str = "\
fn resolve_explicit_path(path: &str, cwd: &str, home: &str) -> std::io::Result<String> {
    let path = path.trim_matches(whitespace);
    let normalized = expand_tilde_text(path, || Ok(home.to_owned()))?;
    if is_absolute(&normalized) {
        Ok(normalized)
    } else {
        resolve(&[cwd, &normalized])
    }
}
";

/// A caller that delegates to the shared owner.
const DELEGATE_BODY: &str = "\
fn normalize_skill_path(path: &str, options: &Options<'_>) -> std::io::Result<String> {
    resolve_explicit_path(path, options.cwd, options.home)
}
";

/// The skill loader's own copy of the expansion, absoluteness check and resolution.
const SKILL_BODY: &str = "\
fn normalize_skill_path(path: &str, options: &Options<'_>) -> std::io::Result<String> {
    let path = path.trim_matches(whitespace);
    if path.starts_with('~') {
        return expand_tilde_text(path, || Ok(options.home.to_owned()));
    }
    if is_absolute(path) {
        return Ok(path.to_owned());
    }
    resolve(&[options.cwd, path])
}
";

/// The prompt loader's own copy of the expansion, absoluteness check and resolution.
const PROMPT_BODY: &str = "\
fn resolve_prompt_path(path: &str, options: Options<'_>) -> std::io::Result<String> {
    let path = path.trim_matches(whitespace);
    let normalized = expand_tilde_text(path, || Ok(options.home.to_owned()))?;
    if is_absolute(&normalized) {
        Ok(normalized)
    } else {
        resolve(&[options.cwd, &normalized])
    }
}
";

/// A prompt loader that delegates to the shared owner.
const PROMPT_DELEGATE_BODY: &str = "\
fn resolve_prompt_path(path: &str, options: Options<'_>) -> std::io::Result<String> {
    resolve_explicit_path(path, options.cwd, options.home)
}
";

/// A prompt loader that returns the path unchanged instead of delegating.
const PROMPT_BYPASS_BODY: &str = "\
fn resolve_prompt_path(path: &str, options: Options<'_>) -> std::io::Result<String> {
    Ok(path.to_owned())
}
";

/// A skill loader that returns the path unchanged instead of delegating.
const SKILL_BYPASS_BODY: &str = "\
fn normalize_skill_path(path: &str, options: &Options<'_>) -> std::io::Result<String> {
    Ok(path.to_owned())
}
";

/// A skill loader that calls the shared owner and discards its result.
const SKILL_DISCARDED_RESULT_BODY: &str = "\
fn normalize_skill_path(path: &str, options: &Options<'_>) -> std::io::Result<String> {
    let _ = resolve_explicit_path(path, options.cwd, options.home);
    Ok(path.to_owned())
}
";

/// A prompt loader that calls the shared owner and discards its result.
const PROMPT_DISCARDED_RESULT_BODY: &str = "\
fn resolve_prompt_path(path: &str, options: Options<'_>) -> std::io::Result<String> {
    let _ = resolve_explicit_path(path, options.cwd, options.home);
    Ok(path.to_owned())
}
";

/// A loader that forwards the working directory and the home spelling in the wrong order.
const SWAPPED_OPERANDS_BODY: &str = "\
fn normalize_skill_path(path: &str, options: &Options<'_>) -> std::io::Result<String> {
    resolve_explicit_path(path, options.home, options.cwd)
}
";

/// A sibling policy that expands in one function and always resolves in another.
const RESOURCE_BODY: &str = "\
fn expand_resource_path(path: &str, home: Home) -> std::io::Result<String> {
    expand_tilde_text(path.trim_matches(whitespace), home)
}
fn resolve_resource_path(cwd: &str, path: &str, home: Home) -> std::io::Result<String> {
    let expanded = expand_resource_path(path, home)?;
    resolve(&[cwd, &expanded])
}
";

/// The shared owner followed by a second copy beside it.
const OWNER_AND_COPY: &str = "\
fn resolve_explicit_path(path: &str, cwd: &str, home: &str) -> std::io::Result<String> {
    let path = path.trim_matches(whitespace);
    let normalized = expand_tilde_text(path, || Ok(home.to_owned()))?;
    if is_absolute(&normalized) {
        Ok(normalized)
    } else {
        resolve(&[cwd, &normalized])
    }
}
fn resolve_explicit_path_copy(path: &str, cwd: &str, home: &str) -> std::io::Result<String> {
    let path = path.trim_matches(whitespace);
    let normalized = expand_tilde_text(path, || Ok(home.to_owned()))?;
    if is_absolute(&normalized) {
        Ok(normalized)
    } else {
        resolve(&[cwd, &normalized])
    }
}
";

/// Expanding a tilde, checking the result is absolute and resolving it has one implementation,
/// which both instruction loaders call; an inline copy in a caller or a second copy beside the
/// owner is rejected.
#[test]
fn explicit_instruction_paths_share_their_resolver() {
    let controls: [(&str, &str, &[&str]); 6] = [
        ("owner", OWNER_BODY, &["resolve_explicit_path"]),
        ("delegate", DELEGATE_BODY, &[]),
        ("skills", SKILL_BODY, &["normalize_skill_path"]),
        ("prompts", PROMPT_BODY, &["resolve_prompt_path"]),
        ("resource policy", RESOURCE_BODY, &[]),
        (
            "owner twice",
            OWNER_AND_COPY,
            &["resolve_explicit_path", "resolve_explicit_path_copy"],
        ),
    ];
    for (label, source, expected) in controls {
        let names: Vec<_> = pattern_owners(label, source)
            .into_iter()
            .map(|(_, name)| name)
            .collect();
        assert_eq!(names, expected, "{label}");
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sources = tracked_rust_sources(&root);
    assert_eq!(
        pattern_owners_in(&sources),
        [(
            "crates/maestro-resources/src/paths.rs".to_owned(),
            "resolve_explicit_path".to_owned()
        )]
    );
    assert_forwarding_controls();
    assert_tracked_delegation(&sources);
}

#[test]
fn print_recorder_listener_edge_stays_native_and_development_only() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-cli", "maestro-listeners"]);
    let production = "[dependencies]\nmaestro-app = { path = \"../maestro-app\" }\n";
    let native = "target.'cfg(not(target_arch = \"wasm32\"))'";
    let declaration = "maestro-listeners = { path = \"../maestro-listeners\" }";
    workspace.member(
        "maestro-cli",
        "maestro-cli",
        &format!("{production}[{native}.dev-dependencies]\n{declaration}"),
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    for table in [
        "target.'cfg(not(target_arch = \"wasm32\"))'.dependencies",
        "target.'cfg(not(target_arch = \"wasm32\"))'.build-dependencies",
        "dev-dependencies",
        "target.'cfg(target_arch = \"wasm32\")'.dev-dependencies",
    ] {
        workspace.member(
            "maestro-cli",
            "maestro-cli",
            &format!("{production}[{table}]\n{declaration}"),
        );
        let expected = if table.ends_with("dev-dependencies") {
            "maestro-cli: maestro-listeners test support must be a native, non-optional development dependency"
        } else {
            "forbidden production dependency: maestro-cli -> maestro-listeners"
        };
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(expected.into()),
            "{table}"
        );
    }
    workspace.member("maestro-cli", "maestro-cli", &format!("{production}[{native}.dev-dependencies]\nmaestro-listeners = {{ path = \"../maestro-listeners\", optional = true }}"));
    assert!(
        check_workspace(workspace.root.path()).is_err(),
        "optional development dependencies are invalid Cargo declarations"
    );
}

/// The target table the interactive frontend declares its native witness in.
const WITNESS_DEV: &str = "target.'cfg(not(target_arch = \"wasm32\"))'.dev-dependencies";

/// The interactive frontend's manifest, keeping its required edges beside the witness table.
fn witness_chat(table: &str, extra: &str) -> String {
    let mut manifest = String::from("[dependencies]\n");
    for target in required_targets("maestro-chat") {
        writeln!(
            manifest,
            "{} = {{ package = {target:?}, path = \"../{target}\" }}",
            target.replace('-', "_")
        )
        .unwrap();
    }
    writeln!(
        manifest,
        "[{table}]\nterminal = {{ package = \"maestro-tui-crossterm\", path = \"../maestro-tui-crossterm\"{extra} }}"
    )
    .unwrap();
    manifest
}

#[test]
fn interactive_native_witness_edge_is_native_development_only() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat", "maestro-tui-crossterm"]);
    workspace.member(
        "maestro-chat",
        "maestro-chat",
        &witness_chat(WITNESS_DEV, ""),
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    for table in [
        "dev-dependencies",
        "target.'cfg(target_arch = \"wasm32\")'.dev-dependencies",
    ] {
        workspace.member("maestro-chat", "maestro-chat", &witness_chat(table, ""));
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("maestro-chat: maestro-tui-crossterm test support must be a native, non-optional development dependency".to_owned()),
            "{table}"
        );
    }
    let optional = witness_chat("dev-dependencies", ", optional = true");
    workspace.member("maestro-chat", "maestro-chat", &optional);
    assert!(
        check_workspace(workspace.root.path())
            .is_err_and(|error| error.contains("dev-dependencies are not allowed to be optional")),
        "Cargo refuses an optional development declaration"
    );
}

/// The guest's checker witness has one native development edge, never a product edge.
#[test]
fn guest_schema_validation_edge_is_native_development_only() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm", "maestro-models"]);
    let declaration = "models = { package = \"maestro-models\", path = \"../maestro-models\" }";
    assert_eq!(
        caller_declares(
            &workspace,
            "maestro-extensions-wasm",
            &format!("[{NATIVE_DEV}]\n{declaration}")
        ),
        Ok(())
    );
    for table in [
        "dependencies",
        "build-dependencies",
        "dev-dependencies",
        "target.'cfg(target_arch = \"wasm32\")'.dev-dependencies",
    ] {
        assert!(
            caller_declares(
                &workspace,
                "maestro-extensions-wasm",
                &format!("[{table}]\n{declaration}")
            )
            .is_err(),
            "{table}"
        );
    }
    assert!(caller_declares(&workspace, "maestro-extensions-wasm", &format!("[{NATIVE_DEV}]\nmodels = {{ package = \"maestro-models\", path = \"../maestro-models\", optional = true }}")).is_err());
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-request", "maestro-models"]);
    assert_eq!(
        caller_declares(&workspace, "maestro-request", &format!("[{NATIVE_DEV}]\n{declaration}")),
        Err("internal dev dependency requires declared dependency-free test support: maestro-request -> maestro-models".into())
    );
}

#[test]
fn native_process_and_guest_share_text_without_giving_text_dependencies() {
    for owner in ["maestro-process", "maestro-extensions-wasm"] {
        let workspace = Workspace::new();
        workspace.foundation(&[owner, "maestro-text"]);
        workspace.member(
            owner,
            owner,
            r#"[dependencies]
maestro-text = { path = "../maestro-text" }
"#,
        );
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
    let text = support::policy::POLICY
        .iter()
        .find(|(name, _)| *name == "maestro-text")
        .unwrap();
    assert!(text.1.is_empty());
}
