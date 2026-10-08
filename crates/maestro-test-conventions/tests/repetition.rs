#![cfg(test)]

mod support;

use maestro_test_conventions::check_workspace;
use support::Workspace;

#[test]
fn repeated_documentation_reports_both_lines() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    std::fs::write(
        &path,
        "/// Read a value.\n///   Read a value.  \nfn read() {}\n",
    )
    .unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.contains(&format!("{}:2:", path.display())), "{error}");
    assert!(
        error.contains("repeated documentation; first at line 1"),
        "{error}"
    );
}

#[test]
fn documentation_ignores_code_fences_and_resets_at_each_item() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    std::fs::write(
        workspace.root.join("crates/tui/src/lib.rs"),
        r#"
//! Module documentation.
/// Read a value.
/// ```text
/// repeated code
/// repeated code
/// ```
/// Read a value.
fn first() {}
/** Read a value. */
fn second() {}
/// ~~~~text
/// repeated code
/// repeated code
/// ~~~~
fn third() {}
const TEXT: &str = "/// repeated\n/// repeated";
// repeated prose
// repeated prose
"#,
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn adjacent_identical_assertions_report_both_locations() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    std::fs::write(
        &path,
        "#[test]\nfn sample() {\n assert_eq!(value, 3);\n assert_eq!( value , 3 );\n}\n",
    )
    .unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.contains(&format!("{}:4:", path.display())), "{error}");
    assert!(
        error.contains("repeated assertion; first at line 3"),
        "{error}"
    );
}

#[test]
fn assertions_after_state_changes_and_in_distinct_blocks_pass() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    std::fs::write(
        workspace.root.join("crates/tui/src/lib.rs"),
        r#"
#[test]
fn first() {
    assert_eq!(value, 3);
    change_state();
    assert_eq!(value, 3);
    { assert_eq!(value, 3); }
    { assert_eq!(value, 3); }
    assert!(ready);
    assert!(!ready);
    assert_eq!(value, "a b");
    assert_eq!(value, "a  b");
}
#[test]
fn second() { assert_eq!(value, 3); }
fn helper() {
    assert_eq!(value, 3);
    assert_eq!(value, 3);
}
"#,
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn opaque_assertion_arguments_remain_review_judgement() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    std::fs::write(
        workspace.root.join("crates/tui/src/lib.rs"),
        r"
#[test]
fn sample() {
    assert!(iter.next().is_some());
    assert!(iter.next().is_some());
    assert!(observe());
    assert!(observe());
    assert_eq!(value!(), 1);
    assert_eq!(value!(), 1);
    assert_eq!({ value = 1; value }, 1);
    assert_eq!({ value = 1; value }, 1);
    assert_eq!({ value += 1; value }, 1);
    assert_eq!({ value += 1; value }, 1);
    assert_matches!(value, Some(_) if observe());
    assert_matches!(value, Some(_) if observe());
    assert_matches!(value, pattern!());
    assert_matches!(value, pattern!());
}
",
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn block_documentation_repetition_keeps_physical_line_numbers() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    std::fs::write(
        &path,
        "/**\r\n * Read a value.\r\n * Read a value.\r\n */\r\nfn read() {}\r\n",
    )
    .unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.contains(&format!("{}:3:", path.display())), "{error}");
    assert!(
        error.contains("repeated documentation; first at line 2"),
        "{error}"
    );
}

#[test]
fn markdown_list_marker_is_not_trimmed_from_line_documentation() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    std::fs::write(
        workspace.root.join("crates/tui/src/lib.rs"),
        "/// * Read a value.\n/// Read a value.\nfn read() {}\n",
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn assertion_families_are_checked_in_nested_test_blocks() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    for call in [
        "assert!(ready)",
        "assert_ne!(value, 1)",
        "assert_matches!(value, 1)",
        "assert_matches!(value, Some(_))",
        "assert_matches!(value, Ok(binding))",
        "assert_matches!(value, binding @ Some(_) if ready)",
        "debug_assert_matches!(value, None)",
        "std::debug_assert_eq!(value, 1)",
    ] {
        std::fs::write(
            &path,
            format!("#[test]\nfn sample() {{\n if ready {{\n {call};\n {call};\n }}\n}}\n"),
        )
        .unwrap();
        let error = check_workspace(&workspace.root).unwrap_err();
        assert!(error.contains(&format!("{}:5:", path.display())), "{error}");
        assert!(
            error.contains("repeated assertion; first at line 4"),
            "{error}"
        );
    }
}

#[test]
fn inner_and_outer_documentation_have_distinct_owners() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    for source in [
        "//! Shared description.\n/// Shared description.\npub struct X;\n",
        "/*! Shared description. */\n/** Shared description. */\npub struct X;\n",
        "//! ```\n/// Shared description.\n/// Shared description.\npub struct X;\n",
    ] {
        std::fs::write(&path, source).unwrap();
        let result = check_workspace(&workspace.root);
        if source.contains("```") {
            assert!(result.unwrap_err().contains("repeated documentation"));
        } else {
            assert_eq!(result, Ok(()));
        }
    }
}

#[test]
fn markdown_container_code_blocks_are_not_paragraphs() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    for source in [
        "/// > ~~~text\n/// > repeated\n/// > repeated\n/// > ~~~\nfn sample() {}\n",
        "/// - ```text\n///   repeated\n///   repeated\n///   ```\nfn sample() {}\n",
        "///     repeated\n///     repeated\nfn sample() {}\n",
    ] {
        std::fs::write(&path, source).unwrap();
        assert_eq!(check_workspace(&workspace.root), Ok(()));
    }
    std::fs::write(
        &path,
        "/// > Same sentence.\n/// > Same sentence.\nfn sample() {}\n",
    )
    .unwrap();
    assert!(
        check_workspace(&workspace.root)
            .unwrap_err()
            .contains("repeated documentation")
    );
}

#[test]
fn assertion_attributes_are_part_of_statement_identity() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    std::fs::write(
        workspace.root.join("crates/tui/src/lib.rs"),
        "#[test]\nfn sample() { #[cfg(unix)] assert!(ready); #[cfg(not(unix))] assert!(ready); }\n",
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn production_methods_and_initializers_are_not_test_blocks() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    std::fs::write(
        workspace.root.join("crates/tui/src/lib.rs"),
        r"
struct X;
impl X {
    fn sample() { assert!(ready); assert!(ready); }
    const VALUE: () = { assert!(ready); assert!(ready); };
}
trait Sample { fn sample() { assert!(ready); assert!(ready); } }
const VALUE: () = { assert!(ready); assert!(ready); };
static OTHER: () = { assert!(ready); assert!(ready); };
#[test]
fn initializers() {
    const LOCAL: () = { assert!(ready); assert!(ready); };
    static LOCAL_STATIC: () = { assert!(ready); assert!(ready); };
    let value = const { assert!(ready); assert!(ready); };
}
",
    )
    .unwrap();
    assert_eq!(check_workspace(&workspace.root), Ok(()));
}

#[test]
fn helpers_inside_test_modules_are_selected() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    for source in [
        "#[cfg(test)] mod tests { mod nested { fn helper() { assert!(ready); assert!(ready); } } }",
        "#[tokio::test] async fn sample() { assert!(ready); assert!(ready); }",
    ] {
        std::fs::write(&path, source).unwrap();
        assert!(
            check_workspace(&workspace.root)
                .unwrap_err()
                .contains("repeated assertion")
        );
    }
}
