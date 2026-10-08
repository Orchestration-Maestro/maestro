#![cfg(test)]

mod support;

use maestro_test_conventions::check_workspace;
use support::Workspace;

#[test]
fn generated_bindings_reject_repeated_line_documentation() {
    let workspace = Workspace::new();
    workspace.member("guest", "maestro-extensions-wasm", "");
    workspace.list(&[("maestro-extensions-wasm", "core")]);
    let path = workspace.root.join("crates/guest/src/bindings.rs");
    for marker in ["///", "//!"] {
        std::fs::write(
            &path,
            format!("{marker} Repeat.\n{marker} Repeat.\npub struct Item;\n"),
        )
        .unwrap();
        let error = check_workspace(&workspace.root).unwrap_err();
        assert_eq!(
            error,
            format!(
                "{}:2: repeated documentation; first at line 1",
                path.display()
            ),
            "{marker}"
        );
    }
}

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
/// Read a value.
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

#[test]
fn physical_documentation_lines_cover_tight_lists_and_separators() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    let mut failures = Vec::new();
    for prose in [
        "- Intro.\n  Repeat.\n  Repeat.",
        "- Repeat.\n- Repeat.",
        "> - Intro.\n>   Repeat.\n>   Repeat.",
        "1. Intro.\n   Repeat.\n   Repeat.",
    ] {
        let lines: Vec<_> = prose.lines().collect();
        let source = format!("/// {}\nfn sample() {{}}\n", lines.join("\n/// "));
        std::fs::write(&path, source).unwrap();
        let Err(error) = check_workspace(&workspace.root) else {
            failures.push(format!("missed repetition: {prose:?}"));
            continue;
        };
        let last = lines.len();
        assert!(
            error.contains(&format!("{}:{last}:", path.display())),
            "{error}"
        );
        assert!(
            error.contains(&format!(
                "repeated documentation; first at line {}",
                last - 1
            )),
            "{error}"
        );
    }
    for separator in ["# Different", "---", "<div>Different</div>"] {
        std::fs::write(
            &path,
            format!("/// Same.\n/// {separator}\n/// Same.\nfn sample() {{}}\n"),
        )
        .unwrap();
        if let Err(error) = check_workspace(&workspace.root) {
            failures.push(format!("false rejection across {separator}: {error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn test_only_files_select_unannotated_helpers() {
    let mut failures = Vec::new();
    for (file, declaration, prefix) in [
        ("tests.rs", "#[cfg(test)] mod tests;", ""),
        ("tests/support.rs", "#[cfg(test)] mod tests;", ""),
        ("helpers.rs", "mod helpers;", "#![cfg(test)]\n"),
    ] {
        let workspace = Workspace::new();
        workspace.member("tui", "maestro-tui", "");
        workspace.list(&[("maestro-tui", "core")]);
        let root = workspace.root.join("crates/tui/src");
        std::fs::write(root.join("lib.rs"), declaration).unwrap();
        if file == "tests/support.rs" {
            std::fs::create_dir(root.join("tests")).unwrap();
            std::fs::write(root.join("tests.rs"), "mod support;").unwrap();
        }
        let path = root.join(file);
        std::fs::write(
            &path,
            format!("{prefix}fn helper() {{\n assert!(ready);\n assert!(ready);\n}}\n"),
        )
        .unwrap();
        match check_workspace(&workspace.root) {
            Ok(()) => failures.push(format!("missed test context: {file}, {prefix:?}")),
            Err(error) => {
                let first = 2 + usize::from(!prefix.is_empty());
                assert!(
                    error.contains(&format!("{}:{}:", path.display(), first + 1)),
                    "{error}"
                );
                assert!(
                    error.contains(&format!("repeated assertion; first at line {first}")),
                    "{error}"
                );
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn source_test_module_declarations_require_local_convention() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    let mut failures = Vec::new();
    for declaration in [
        "#[cfg(test)] mod helpers;",
        "mod tests;",
        "#[cfg(test)] #[path = \"helpers.rs\"] mod tests;",
        "mod tests {}",
        "#[cfg(test)] mod helpers {}",
    ] {
        std::fs::write(&path, declaration).unwrap();
        match check_workspace(&workspace.root) {
            Ok(()) => failures.push(format!("accepted invalid declaration: {declaration}")),
            Err(error) => {
                assert!(error.contains(&format!("{}:1:", path.display())), "{error}");
                assert!(error.contains("test module convention"), "{error}");
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn empty_documentation_lines_open_indented_code_for_both_owners() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    for marker in ["///", "//!"] {
        std::fs::write(
            &path,
            format!("{marker} Example:\n{marker}\n{marker}     repeat\n{marker}     repeat\nfn sample() {{}}\n"),
        )
        .unwrap();
        assert_eq!(check_workspace(&workspace.root), Ok(()), "{marker}");
    }
}

#[test]
fn integration_test_support_helpers_are_test_context() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/tests/support/mod.rs");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "fn helper() {\n assert!(ready);\n assert!(ready);\n}\n",
    )
    .unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.contains(&format!("{}:3:", path.display())), "{error}");
    assert!(
        error.contains("repeated assertion; first at line 2"),
        "{error}"
    );
}

#[test]
fn source_test_only_files_require_recognized_layout() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/helpers.rs");
    std::fs::write(&path, "#![cfg(test)]\nmod nested;\n").unwrap();
    let error = check_workspace(&workspace.root).unwrap_err();
    assert!(error.contains(&format!("{}:1:", path.display())), "{error}");
    assert!(error.contains("test file convention"), "{error}");
}

#[test]
fn awaited_assertion_arguments_and_match_guards_are_opaque() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    for call in [
        "assert!((&mut event).await)",
        "assert_matches!((&mut event).await, Some(_))",
        "assert_matches!(value, Some(_) if (&mut event).await)",
    ] {
        std::fs::write(
            &path,
            format!("#[test]\nasync fn sample() {{ {call}; {call}; }}\n"),
        )
        .unwrap();
        assert_eq!(check_workspace(&workspace.root), Ok(()), "{call}");
    }
}
