#![cfg(test)]

mod support;

use maestro_test_conventions::check_workspace;
use support::Workspace;

#[test]
fn production_files_stop_at_five_hundred_lines() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    std::fs::write(&path, "// Technical.\n".repeat(500)).unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    std::fs::write(&path, "// Technical.\n".repeat(501)).unwrap();
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("501 production lines")
    );
}

#[test]
fn quality_lint_allowances_are_rejected() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    std::fs::write(
        &path,
        "#![allow(clippy::too_many_lines, reason = \"scheduled for re-implementation\")]\n",
    )
    .unwrap();
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("quality lint allowance")
    );
}

#[test]
fn all_crates_must_inherit_workspace_lints() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let manifest = workspace.root.join("crates/tui/Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(&manifest, text.replace("[lints]\nworkspace = true\n", "")).unwrap();
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("must inherit workspace lints")
    );
}

#[test]
fn test_files_and_trailing_test_modules_do_not_count() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let root = workspace.root.join("crates/tui");
    let tests = "// Technical.\n".repeat(501);
    std::fs::create_dir(root.join("tests")).unwrap();
    std::fs::write(root.join("tests/behavior.rs"), &tests).unwrap();
    std::fs::write(root.join("src/tests.rs"), &tests).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        format!(
            "{}#[cfg(test)]\nmod tests {{\n{tests}}}\n",
            "// Technical.\n".repeat(500)
        ),
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    std::fs::write(
        root.join("src/lib.rs"),
        format!("#[cfg(test)]\nmod tests {{}}\n{tests}"),
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    // Production declarations after a test module still count.
    std::fs::write(
        root.join("src/lib.rs"),
        format!("#[cfg(test)]\nmod tests {{}}\n{tests}pub fn production() {{}}\n"),
    )
    .unwrap();
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("production lines")
    );
}

#[test]
fn quality_allowances_are_attributes_not_innocent_source_tokens() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    std::fs::write(
        &path,
        "fn allow(unwrap_used: usize) {}\nconst TEXT: &str = \"#[allow(clippy::unwrap_used)]\";\n",
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    for attribute in [
        "#[allow(clippy::unwrap_used)]",
        "#![allow(clippy::pedantic)]",
        "#[cfg_attr(unix, allow(clippy::too_many_lines))]",
        "#![allow(unsafe_code)]",
        "#[allow(clippy::doc_markdown)]",
        "#[expect(clippy::unwrap_used)]",
    ] {
        std::fs::write(&path, format!("{attribute}\nfn fixture() {{}}\n")).unwrap();
        assert!(
            check_workspace(&workspace.root)
                .unwrap_err()
                .contains("quality lint allowance")
        );
    }
}

#[test]
fn multiple_trailing_test_modules_do_not_count() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    std::fs::write(
        &path,
        format!(
            "{}#[cfg(test)]\nmod first {{\n{}}}\n#[cfg(test)]\nmod second {{}}\n",
            "// Technical.\n".repeat(500),
            "// Technical.\n".repeat(501)
        ),
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn lint_inheritance_uses_the_manifest_table() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let manifest = workspace.root.join("crates/tui/Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    let forged = text.replace("[lints]\nworkspace = true\n", "").replace(
        "[package]\n",
        "[package]\ndescription = '''\n[lints]\nworkspace = true\n'''\n",
    );
    std::fs::write(&manifest, forged).unwrap();
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("must inherit workspace lints")
    );
    std::fs::write(
        &manifest,
        text.replace("workspace = true", "\"workspace\" = true"),
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn lint_groups_are_rejected_at_every_attribute_depth() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    for contents in [
        "#![allow(clippy::restriction)]\n",
        "fn outer() { #[expect(clippy::pedantic)] fn nested() {} }\n",
        "#[cfg_attr(test, allow(clippy::all))] fn fixture() {}\n",
    ] {
        std::fs::write(&path, contents).unwrap();
        assert!(
            check_workspace(&workspace.root)
                .unwrap_err()
                .contains("quality lint allowance"),
            "{contents}"
        );
    }
}

#[test]
fn test_modules_with_extra_attributes_do_not_count_as_production() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let root = workspace.root.join("crates/tui/src");
    std::fs::write(root.join("tests.rs"), "").unwrap();
    for attributes in [
        "#[cfg(test)]\n#[path = \"tests.rs\"]\n",
        "#[path = \"tests.rs\"]\n#[cfg(test)]\n",
    ] {
        std::fs::write(
            root.join("lib.rs"),
            format!("{}{attributes}mod tests;\n", "// Technical.\n".repeat(500)),
        )
        .unwrap();
        assert_eq!(check_workspace(&workspace.root), Ok(()));
        std::fs::write(
            root.join("lib.rs"),
            format!("{}{attributes}mod tests;\n", "// Technical.\n".repeat(501)),
        )
        .unwrap();
        assert!(
            check_workspace(&workspace.root)
                .unwrap_err()
                .contains("501 production lines")
        );
    }
}

#[test]
fn nested_test_items_do_not_count_as_production() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    std::fs::write(&path, format!(
        "struct Thing;\nimpl Thing {{\n#[cfg(test)]\nfn helper() {{\n{}}}\n}}\npub fn production() {{}}\n",
        "// Technical.\n".repeat(501),
    )).unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn invalid_rust_source_reports_its_file() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    std::fs::write(&path, "fn broken( {").unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.contains(&path.display().to_string()), "{error}");
    assert!(error.contains("cannot parse"), "{error}");
}
