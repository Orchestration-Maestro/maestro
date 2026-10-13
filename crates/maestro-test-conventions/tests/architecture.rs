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
        ("maestro-tooling", "dedicated"),
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
    for &(name, _) in support::policy::POLICY {
        let workspace = Workspace::new();
        workspace.foundation(&[name]);
        let path = workspace.root.join("workspace-crates.json");
        let mut list: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        list.as_object_mut().unwrap().shift_remove(name);
        std::fs::write(&path, serde_json::to_vec(&list).unwrap()).unwrap();
        assert_eq!(
            check_workspace(&workspace.root),
            Err(format!(
                "workspace crate is not listed in workspace-crates.json: {name}"
            ))
        );
        workspace.foundation(&[name]);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn crate_list_requires_core_or_dedicated_layers() {
    all_crates_require_their_declared_layer();
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
                check_workspace(&workspace.root),
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
            workspace.member("leaf", leaf, "");
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        }
    }
}

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
    workspace.list(&[("maestro-session", "core"), ("maestro-agent", "core")]);
    workspace.member("agent", "maestro-agent", "");
    let manifest = workspace.root.join("Cargo.toml");
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
            assert_eq!(check_workspace(&workspace.root), Err("dependency maestro-session -> maestro-agent must use a path to that workspace member".into()));
        }
        workspace.member("session", "maestro-session", &format!("[{kind}]\nrenamed = {{ package = \"maestro-agent\", path = \"../agent/../agent\" }}"));
        let expected = if kind == "dev-dependencies" {
            Err("internal dev dependency requires declared dependency-free test support: maestro-session -> maestro-agent".into())
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
    leaf_internal_dependencies_are_rejected();
    core_to_dedicated_dependency_is_rejected();
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
        ("maestro-extensions-wasmtime", "maestro-models"),
        ("maestro", "maestro-agent"),
        ("maestro-agent", "maestro-app"),
        ("maestro-app", "maestro-pattern"),
        ("maestro-process", "maestro-tools"),
        ("maestro-process", "maestro-packages"),
        ("maestro-process", "maestro-tui"),
        ("maestro-models", "maestro-process"),
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
                check_workspace(&workspace.root),
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
        assert_eq!(check_workspace(&corrected.root), Ok(()));
    }
}

#[test]
fn export_reuses_text_and_json_without_frontend_edge() {
    for owner in ["maestro-text", "maestro-json"] {
        let workspace = Workspace::new();
        workspace.foundation(&["maestro-export", owner]);
        check_permitted_declarations(&workspace, "maestro-export", owner, &[owner]);
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
            check_workspace(&workspace.root),
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
        check_permitted_declarations(&workspace, from, "maestro-pattern", &["maestro-pattern"]);
    }
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-catalog", "maestro-pattern"]);
    workspace.member(
        "maestro-catalog",
        "maestro-catalog",
        "[dev-dependencies]\nalias = { package = \"maestro-pattern\", path = \"../maestro-pattern\" }",
    );
    assert_eq!(
        check_workspace(&workspace.root),
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
            check_workspace(&workspace.root),
            Err("forbidden production dependency: maestro-pattern -> maestro-catalog".into())
        );
    }
}

#[test]
fn process_capture_consumers_may_declare_the_owner() {
    for from in [
        "maestro-tools",
        "maestro-packages",
        "maestro-tui",
        "maestro-credentials",
    ] {
        let workspace = Workspace::new();
        workspace.foundation(&[from, "maestro-process"]);
        check_permitted_declarations(&workspace, from, "maestro-process", &["maestro-process"]);
        workspace.member(
            from,
            from,
            "[target.'cfg(not(target_arch = \"wasm32\"))'.dependencies]\nnative = { package = \"maestro-process\", path = \"../maestro-process\" }",
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()), "{from}");
        workspace.member(
            from,
            from,
            "[dev-dependencies]\nalias = { package = \"maestro-process\", path = \"../maestro-process\" }",
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Err(format!(
                "internal dev dependency requires declared dependency-free test support: {from} -> maestro-process"
            ))
        );
    }
}

#[test]
fn process_leaf_cannot_depend_on_cancellation() {
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
            check_workspace(&workspace.root),
            Err("maestro-process must not depend on workspace crate maestro-cancellation".into()),
            "{kind}{extra}"
        );
    }
    workspace.member("maestro-process", "maestro-process", "");
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn session_shared_records_edge_is_scoped() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-session", "maestro-request"]);
    check_permitted_declarations(
        &workspace,
        "maestro-session",
        "maestro-request",
        &["maestro-request"],
    );
    workspace.member(
        "maestro-session",
        "maestro-session",
        "[dev-dependencies]\nalias = { package = \"maestro-request\", path = \"../maestro-request\" }",
    );
    assert_eq!(
        check_workspace(&workspace.root),
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
            check_workspace(&workspace.root),
            Err("maestro-request must not depend on workspace crate maestro-session".into())
        );
    }
}

