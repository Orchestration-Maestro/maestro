#![cfg(test)]

use crate::support::{self, Workspace};
use maestro_test_conventions::check_workspace;
use std::fmt::Write;

/// Installs a reusable package with its single component export.
fn package(workspace: &Workspace, name: &str, dependencies: &str) {
    workspace.member(name, name, &format!(
        "[lib]\ncrate-type = [\"rlib\"]\n[[example]]\nname = \"component\"\npath = \"src/component.rs\"\ncrate-type = [\"cdylib\"]\n{dependencies}"
    ));
    support::source(
        workspace,
        name,
        "src/lib.rs",
        "/// Registers the package.\npub fn factory() {}\n",
    );
    support::source(
        workspace,
        name,
        "src/component.rs",
        "maestro_extensions_wasm::export_extension!(factory);\n",
    );
}

#[test]
fn package_crates_accept_reusable_libraries_and_acyclic_dependencies() {
    let workspace = Workspace::new();
    package(
        &workspace,
        "maestro-subagents",
        "[dependencies]\nmaestro-questions = { path = \"../maestro-questions\" }\n",
    );
    package(&workspace, "maestro-questions", "");
    workspace.list(&[
        ("maestro-subagents", "package"),
        ("maestro-questions", "package"),
    ]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn package_edges_reject_native_owners_and_reverse_dependencies() {
    for kind in [
        "dependencies",
        "build-dependencies",
        "dev-dependencies",
        "target.'cfg(target_os = \"none\")'.dependencies",
    ] {
        let workspace = Workspace::new();
        package(&workspace, "maestro-subagents", "");
        workspace.member("process", "maestro-process", "");
        workspace.list(&[
            ("maestro-subagents", "package"),
            ("maestro-process", "core"),
        ]);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
        package(
            &workspace,
            "maestro-subagents",
            &format!("[{kind}]\nmaestro-process = {{ path = \"../process\" }}\n"),
        );
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err("forbidden package dependency: maestro-subagents -> maestro-process".into())
        );
        package(&workspace, "maestro-subagents", "");
        for (name, class) in [
            ("maestro-process", "core"),
            ("maestro-tooling", "dedicated"),
        ] {
            workspace.member(
                "process",
                name,
                &format!("[{kind}]\nmaestro-subagents = {{ path = \"../maestro-subagents\" }}\n"),
            );
            workspace.list(&[("maestro-subagents", "package"), (name, class)]);
            assert_eq!(
                check_workspace(workspace.root.path()),
                Err(format!(
                    "non-package crate {name} must not depend on package crate maestro-subagents"
                ))
            );
        }
    }
}

#[test]
fn package_dependencies_reuse_only_the_guest_record_owners() {
    let workspace = Workspace::new();
    package(&workspace, "maestro-subagents", "");
    workspace.list(&[("maestro-subagents", "package")]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    let mut entries = vec![("maestro-subagents", "package")];
    let mut dependencies = String::from("[dependencies]\n");
    for name in [
        "maestro-extensions-wasm",
        "maestro-request",
        "maestro-entry",
        "maestro-tool",
        "maestro-json",
    ] {
        workspace.member(name, name, "");
        entries.push((name, "core"));
        writeln!(dependencies, "{name} = {{ path = \"../{name}\" }}").unwrap();
    }
    workspace.list(&entries);
    package(&workspace, "maestro-subagents", &dependencies);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn package_dependencies_preserve_production_and_test_cycle_checks() {
    for kind in ["dependencies", "dev-dependencies"] {
        let workspace = Workspace::new();
        package(
            &workspace,
            "maestro-subagents",
            "[dependencies]\nmaestro-questions = { path = \"../maestro-questions\" }\n",
        );
        package(&workspace, "maestro-questions", "");
        workspace.list(&[
            ("maestro-subagents", "package"),
            ("maestro-questions", "package"),
        ]);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
        package(
            &workspace,
            "maestro-questions",
            &format!("[{kind}]\nmaestro-subagents = {{ path = \"../maestro-subagents\" }}\n"),
        );
        let error = check_workspace(workspace.root.path()).unwrap_err();
        assert!(error.contains("dependency cycle"), "{error}");
    }
}

#[test]
fn package_components_require_separate_library_and_component_targets() {
    let workspace = Workspace::new();
    package(&workspace, "maestro-subagents", "");
    workspace.list(&[("maestro-subagents", "package")]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    let manifest = workspace
        .root
        .path()
        .join("crates/maestro-subagents/Cargo.toml");
    let original = std::fs::read_to_string(&manifest).unwrap();
    for changed in [
        original.replace("[\"rlib\"]", "[\"rlib\", \"cdylib\"]"),
        original.replace("src/component.rs", "src/other.rs"),
    ] {
        support::source(&workspace, "maestro-subagents", "src/other.rs", "");
        std::fs::write(&manifest, changed).unwrap();
        assert!(
            check_workspace(workspace.root.path())
                .unwrap_err()
                .contains("package needs an rlib")
        );
    }
    std::fs::write(&manifest, original).unwrap();
}

#[test]
fn package_components_keep_the_single_export_out_of_the_library() {
    let workspace = Workspace::new();
    package(&workspace, "maestro-subagents", "");
    workspace.list(&[("maestro-subagents", "package")]);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    for path in ["src/lib.rs", "src/helper.rs", "examples/other.rs"] {
        let source = support::source(
            &workspace,
            "maestro-subagents",
            path,
            "maestro_extensions_wasm::export_extension!(factory);\n",
        );
        assert!(
            check_workspace(workspace.root.path())
                .unwrap_err()
                .contains("export_extension belongs only")
        );
        std::fs::remove_file(source).unwrap();
        support::source(
            &workspace,
            "maestro-subagents",
            "src/lib.rs",
            "pub fn factory() {}\n",
        );
    }
    for text in [
        "",
        "export_extension!(factory); export_extension!(factory);",
    ] {
        support::source(&workspace, "maestro-subagents", "src/component.rs", text);
        assert!(
            check_workspace(workspace.root.path())
                .unwrap_err()
                .contains("exactly one export_extension")
        );
    }
    support::source(
        &workspace,
        "maestro-subagents",
        "src/component.rs",
        "// export_extension!(fake);\nconst TEXT: &str = \"export_extension!(fake)\";\nmaestro_extensions_wasm::export_extension!(factory);\n",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    support::source(
        &workspace,
        "maestro-subagents",
        "src/lib.rs",
        "fn factory() {}\n",
    );
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("public factory")
    );
}

#[test]
fn package_export_aliases_cannot_create_a_second_export_set() {
    let workspace = Workspace::new();
    package(&workspace, "maestro-subagents", "");
    workspace.list(&[("maestro-subagents", "package")]);
    for (import, call) in [
        (
            "use maestro_extensions_wasm::export_extension as guest;",
            "guest",
        ),
        (
            "use maestro_extensions_wasm::{export_extension as guest};",
            "guest",
        ),
        (
            "use maestro_extensions_wasm::r#export_extension as guest;",
            "guest",
        ),
        (
            "use maestro_extensions_wasm::{export_extension as r#guest};",
            "guest",
        ),
        ("", "maestro_extensions_wasm::r#export_extension"),
        (
            "use maestro_extensions_wasm::export_extension as first; use first as guest;",
            "guest",
        ),
    ] {
        support::source(
            &workspace,
            "maestro-subagents",
            "src/lib.rs",
            &format!("pub fn factory() {{}}\n{import}\n{call}!(factory);\n"),
        );
        assert!(
            check_workspace(workspace.root.path())
                .unwrap_err()
                .contains("export_extension belongs only")
        );
    }
}

#[test]
fn package_factories_may_be_explicit_public_reexports() {
    let workspace = Workspace::new();
    package(&workspace, "maestro-subagents", "");
    workspace.list(&[("maestro-subagents", "package")]);
    support::source(
        &workspace,
        "maestro-subagents",
        "src/registration.rs",
        "pub fn register() {}\n",
    );
    support::source(
        &workspace,
        "maestro-subagents",
        "src/lib.rs",
        "mod registration;\npub use registration::register as factory;\n",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn package_factories_follow_the_actual_library_target() {
    let workspace = Workspace::new();
    package(&workspace, "maestro-subagents", "");
    workspace.list(&[("maestro-subagents", "package")]);
    let manifest = workspace
        .root
        .path()
        .join("crates/maestro-subagents/Cargo.toml");
    let contents = std::fs::read_to_string(&manifest)
        .unwrap()
        .replace("[lib]", "[lib]\npath = \"src/api.rs\"");
    std::fs::write(manifest, contents).unwrap();
    support::source(&workspace, "maestro-subagents", "src/api.rs", "");
    assert!(
        check_workspace(workspace.root.path())
            .unwrap_err()
            .contains("public factory")
    );
    support::source(
        &workspace,
        "maestro-subagents",
        "src/api.rs",
        "pub fn factory() {}\n",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn package_factory_names_follow_raw_identifier_semantics() {
    let workspace = Workspace::new();
    package(&workspace, "maestro-subagents", "");
    workspace.list(&[("maestro-subagents", "package")]);
    support::source(
        &workspace,
        "maestro-subagents",
        "src/registration.rs",
        "pub fn register() {}
",
    );
    for source in [
        "pub fn r#factory() {}
",
        "mod registration;
pub use registration::register as r#factory;
",
    ] {
        support::source(&workspace, "maestro-subagents", "src/lib.rs", source);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}
