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
fn duplicate_assertions_remain_review_judgement() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    std::fs::write(
        workspace.root.join("crates/tui/src/lib.rs"),
        "#[test]\nfn sample() {\n assert_eq!(value, 3);\n assert_eq!( value , 3 );\n}\n",
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
fn initializers_and_signatures_do_not_hide_module_conventions() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    let mut failures = Vec::new();
    for source in [
        "const VALUE: () = { mod tests {} };",
        "static VALUE: () = { mod tests {} };",
        "struct X; impl X { const VALUE: () = { mod tests {} }; }",
        "trait X { const VALUE: () = { mod tests {} }; }",
        "fn outer() { let value = const { mod tests {} }; }",
        "fn outer(_: [(); { mod tests {} 0 }]) {}",
        "struct X; impl X { fn outer(_: [(); { mod tests {} 0 }]) {} }",
        "trait X { fn outer(_: [(); { mod tests {} 0 }]) {} }",
    ] {
        std::fs::write(&path, source).unwrap();
        match check_workspace(&workspace.root) {
            Ok(()) => failures.push(format!("missed initializer module convention: {source}")),
            Err(error) => {
                assert!(error.contains(&format!("{}:1:", path.display())), "{error}");
                assert!(error.contains("test module convention"), "{error}");
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn nested_test_module_declarations_follow_layout_conventions() {
    let workspace = Workspace::new();
    workspace.member("tui", "maestro-tui", "");
    workspace.list(&[("maestro-tui", "core")]);
    let path = workspace.root.join("crates/tui/src/lib.rs");
    for enclosing in [
        "fn outer() { BODY }",
        "struct X; impl X { fn outer() { BODY } }",
        "trait X { fn outer() { BODY } }",
    ] {
        for declaration in ["#[cfg(test)] mod helpers {}", "mod tests {}"] {
            std::fs::write(&path, enclosing.replace("BODY", declaration)).unwrap();
            let error = check_workspace(&workspace.root).unwrap_err();
            assert!(error.contains(&format!("{}:1:", path.display())), "{error}");
            assert!(error.contains("test module convention"), "{error}");
        }
    }
}
