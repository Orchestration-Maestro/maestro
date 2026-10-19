//! Decoded string-token ownership without macro expansion or source-tree assumptions.

use super::{check, exact_offset};
use crate::source::{Member, Source};
use std::path::PathBuf;

/// A source with its caller-supplied package identity and directory.
fn member(name: &str, directory: &str, path: &str, contents: &str) -> Member {
    Member {
        name: name.into(),
        directory: PathBuf::from(directory),
        sources: vec![Source {
            path: PathBuf::from(path),
            contents: contents.into(),
            syntax: syn::parse_file(contents).ok(),
        }],
    }
}

/// Decode all Rust string spellings before comparing the complete value.
#[test]
fn exact_child_selection_rejects_string_token_forms() {
    for source in [
        "launch(\"--exact\");",
        "launch(r\"--exact\");",
        "launch(r#\"--exact\"#);",
        "launch(r###\"--exact\"###);",
        "const FLAG: &str = \"\\x2d-exact\";",
        "let flag = \"--ex\\u{61}ct\";",
        "let flag = \"--ex\\\n    act\";",
        "let flags = [\"adjacent\", \"--exact\", \"nearby\"];",
    ] {
        assert!(exact_offset(source).is_some(), "{source}");
    }
    assert_eq!(
        exact_offset("let flags = [\"adjacent\", \"nearby\"];"),
        None
    );
}

/// Literal tokens count inside macros, disabled code and incomplete fragments.
#[test]
fn exact_child_selection_checks_macro_tokens_and_test_scopes() {
    for source in [
        "macro_rules! launch { () => { run(\"--exact\") }; }",
        "outer!(inner!([\"--exact\"]));",
        "#[cfg(test)] fn f() { run(\"--exact\"); }",
        "#[cfg(any())] fn f() { run(\"--exact\"); }",
        "unfinished fragment [ \"--exact\"",
    ] {
        for path in [
            "src/lib.rs",
            "tests/child.rs",
            "tests/support/mod.rs",
            "examples/child.rs",
        ] {
            let package = member("maestro-models", "models", path, source);
            assert!(check(&[package]).is_err(), "{path}: {source}");
        }
    }
}

/// Comments, fixture contents and non-string literals do not select a child.
#[test]
fn exact_child_selection_ignores_comments_and_fixture_strings() {
    for source in [
        "// \"--exact\"\nfn f() {}",
        "/// \"--exact\"\nfn f() {}",
        "//! \"--exact\"\nfn f() {}",
        "/* outer /* \"--exact\" */ inner */ fn f() {}",
        "const SOURCE: &str = r###\"fn f() { run(\"--exact\"); }\"###;",
        "const SOURCE: &str = \"run(\\\"--exact\\\")\";",
        "let x = \"before --exact after\";",
        "let x = b\"--exact\"; let y = c\"--exact\";",
        "let x = \"--exactly\"; let y = \"--exac\";",
    ] {
        assert_eq!(exact_offset(source), None, "{source}");
    }
}

/// The exemption follows the exact Cargo package name, never a path spelling.
#[test]
fn exact_child_selection_exempts_only_the_owner_package() {
    for path in ["src/lib.rs", "tests/child.rs"] {
        for (name, directory, exempt) in [
            ("maestro-test-process", "renamed", true),
            ("maestro-models", "maestro-test-process", false),
            ("maestro-test-process-extra", "renamed", false),
            ("maestro-test-proc", "renamed", false),
        ] {
            let package = member(name, directory, path, "run(\"--exact\");");
            assert_eq!(check(&[package]).is_ok(), exempt, "{name}: {path}");
        }
    }
}

/// Byte offsets retain Unicode, blank lines and the shebang before the first violation.
#[test]
fn exact_child_selection_reports_source_line() {
    let source = "#!/usr/bin/env rustx\n// café 🦀\n\nrun(r#\"--exact\"#);\nrun(\"--exact\");";
    let package = member("maestro-models", "models", "models/src/child.rs", source);
    assert_eq!(
        check(&[package]),
        Err("models/src/child.rs:4: exact test selection belongs to maestro-test-process".into())
    );
    assert_eq!(exact_offset(source), Some(source.find("r#").unwrap()));
}
