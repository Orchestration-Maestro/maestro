#![cfg(test)]

mod support;

use std::fmt::Write;

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
fn protected_lints_cannot_be_downgraded_to_deny() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let manifest = workspace.root.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        text.replace("unwrap_used = \"forbid\"", "unwrap_used = \"deny\""),
    )
    .unwrap();
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("unwrap_used must be forbid")
    );
}

#[test]
fn missing_protected_lints_are_rejected() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let manifest = workspace.root.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(&manifest, text.replace("panic = \"forbid\"\n", "")).unwrap();
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("panic must be forbid")
    );
}

#[test]
fn protected_lints_accept_string_and_priority_table_forms() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    let manifest = workspace.root.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        text.replace(
            "unwrap_used = \"forbid\"",
            "unwrap_used = { level = \"forbid\", priority = 1 }",
        ),
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn included_expression_files_pass_and_count_as_production() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let root = workspace.root.join("crates/tui/src");
    std::fs::write(
        root.join("lib.rs"),
        "pub fn value() -> u8 { include!(\"value.rs\") }\n",
    )
    .unwrap();
    let fragment = root.join("value.rs");
    std::fs::write(&fragment, "1_u8\n").unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    std::fs::write(
        &fragment,
        format!("{}1_u8\n", "// Technical.\n".repeat(499)),
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
    std::fs::write(
        &fragment,
        format!("{}1_u8\n", "// Technical.\n".repeat(500)),
    )
    .unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.contains("value.rs: 501 production lines"), "{error}");
}

#[test]
fn forbidden_lint_groups_cannot_be_downgraded() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let manifest = workspace.root.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        text.replace(
            "forbidden_lint_groups = \"forbid\"",
            "forbidden_lint_groups = \"deny\"",
        ),
    )
    .unwrap();
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("forbidden_lint_groups must be forbid")
    );
}

#[test]
fn production_sharing_lines_with_test_items_still_counts() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    let mut items = String::new();
    for index in 0..501 {
        writeln!(
            items,
            "const P{index}: u8 = 0; #[cfg(test)] const T{index}: u8 = 0;"
        )
        .unwrap();
    }
    std::fs::write(
        &path,
        format!("#[rustfmt::skip]\nmod mixed {{\n{items}}}\n"),
    )
    .unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.contains("504 production lines"), "{error}");
}

#[test]
fn five_hundred_production_lines_with_separate_test_module_pass() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    let mut items = String::new();
    for index in 0..500 {
        writeln!(items, "const P{index}: u8 = 0;").unwrap();
    }
    std::fs::write(&path, format!("{items}#[cfg(test)]\nmod tests {{}}\n")).unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn leading_byte_order_mark_preserves_production_count() {
    assert_production_count_unchanged(|source| format!("\u{feff}{source}"));
}

fn assert_production_count_unchanged(vary: fn(&str) -> String) {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    let mut production = String::new();
    for index in 0..500 {
        writeln!(production, "const P{index}: u8 = 0;").unwrap();
    }
    for suffix in ["", "const EXTRA: u8 = 0;\n"] {
        let baseline = format!("#[cfg(test)] mod tests {{}}\n{production}{suffix}");
        std::fs::write(&path, &baseline).unwrap();
        let expected = check_workspace(&workspace.root);
        if suffix.is_empty() {
            assert_eq!(expected, Ok(()));
        } else {
            assert!(
                expected
                    .as_ref()
                    .unwrap_err()
                    .contains("501 production lines")
            );
        }
        std::fs::write(&path, vary(&baseline)).unwrap();
        assert_eq!(check_workspace(&workspace.root), expected);
    }
}

#[test]
fn crlf_endings_preserve_production_count() {
    assert_production_count_unchanged(|source| source.replace('\n', "\r\n"));
}

#[test]
fn tab_indented_test_items_preserve_production_count() {
    assert_production_count_unchanged(|source| source.replace("#[cfg(test)]", "\t#[cfg(test)]"));
}

#[test]
fn unicode_before_same_line_test_items_preserves_production_count() {
    assert_production_count_unchanged(|source| {
        source.replace(
            "const P0: u8 = 0;",
            "const É: &str = \"é\"; #[cfg(test)] const TEST: u8 = 0;",
        )
    });
}
