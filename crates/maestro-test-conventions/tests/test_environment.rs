//! Environment names checked through the workspace boundary.
#![cfg(test)]

use crate::support::{Workspace, source};
use maestro_test_conventions::check_workspace;

/// Build a hypothetical variable without declaring it in this test's own source.
fn fixture(text: &str, suffix: &str) -> String {
    text.replace("$NAME", &format!("MAESTRO_{suffix}"))
}

#[test]
fn production_occurrences_keep_names_even_across_members() {
    let workspace = Workspace::with_text_member();
    workspace.member("json", "maestro-json", "");
    workspace.list(&[("maestro-text", "core"), ("maestro-json", "core")]);
    source(
        &workspace,
        "text",
        "src/tests.rs",
        &fixture("fn read() { let _ = std::env::var(\"$NAME\"); }", "FOO"),
    );
    source(
        &workspace,
        "json",
        "src/lib.rs",
        &fixture("const KEY: &str = \"$NAME\";", "FOO"),
    );

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn gated_items_layouts_and_support_modules_are_test_owned() {
    for (path, text) in [
        ("src/tests.rs", "const KEY: &str = \"$NAME\";"),
        ("src/tests/nested.rs", "const KEY: &str = r#\"$NAME\"#;"),
        ("src/tests.rs", "const KEY: &str = \"PREFIX$NAME suffix\";"),
        ("src/tests.rs", "\"$NAME\","),
        ("src/lib.rs", "#[test] fn reads() { let _ = \"$NAME\"; }"),
        ("tests/read.rs", "fn read() { outer!(inner!(\"$NAME\")); }"),
        (
            "src/lib.rs",
            "#[cfg(test)] mod tests { const KEY: &str = \"$NAME\"; }",
        ),
        ("src/lib.rs", "#[cfg(test)] const $NAME: &str = \"value\";"),
        (
            "src/lib.rs",
            "#[cfg(test)] fn read() { stringify!(r#$NAME); }",
        ),
        (
            "src/lib.rs",
            "mod test_support { const KEY: &str = \"$NAME\"; }",
        ),
        (
            "src/lib.rs",
            "mod r#test_support { const KEY: &str = \"$NAME\"; }",
        ),
        ("src/test_support.rs", "const KEY: &str = \"$NAME\";"),
        ("src/test_support/nested.rs", "const KEY: &str = \"$NAME\";"),
        (
            "src/lib.rs",
            "#[cfg(feature = \"test-support\")] fn read() { let _ = \"$NAME\"; }",
        ),
        (
            "src/lib.rs",
            "#[cfg(any(test, feature = \"test-support\"))] const KEY: &str = \"$NAME\";",
        ),
        (
            "src/lib.rs",
            "#[cfg(all(unix, feature = \"test-support\"))] impl Reader { fn read() { let _ = \"$NAME\"; } }",
        ),
        (
            "src/reader.rs",
            "#![cfg(feature = \"test-support\")] const KEY: &str = \"$NAME\";",
        ),
    ] {
        let workspace = Workspace::with_text_member();
        let path = source(&workspace, "text", path, &fixture(text, "FOO"));
        if path.ends_with("tests/read.rs") {
            workspace.integration("text");
        }

        let error = check_workspace(workspace.root.path()).unwrap_err();

        assert!(error.contains("test-only environment variable"), "{error}");
        assert!(error.contains(&path.display().to_string()), "{error}");
    }
}

#[test]
fn external_module_gates_cover_their_descendants() {
    for (declaration, module) in [
        ("mod fixture;", "fixture"),
        ("#[path = \"gated.rs\"] mod fixture;", "gated"),
        ("#[path = \"../src/gated.rs\"] mod fixture;", "gated"),
        (
            "mod inline { #[path = \"gated.rs\"] mod fixture; }",
            "inline/gated",
        ),
    ] {
        let workspace = Workspace::with_text_member();
        source(
            &workspace,
            "text",
            "src/lib.rs",
            &format!("#[cfg(feature = \"test-support\")] {declaration}\n"),
        );
        source(
            &workspace,
            "text",
            &format!("src/{module}.rs"),
            "mod nested;\n",
        );
        let path = source(
            &workspace,
            "text",
            &format!("src/{module}/nested.rs"),
            &fixture("const KEY: &str = \"$NAME\";", "FOO"),
        );

        let error = check_workspace(workspace.root.path()).unwrap_err();

        assert!(error.contains(&path.display().to_string()), "{error}");
    }
}

#[test]
fn dedicated_crate_sources_are_test_owned() {
    let workspace = Workspace::new();
    workspace.member("fixtures", "maestro-test-fixtures", "");
    workspace.list(&[("maestro-test-fixtures", "dedicated")]);
    source(
        &workspace,
        "fixtures",
        "src/lib.rs",
        &fixture("const KEY: &str = \"$NAME\";", "FOO"),
    );

    let error = check_workspace(workspace.root.path()).unwrap_err();

    assert!(error.contains("test-only environment variable"), "{error}");
}

#[test]
fn production_cfg_alternatives_and_siblings_remain_production() {
    for text in [
        "#[cfg(any(unix, feature = \"test-support\"))] const KEY: &str = \"$NAME\";",
        "#[cfg(not(test))] const KEY: &str = \"$NAME\";",
        "#[cfg(test)] mod tests { const TEST: &str = \"$NAME\"; } const KEY: &str = \"$NAME\";",
        "mod test_support { const TEST: &str = \"$NAME\"; } const KEY: &str = \"$NAME\";",
    ] {
        let workspace = Workspace::with_text_member();
        source(&workspace, "text", "src/lib.rs", &fixture(text, "FOO"));

        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[test]
fn production_documentation_keeps_dynamically_built_keys() {
    let workspace = Workspace::with_text_member();
    source(
        &workspace,
        "text",
        "src/lib.rs",
        &fixture(
            "/// The `$NAME` override.\nconst KEY: &str = \"dynamic\";",
            "FOO",
        ),
    );
    source(
        &workspace,
        "text",
        "src/tests.rs",
        &fixture("const KEY: &str = \"$NAME\";", "FOO"),
    );

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn comments_do_not_create_production_occurrences() {
    let workspace = Workspace::with_text_member();
    source(
        &workspace,
        "text",
        "src/lib.rs",
        &fixture("// $NAME\n/* $NAME */\n", "FOO"),
    );
    source(
        &workspace,
        "text",
        "src/tests.rs",
        &fixture("const KEY: &str = \"$NAME\";", "FOO"),
    );

    let error = check_workspace(workspace.root.path()).unwrap_err();

    assert!(error.contains("test-only environment variable"), "{error}");
}

#[test]
fn prefixed_test_only_names_are_accepted() {
    let workspace = Workspace::with_text_member();
    source(
        &workspace,
        "text",
        "src/tests.rs",
        &fixture(
            "fn read() { let _ = std::env::var(\"$NAME\"); }",
            "TEST_FOO",
        ),
    );

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn unprefixed_test_only_names_report_every_file() {
    let workspace = Workspace::with_text_member();
    let contents = fixture("fn read() { let _ = std::env::var(\"$NAME\"); }", "FOO");
    let paths = ["src/tests.rs", "src/tests/other.rs"]
        .map(|path| source(&workspace, "text", path, &contents));

    let error = check_workspace(workspace.root.path()).unwrap_err();

    assert!(error.contains(&format!("`MAESTRO_{}`", "FOO")));
    for path in paths {
        assert!(error.contains(&format!("{}:1", path.display())), "{error}");
    }
}