#[test]
fn theme_shared_records_edge_is_scoped() {
    let allowed = Workspace::new();
    allowed.foundation(&["maestro-theme", "maestro-request"]);
    check_permitted_declarations(
        &allowed,
        "maestro-theme",
        "maestro-request",
        &["maestro-request"],
    );
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
            let error = check_workspace(&workspace.root).unwrap_err();
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
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(error.contains(from) && error.contains(to), "{error}");
    }
}

#[test]
fn permitted_downward_edges_pass_without_absent_crates() {
    documented_foundation_graph_matches_policy();
    assert_eq!(support::policy::POLICY.len(), 37);
    assert_eq!(
        support::policy::POLICY
            .iter()
            .map(|row| row.1.len())
            .sum::<usize>(),
        113
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

const DECLARATIONS: &[(&str, &str)] = &[
    ("dependencies", ""),
    ("build-dependencies", ""),
    ("dependencies", ", optional = true"),
    ("target.'cfg(target_os = \"none\")'.dependencies", ""),
    ("target.'cfg(target_os = \"none\")'.build-dependencies", ""),
];

/// Crates that may not declare the foundation utility.
const UTILITY_EXCLUDED: &[&str] = &[
    "maestro-watch",
    "maestro-lock",
    "maestro-process",
    "maestro-pattern",
    "maestro-cancellation",
    "maestro-request",
    "maestro-extensions-wasm",
    "maestro-extensions-wasmtime",
    "maestro-test-terminal",
    "maestro-text",
    "maestro-json",
    "maestro-entry",
    "maestro-tool",
    "maestro-brand",
];

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
    let consumers: Vec<_> = rows
        .iter()
        .map(|row| row.0)
        .filter(|name| *name != "maestro-path" && !UTILITY_EXCLUDED.contains(name))
        .collect();
    assert_eq!(rows.len(), 37);
    assert_eq!(consumers.len(), 22);
    assert_eq!(rows.iter().filter(|row| row.1.is_empty()).count(), 10);
    for from in consumers {
        native_consumer_may_declare_the_utility(from);
    }
    for from in UTILITY_EXCLUDED {
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

/// Accept every production declaration form of the utility and reject a dev one.
fn native_consumer_may_declare_the_utility(from: &str) {
    let workspace = Workspace::new();
    workspace.foundation(&[from, "maestro-path"]);
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            from,
            from,
            &dependencies_with_utility(required_targets(from), kind, extra),
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Ok(()),
            "{from} -> maestro-path ({kind}{extra})"
        );
    }
    workspace.member(
        from,
        from,
        &dependencies_with_utility(required_targets(from), "dev-dependencies", ""),
    );
    assert_eq!(
        check_workspace(&workspace.root),
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
            check_workspace(&workspace.root),
            Err(if is_leaf(from) {
                format!("{from} must not depend on workspace crate maestro-path")
            } else {
                format!("forbidden production dependency: {from} -> maestro-path")
            })
        );
    }
    workspace.foundation(&[from, "maestro-path"]);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
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
            check_workspace(&workspace.root),
            Err("maestro-path must not depend on workspace crate maestro-models".into())
        );
    }
}

