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
