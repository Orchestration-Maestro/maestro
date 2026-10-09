#![cfg(test)]

mod support;

use maestro_test_conventions::check_workspace;
use support::Workspace;

/// The canonical workspace declaration used by corrected fixtures.
const JSON: &str = "{ version = \"=1.0.151\", features = [\"float_roundtrip\"] }";

/// Build a workspace with a local package supplying the declared JSON features.
fn fixture() -> Workspace {
    let workspace = Workspace::new();
    workspace.external("serde_json");
    let external = workspace.root.join("external/Cargo.toml");
    let text = std::fs::read_to_string(&external).unwrap();
    std::fs::write(
        external,
        format!(
            "{}\n[features]\nfloat_roundtrip = []\nraw_value = []\nfloat_roundtrip_extra = []\n",
            text.replace("9.0.0", "1.0.151")
        ),
    )
    .unwrap();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    workspace
}

/// Replace the canonical workspace entry without changing unrelated declarations.
fn root_json(workspace: &Workspace, declaration: &str) {
    let path = workspace.root.join("Cargo.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut document: toml::Table = text.parse().unwrap();
    let dependencies = document["workspace"]["dependencies"]
        .as_table_mut()
        .unwrap();
    dependencies.remove("serde_json");
    if !declaration.is_empty() {
        let entry: toml::Table = format!("serde_json = {declaration}").parse().unwrap();
        dependencies.extend(entry);
    }
    std::fs::write(path, document.to_string()).unwrap();
}

/// Render the required root diagnostic with the fixture manifest path.
fn root_error(workspace: &Workspace) -> String {
    format!(
        "{}: workspace.dependencies.serde_json must declare serde_json with float_roundtrip on the normal dependency path",
        workspace.root.join("Cargo.toml").display()
    )
}

#[test]
fn workspace_json_requires_normal_float_roundtrip() {
    for declaration in [
        "",
        "\"=1.0.151\"",
        "{ version = \"=1.0.151\" }",
        "{ version = \"=1.0.151\", features = [] }",
        "{ package = \"another\", version = \"1\", features = [\"float_roundtrip\"] }",
    ] {
        let workspace = fixture();
        root_json(&workspace, declaration);
        assert_eq!(
            check_workspace(&workspace.root),
            Err(root_error(&workspace))
        );
        root_json(&workspace, JSON);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
    for declaration in [
        "{ package = \"serde_json\", version = \"=1.0.151\", features = [\"float_roundtrip\", \"raw_value\"] }",
        "{ version = \"=1.0.151\", features = [\"raw_value\", \"float_roundtrip\"] }",
    ] {
        let workspace = fixture();
        root_json(&workspace, declaration);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn dev_and_build_features_do_not_supply_normal_precision() {
    for section in [
        "dev-dependencies",
        "build-dependencies",
        "target.'cfg(unix)'.dev-dependencies",
        "target.'cfg(windows)'.build-dependencies",
    ] {
        let workspace = fixture();
        workspace.member("tui", "maestro-tui", &format!("[dependencies]\nserde_json.workspace = true\n[{section}]\nserde_json = {{ version = \"=1.0.151\", features = [\"float_roundtrip\"] }}\n"));
        root_json(&workspace, "{ version = \"=1.0.151\" }");
        assert_eq!(
            check_workspace(&workspace.root),
            Err(root_error(&workspace))
        );
        root_json(&workspace, JSON);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

/// Render the required member diagnostic at the declaration being checked.
fn member_error(workspace: &Workspace, directory: &str, section: &str, alias: &str) -> String {
    format!(
        "{}: {section}.{alias} must inherit workspace.dependencies.{alias} with float_roundtrip on the normal dependency path",
        workspace.root.join(directory).join("Cargo.toml").display()
    )
}

#[test]
fn normal_json_dependencies_must_inherit_workspace() {
    for declaration in ["\"=1.0.151\"", "{ version = \"=1.0.151\" }", JSON] {
        let workspace = fixture();
        workspace.member(
            "tui",
            "maestro-tui",
            &format!("[dependencies]\nserde_json = {declaration}\n"),
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Err(member_error(
                &workspace,
                "crates/tui",
                "dependencies",
                "serde_json"
            ))
        );
        workspace.member(
            "tui",
            "maestro-tui",
            "[dependencies]\nserde_json.workspace = true\n",
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

/// Set the same-key workspace entry used by renamed dependency fixtures.
fn workspace_alias(workspace: &Workspace, declaration: &str) {
    let path = workspace.root.join("Cargo.toml");
    let mut document: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    let entry: toml::Table = format!("json = {declaration}").parse().unwrap();
    document["workspace"]["dependencies"]
        .as_table_mut()
        .unwrap()
        .extend(entry);
    std::fs::write(path, document.to_string()).unwrap();
}

#[test]
fn renamed_json_dependencies_must_inherit_workspace() {
    for features in ["", ", features = [\"float_roundtrip\"]"] {
        let workspace = fixture();
        workspace.member("tui", "maestro-tui", &format!("[dependencies]\njson = {{ package = \"serde_json\", version = \"=1.0.151\"{features} }}\n"));
        assert_eq!(
            check_workspace(&workspace.root),
            Err(member_error(
                &workspace,
                "crates/tui",
                "dependencies",
                "json"
            ))
        );
        workspace_alias(
            &workspace,
            "{ package = \"serde_json\", version = \"=1.0.151\", features = [\"float_roundtrip\"] }",
        );
        workspace.member(
            "tui",
            "maestro-tui",
            "[dependencies]\njson.workspace = true\n",
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn inherited_json_aliases_require_workspace_precision() {
    for features in ["", ", features = []"] {
        for declaration in [
            "[dependencies]\njson.workspace = true\n",
            "[dependencies]\njson = { workspace = true, features = [\"float_roundtrip\"] }\n",
            "[dependencies]\njson.workspace = true\n[dev-dependencies]\njson = { workspace = true, features = [\"float_roundtrip\"] }\n",
        ] {
            let workspace = fixture();
            workspace_alias(
                &workspace,
                &format!("{{ package = \"serde_json\", version = \"=1.0.151\"{features} }}"),
            );
            workspace.member("tui", "maestro-tui", declaration);
            assert_eq!(
                check_workspace(&workspace.root),
                Err(member_error(
                    &workspace,
                    "crates/tui",
                    "dependencies",
                    "json"
                ))
            );
            workspace_alias(
                &workspace,
                "{ package = \"serde_json\", version = \"=1.0.151\", features = [\"float_roundtrip\"] }",
            );
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        }
    }
}

#[test]
fn target_json_dependencies_keep_normal_precision() {
    for target in ["cfg(unix)", "cfg(windows)", "aarch64-unknown-linux-gnu"] {
        for (alias, package, optional) in [
            ("serde_json", "", ""),
            ("json", "package = \"serde_json\", ", ""),
            ("json", "package = \"serde_json\", ", ", optional = true"),
        ] {
            let workspace = fixture();
            workspace_alias(
                &workspace,
                "{ package = \"serde_json\", version = \"=1.0.151\", features = [\"float_roundtrip\"] }",
            );
            workspace.member("tui", "maestro-tui", &format!("[target.{target:?}.dependencies]\n{alias} = {{ {package}version = \"=1.0.151\"{optional} }}\n"));
            assert_eq!(
                check_workspace(&workspace.root),
                Err(member_error(
                    &workspace,
                    "crates/tui",
                    &format!("target.{target:?}.dependencies"),
                    alias
                ))
            );
            workspace.member(
                "tui",
                "maestro-tui",
                &format!(
                    "[target.{target:?}.dependencies]\n{alias} = {{ workspace = true{optional} }}\n"
                ),
            );
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        }
    }
}

#[test]
fn optional_json_dependencies_keep_normal_precision() {
    for (alias, package) in [("serde_json", ""), ("json", "package = \"serde_json\", ")] {
        let workspace = fixture();
        workspace.member(
            "tui",
            "maestro-tui",
            &format!(
                "[dependencies]\n{alias} = {{ {package}version = \"=1.0.151\", optional = true }}\n"
            ),
        );
        assert_eq!(
            check_workspace(&workspace.root),
            Err(member_error(
                &workspace,
                "crates/tui",
                "dependencies",
                alias
            ))
        );
        workspace_alias(
            &workspace,
            "{ package = \"serde_json\", version = \"=1.0.151\", features = [\"float_roundtrip\"] }",
        );
        workspace.member(
            "tui",
            "maestro-tui",
            &format!("[dependencies]\n{alias} = {{ workspace = true, optional = true }}\n"),
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn inherited_json_dependencies_preserve_added_features() {
    for alias in ["serde_json", "json"] {
        for features in ["[]", "[\"raw_value\"]"] {
            for defaults in ["", ", default-features = false"] {
                let workspace = fixture();
                let entry = format!(
                    "{{ package = \"serde_json\", version = \"=1.0.151\", features = [\"float_roundtrip\"]{defaults} }}"
                );
                root_json(&workspace, &entry);
                workspace_alias(&workspace, &entry);
                workspace.member("tui", "maestro-tui", &format!("[dependencies]\n{alias} = {{ workspace = true, features = {features}{defaults} }}\n"));
                assert_eq!(check_workspace(&workspace.root), Ok(()));
            }
        }
    }
}

#[test]
fn dependency_package_identity_controls_json_check() {
    for (alias, entry, inherited) in [
        (
            "serde_json",
            "{ package = \"another\", version = \"9\" }",
            false,
        ),
        ("other", "{ package = \"another\", version = \"9\" }", false),
        ("json", "{ package = \"another\", version = \"9\" }", true),
    ] {
        let workspace = Workspace::new();
        workspace.external("another");
        workspace.member("tui", "maestro-tui", "");
        workspace.list(&[("maestro-tui", "core")]);
        let entry = if inherited {
            let path = workspace.root.join("Cargo.toml");
            let mut document: toml::Table =
                std::fs::read_to_string(&path).unwrap().parse().unwrap();
            document["workspace"]["dependencies"]
                .as_table_mut()
                .unwrap()
                .extend(format!("{alias} = {entry}").parse::<toml::Table>().unwrap());
            std::fs::write(path, document.to_string()).unwrap();
            "{ workspace = true }"
        } else {
            entry
        };
        workspace.member(
            "tui",
            "maestro-tui",
            &format!("[dependencies]\n{alias} = {entry}\n"),
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
}

#[test]
fn nonproduction_json_dependencies_are_outside_inheritance_check() {
    for section in [
        "dev-dependencies",
        "build-dependencies",
        "target.'cfg(unix)'.dev-dependencies",
        "target.'cfg(windows)'.build-dependencies",
        "target.aarch64-unknown-linux-gnu.dev-dependencies",
        "target.aarch64-unknown-linux-gnu.build-dependencies",
    ] {
        for (alias, entry) in [
            ("serde_json", "{ version = \"=1.0.151\" }"),
            (
                "json",
                "{ package = \"serde_json\", version = \"=1.0.151\" }",
            ),
            ("json", "{ workspace = true }"),
        ] {
            let workspace = fixture();
            workspace_alias(
                &workspace,
                "{ package = \"serde_json\", version = \"=1.0.151\" }",
            );
            workspace.member(
                "tui",
                "maestro-tui",
                &format!("[{section}]\n{alias} = {entry}\n"),
            );
            assert_eq!(check_workspace(&workspace.root), Ok(()));
        }
    }
    let workspace = fixture();
    workspace_alias(
        &workspace,
        "{ package = \"serde_json\", version = \"=1.0.151\" }",
    );
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn json_rule_reads_toml_structure_not_text() {
    for declaration in [
        "[dependencies]\n\"serde_json\".workspace = true\n",
        "[dependencies.serde_json]\nworkspace = true\n",
        "[dependencies]\n\"json\" = { workspace = true }\n",
        "[dependencies.\"json\"]\nworkspace = true\n",
    ] {
        let workspace = fixture();
        workspace_alias(
            &workspace,
            "{ package = \"serde_json\", version = \"=1.0.151\", features = [\"float_roundtrip\"] }",
        );
        workspace.member("tui", "maestro-tui", declaration);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
    for distraction in [
        "# float_roundtrip\n",
        "[workspace.metadata]\nfeatures = [\"float_roundtrip\"]\n",
        "[package]\nname = \"maestro-tui\"\nversion = \"0.1.0\"\ndescription = '''\nfloat_roundtrip\n'''\n[lints]\nworkspace = true\n",
    ] {
        let workspace = fixture();
        root_json(&workspace, "{ version = \"=1.0.151\" }");
        let path = workspace.root.join("Cargo.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        if distraction.contains("name =") {
            std::fs::create_dir(workspace.root.join("src")).unwrap();
            std::fs::write(workspace.root.join("src/lib.rs"), "").unwrap();
            std::fs::remove_dir_all(workspace.root.join("crates/tui")).unwrap();
        }
        std::fs::write(
            &path,
            format!(
                "{}\n{distraction}",
                if distraction.contains("name =") {
                    text.replace("members = [\"crates/*\"]", "members = []")
                } else {
                    text
                }
            ),
        )
        .unwrap();
        assert_eq!(
            check_workspace(&workspace.root),
            Err(root_error(&workspace))
        );
        root_json(&workspace, JSON);
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
    let workspace = fixture();
    root_json(
        &workspace,
        "{ version = \"=1.0.151\", features = [\"float_roundtrip_extra\"] }",
    );
    workspace.member(
        "tui",
        "maestro-tui",
        "[dependencies]\nserde_json.workspace = true\n",
    );
    assert_eq!(
        check_workspace(&workspace.root),
        Err(root_error(&workspace))
    );
    root_json(&workspace, JSON);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn json_checks_include_guest_and_all_workspace_members() {
    for (directory, name) in [("guest", "maestro-extensions-wasm"), ("tui", "maestro-tui")] {
        let workspace = fixture();
        workspace.member(
            "tui",
            "maestro-tui",
            "[dependencies]\nserde_json.workspace = true\n",
        );
        workspace.member(
            directory,
            name,
            "[dependencies]\nserde_json = \"=1.0.151\"\n",
        );
        if name == "maestro-tui" {
            workspace.member(
                "settings",
                "maestro-settings",
                "[dependencies]\nserde_json.workspace = true\n",
            );
            workspace.list(&[("maestro-settings", "core"), ("maestro-tui", "core")]);
        } else {
            workspace.list(&[("maestro-tui", "core"), (name, "core")]);
        }
        assert_eq!(
            check_workspace(&workspace.root),
            Err(member_error(
                &workspace,
                &format!("crates/{directory}"),
                "dependencies",
                "serde_json"
            ))
        );
        workspace.member(
            directory,
            name,
            "[dependencies]\nserde_json.workspace = true\n",
        );
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
    let workspace = fixture();
    let path = workspace.root.join("Cargo.toml");
    std::fs::remove_dir_all(workspace.root.join("crates/tui")).unwrap();
    std::fs::create_dir(workspace.root.join("src")).unwrap();
    std::fs::write(workspace.root.join("src/lib.rs"), "").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.replace("members = [\"crates/*\"]", "members = []");
    let package = "[package]\nname = \"maestro-tui\"\nversion = \"0.1.0\"\n[lints]\nworkspace = true\n[dependencies]\nserde_json = \"=1.0.151\"\n";
    std::fs::write(&path, format!("{text}\n{package}")).unwrap();
    assert_eq!(
        check_workspace(&workspace.root),
        Err(member_error(&workspace, "", "dependencies", "serde_json"))
    );
    std::fs::write(
        path,
        format!(
            "{text}\n{}",
            package.replace("serde_json = \"=1.0.151\"", "serde_json.workspace = true")
        ),
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}