/// Verify that the optional utility edge cannot replace the required application edge.
fn path_utility_does_not_replace_the_application() {
    for frontend in ["maestro-cli", "maestro-chat", "maestro-rpc", "maestro-web"] {
        let required = required_targets(frontend);
        let workspace = Workspace::new();
        workspace.foundation(&[&[frontend, "maestro-path"], required].concat());
        workspace.member(
            frontend,
            frontend,
            &dependencies_with_utility(required, "dependencies", ""),
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
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
                check_workspace(&workspace.root),
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
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    for absent in ["text", "json", "entry", "tool", "brand"] {
        assert!(
            !workspace
                .root
                .join(format!("crates/maestro-{absent}"))
                .exists()
        );
    }
    assert!(!workspace.root.join("crates/maestro-process").exists());
    assert!(!workspace.root.join("crates/maestro-tools").exists());
    workspace.member("tools", "maestro-tools", "");
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    assert!(!workspace.root.join("crates/maestro-text").exists());
    assert!(!workspace.root.join("crates/maestro-chat").exists());
    assert!(!workspace.root.join("crates/maestro-watch").exists());
    assert!(!workspace.root.join("crates/maestro-lock").exists());
    assert!(!workspace.root.join("crates/maestro-pattern").exists());
    assert!(!workspace.root.join("wit").exists());
}

#[test]
fn frontends_require_application_without_unused_allowed_edges() {
    for frontend in ["maestro-cli", "maestro-chat", "maestro-rpc", "maestro-web"] {
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
            assert_eq!(check_workspace(&workspace.root), Ok(()));
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
fn terminal_adapter_and_harness_require_toolkit() {
    for owner in ["maestro-tui-crossterm", "maestro-test-terminal"] {
        let text = if owner == "maestro-tui-crossterm" {
            "maestro-text = { path = \"../maestro-text\" }\n"
        } else {
            ""
        };
        let workspace = Workspace::new();
        workspace.foundation(&[owner, "maestro-models", "maestro-text"]);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
        workspace.member(owner, owner, "");
        assert_eq!(
            check_workspace(&workspace.root),
            Err(format!(
                "{owner}: missing required direct dependency maestro-tui"
            ))
        );
        for (kind, extra) in DECLARATIONS {
            workspace.member(owner, owner, &format!("[{kind}]\nmaestro-tui = {{ path = \"../maestro-tui\" }}\n{text}models = {{ package = \"maestro-models\", path = \"../maestro-models\"{extra} }}"));
            assert_eq!(
                check_workspace(&workspace.root),
                Err(format!(
                    "forbidden production dependency: {owner} -> maestro-models"
                ))
            );
            workspace.member(
                owner,
                owner,
                &format!("[{kind}]\nmaestro-tui = {{ path = \"../maestro-tui\"{extra} }}\n{text}"),
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
            check_workspace(&workspace.root)
                .unwrap_err()
                .contains("internal dev dependency")
        );
        workspace.member("maestro-agent", "maestro-agent", "");
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[cfg(unix)]
#[test]
fn metadata_fixtures_run_while_scenario_data_is_open() {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-models"]);
    std::fs::write(
        workspace.root.join("resolved.json"),
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
fn metadata_graph_failures_cross_the_public_interface() {
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

fn documented_foundation_graph_matches_policy() {
    use std::collections::{BTreeMap, BTreeSet};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let specification = std::fs::read_to_string(root.join("docs/specs/maestro-port.md")).unwrap();
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
    assert_eq!(documented, expected);
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
        check_workspace(&workspace.root),
        Err("test dependency cycle: maestro-agent -> maestro-models -> maestro-agent".into())
    );
    workspace.member("maestro-models", "maestro-models", "");
    assert_eq!(check_workspace(&workspace.root), Ok(()));
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
            check_workspace(&workspace.root),
            Err("forbidden production dependency: maestro-agent -> maestro-session".into())
        );
        workspace.member("maestro-agent", "maestro-agent", "");
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

fn forbidden_policy_matrix() {
    #[cfg(unix)]
    policy_edge_matrix(false);
}

#[cfg(unix)]
fn policy_edge_matrix(dev: bool) {
    use serde_json::json;
    let workspace = Workspace::new();
    let names: Vec<_> = support::policy::POLICY.iter().map(|row| row.0).collect();
    workspace.foundation(&names);
    let valid = workspace.metadata();
    let template = valid["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "maestro-cli")
        .unwrap()["dependencies"][0]
        .clone();
    for &(from, targets) in support::policy::POLICY {
        for &(to, _) in support::policy::POLICY {
            let utility_edge = to == "maestro-path" && !UTILITY_EXCLUDED.contains(&from);
            if from == to || (!dev && (targets.contains(&to) || utility_edge)) {
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
            dependency["path"] = json!(workspace.root.join("crates").join(to));
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
            workspace.metadata_probe(&declared, &valid, &expected);
            workspace.metadata_probe(&valid, &valid, "ok");
        }
    }
    workspace.metadata_probe(&valid, &valid, "ok");
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
            log = workspace.root.join("order")
        ),
    );
    workspace.probe(&cargo, &rustc, "ok");
    let log = std::fs::read_to_string(workspace.root.join("order")).unwrap();
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
        &workspace.root.join("missing-rustc"),
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
        check_workspace(&workspace.root),
        Err(format!(
            "{ADAPTER}: missing required direct dependency maestro-text"
        ))
    );
    workspace.member(
        ADAPTER,
        ADAPTER,
        &format!("[dependencies]\n{toolkit}{plain_text}"),
    );
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    for (kind, extra) in DECLARATIONS {
        workspace.member(
            ADAPTER,
            ADAPTER,
            &format!("[{kind}]\n{toolkit}renamed = {{ package = \"maestro-text\", path = \"../maestro-text\"{extra} }}"),
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()), "{kind}{extra}");
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
            check_workspace(&sparse.root),
            Err(missing.clone()),
            "{only}"
        );
        workspace.member(ADAPTER, ADAPTER, only);
        assert_eq!(
            check_workspace(&workspace.root),
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
        check_workspace(&workspace.root),
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
        check_workspace(&harness.root),
        Err("forbidden production dependency: maestro-test-terminal -> maestro-text".into())
    );
}

#[test]
fn complete_permitted_graph_is_acyclic() {
    let names: Vec<_> = support::policy::POLICY.iter().map(|row| row.0).collect();
    let workspace = Workspace::new();
    workspace.foundation(&names);
    for (kind, extra) in DECLARATIONS {
        for &(from, targets) in support::policy::POLICY {
            let mut declarations = format!("[{kind}]\n");
            let utility = (from != "maestro-path" && !UTILITY_EXCLUDED.contains(&from))
                .then_some("maestro-path");
            for (index, target) in targets.iter().copied().chain(utility).enumerate() {
                writeln!(
                    declarations,
                    "edge{index} = {{ package = {target:?}, path = \"../{target}\"{extra} }}"
                )
                .unwrap();
            }
            workspace.member(from, from, &declarations);
        }
        assert_eq!(check_workspace(&workspace.root), Ok(()), "{kind}{extra}");
    }
}
