use maestro_test_conventions::check_workspace;

use crate::support;

/// Authored files for one disposable workspace member.
struct Fixture {
    /// Foundation owner name.
    name: &'static str,
    /// Relative file names and their authored contents.
    files: Vec<(&'static str, String)>,
}

/// Describe one member without interpreting its source.
fn member(name: &'static str, files: &[(&'static str, String)]) -> Fixture {
    Fixture {
        name,
        files: files.to_vec(),
    }
}

/// Exercise the public checker, removing only the disposable root from diagnostics.
fn check(fixtures: &[Fixture]) -> Result<(), String> {
    let workspace = support::Workspace::new();
    workspace.foundation(
        &fixtures
            .iter()
            .map(|fixture| fixture.name)
            .collect::<Vec<_>>(),
    );
    for fixture in fixtures {
        for (path, contents) in &fixture.files {
            support::source(&workspace, fixture.name, path, contents);
        }
        if fixture
            .files
            .iter()
            .any(|(path, _)| path.starts_with("tests/"))
        {
            workspace.integration(fixture.name);
        }
    }
    check_workspace(workspace.root.path()).map_err(|error| {
        error.replace(
            &format!(
                "{}{sep}",
                workspace.root.path().display(),
                sep = std::path::MAIN_SEPARATOR
            ),
            "",
        )
    })
}

/// Build two authored functions with the same interior.
fn copied(body: &str) -> String {
    format!("fn first() {{{body}}}\nfn second() {{{body}}}")
}

#[test]
fn threshold_counts_only_interior_tokens() {
    for n in [0, 1, 29, 30, 31] {
        let source = copied(&";".repeat(n));
        let result = check(&[member("maestro-settings", &[("src/lib.rs", source)])]);
        assert_eq!(result.is_err(), n >= 30, "{n}");
        if n >= 30 {
            assert!(result.unwrap_err().contains(&format!("({n} tokens)")));
        }
    }
}

#[test]
fn whitespace_and_comments_preserve_body_identity() {
    let a = "foo(value);".repeat(6);
    let b = "foo /* outer /* inner */ outer */ ( value ) ; // tail\n".repeat(6);
    let source = format!("fn first() {{{a}}}\nfn second() {{{b}}}");
    assert!(check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_err());
}

#[test]
fn token_spelling_and_delimiters_remain_distinct() {
    let original = "foo(value);".repeat(6);
    for change in [
        "bar(value);",
        "foo(other);",
        "foo(r#value);",
        "foo(1);",
        "foo[ value ];",
        "foo{value};",
        "foo(value)?;",
        "foo(\"// text\");",
        "foo(\"/* text */\");",
        "foo(b\"a\");",
        "foo(r#\"a\"#);",
    ] {
        let source = format!(
            "fn first() {{{original}}}\nfn second() {{{}}}",
            change.repeat(6)
        );
        assert!(
            check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_ok(),
            "{change}"
        );
    }
    for (a, b) in [
        ("1", "2"),
        ("1u8", "1"),
        ("1_000", "1000"),
        ("value + 1", "value - 1"),
        ("\"a b\"", "\"ab\""),
        ("r#\"a\"#", "r#\"b\"#"),
        ("r#\"/* x */\"#", "r#\"/*x*/\"#"),
        ("b\"a\"", "b\"b\""),
        ("b\"// x\"", "b\"//x\""),
        ("'é'", "'x'"),
    ] {
        let source = format!(
            "fn first() {{{}}} fn second() {{{}}}",
            format!("foo({a});").repeat(6),
            format!("foo({b});").repeat(6)
        );
        assert!(
            check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_ok(),
            "{a}/{b}"
        );
    }
    let source = copied(&"foo(\"// text\");foo(r#\"/* text */\"#);foo('é');foo(1u8);".repeat(2));
    assert!(check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_err());
}

#[test]
fn signatures_and_bindings_do_not_exempt_exact_bodies() {
    let body = "foo(value);".repeat(6);
    let source = format!(
        "mod a {{ fn foo() {{}} fn first() {{{body}}} }} mod b {{ fn foo(_:u8) {{}} pub fn second<T>(_: T) {{{body}}} }}"
    );
    let error = check(&[member("maestro-settings", &[("src/lib.rs", source)])]).unwrap_err();
    assert!(error.contains("30 tokens"));
}

#[test]
fn public_check_rejects_copied_body_and_accepts_shared_owner() {
    let workspace = support::Workspace::new();
    workspace.foundation(&["maestro-settings"]);
    let path = workspace
        .root
        .path()
        .join("crates/maestro-settings/src/lib.rs");
    std::fs::write(&path, copied(&"let _ = 1;".repeat(6))).unwrap();
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.starts_with("exact duplicate body (30 tokens): "),
        "{error}"
    );
    std::fs::write(
        &path,
        format!(
            "fn owner() {{{}}} fn first() {{owner();}} fn second() {{owner();}}",
            "let _ = 1;".repeat(6)
        ),
    )
    .unwrap();
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn free_associated_default_and_nested_bodies_are_checked() {
    let body = "foo(value);".repeat(6);
    let source = format!(
        "fn free() {{{body}}}\nstruct X;\nimpl X {{\n fn method() {{{body}}}\n}}\ntrait T {{\n fn declaration();\n fn defaulted() {{{body}}}\n}}\nfn outer() {{\n fn nested() {{{body}}}\n}}"
    );
    let error = check(&[member("maestro-settings", &[("src/lib.rs", source)])]).unwrap_err();
    let expected = [1, 4, 8, 11].map(|line| format!("crates/maestro-settings/src/lib.rs:{line}"));
    assert_eq!(
        error,
        format!(
            "exact duplicate body (30 tokens): {}",
            expected
                .join(", ")
                .replace('/', std::path::MAIN_SEPARATOR_STR)
        )
    );
}

#[test]
fn parsed_test_scopes_do_not_hide_production_neighbours() {
    let body = "foo(value);".repeat(6);
    let exclusions = [
        format!("#[cfg(test)] fn hidden() {{{body}}}"),
        format!("#[cfg(test)] mod tests {{ fn hidden() {{{body}}} }}"),
        format!("#[cfg(test)] impl X {{ fn hidden() {{{body}}} }}"),
        format!("impl X {{ #[cfg(test)] fn hidden() {{{body}}} }}"),
        format!("#[cfg(test)] trait T {{ fn hidden() {{{body}}} }}"),
        format!("trait T {{ #[cfg(test)] fn hidden() {{{body}}} }}"),
        format!("impl X {{ #[cfg(test)] const C: () = {{ fn hidden() {{{body}}} }}; }}"),
        format!("trait T {{ #[cfg(test)] const C: () = {{ fn hidden() {{{body}}} }}; }}"),
    ];
    for excluded in exclusions {
        let source = format!("fn visible() {{{body}}} {excluded}");
        assert!(
            check(&[member(
                "maestro-settings",
                &[("src/lib.rs", source.clone())]
            )])
            .is_ok()
        );
        let with_neighbour = format!("{source} fn neighbour() {{{body}}}");
        assert!(
            check(&[member(
                "maestro-settings",
                &[("src/lib.rs", with_neighbour)]
            )])
            .is_err()
        );
    }
    let source = format!(
        "fn first() {{ let _ = \"#[cfg(test)]\"; /* #[cfg(test)] */ {body} }} fn second() {{ let _ = \"#[cfg(test)]\"; /* #[cfg(test)] */ {body} }}"
    );
    assert!(check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_err());
}

#[test]
fn test_layout_and_file_attributes_are_excluded() {
    let body = copied(&";".repeat(30));
    for path in ["src/tests.rs", "src/tests/case.rs", "tests/case.rs"] {
        assert!(check(&[member("maestro-settings", &[(path, body.clone())])]).is_ok());
    }
    assert!(
        check(&[member(
            "maestro-settings",
            &[("examples/test.rs", format!("#![cfg(test)]\n{body}"))]
        )])
        .is_ok()
    );
    assert!(
        check(&[member(
            "maestro-settings",
            &[("examples/example.rs", body.clone())]
        )])
        .is_err()
    );
    assert!(
        check(&[member(
            "maestro-settings",
            &[(
                "src/lib.rs",
                format!("#[cfg(any(test, feature = \"x\"))]\n{body}")
            )]
        )])
        .is_err()
    );
}

#[test]
fn generated_exclusions_keep_exact_owner_and_path() {
    let body = copied(&";".repeat(30));
    for (name, path) in [
        ("maestro-models", "src/catalog/models_generated/records.rs"),
        ("maestro-extensions-wasm", "src/bindings.rs"),
    ] {
        assert!(check(&[member(name, &[(path, body.clone())])]).is_ok());
    }
    for (name, path) in [
        ("maestro-settings", "src/bindings.rs"),
        (
            "maestro-settings",
            "src/catalog/models_generated/records.rs",
        ),
        ("maestro-extensions-wasm", "src/handwritten.rs"),
        ("maestro-models", "src/catalog/other.rs"),
        (
            "maestro-models",
            "src/catalog/models_generated_extra/records.rs",
        ),
    ] {
        let body = if name == "maestro-extensions-wasm" {
            format!(
                "#![forbid(clippy::pedantic, clippy::too_many_arguments, clippy::excessive_nesting)]\n{body}"
            )
        } else {
            body.clone()
        };
        assert!(
            check(&[member(name, &[(path, body)])])
                .unwrap_err()
                .starts_with("exact duplicate body"),
            "{name} {path}"
        );
    }
}

#[test]
fn macro_definitions_are_not_expanded_but_invocation_bodies_are_checked() {
    let body = "foo(value);".repeat(6);
    let source = format!(
        "fn visible() {{{body}}} macro_rules! emitted {{ () => {{ fn hidden() {{{body}}} }} }} emitted!();"
    );
    assert!(check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_ok());
    let source = copied(&"invoke!({value});".repeat(6));
    assert!(check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_err());
}

#[test]
fn unparsed_fragments_do_not_create_function_candidates() {
    let source = format!("{{ {} }}", copied(&";".repeat(30)));
    let m = member("maestro-settings", &[("src/fragment.rs", source)]);
    assert!(syn::parse_file(&m.files[0].1).is_err());
    assert!(check(&[m]).is_ok());
}

#[test]
fn diagnostics_include_every_site() {
    let body = ";".repeat(30);
    let a = member(
        "maestro-settings",
        &[
            ("src/b.rs", format!("\nfn first() {{{body}}}")),
            (
                "src/a.rs",
                format!("fn second() {{{body}}} fn third() {{{body}}}"),
            ),
        ],
    );
    let b = member(
        "maestro-json",
        &[("src/lib.rs", format!("fn fourth() {{{body}}}"))],
    );
    let extra = member("maestro-lock", &[("src/lib.rs", copied(&";".repeat(31)))]);
    let expected = "exact duplicate body (30 tokens): crates/maestro-json/src/lib.rs:1, crates/maestro-settings/src/a.rs:1, crates/maestro-settings/src/a.rs:1, crates/maestro-settings/src/b.rs:2\nexact duplicate body (31 tokens): crates/maestro-lock/src/lib.rs:1, crates/maestro-lock/src/lib.rs:2";
    assert_eq!(
        check(&[a, b, extra]),
        Err(expected.replace('/', std::path::MAIN_SEPARATOR_STR))
    );
}

#[test]
fn normalized_sources_keep_duplicate_locations() {
    let workspace = support::Workspace::new();
    workspace.foundation(&["maestro-settings"]);
    let path = workspace
        .root
        .path()
        .join(support::native_path("crates/maestro-settings/src/lib.rs"));
    let body = "let _ = 1;".repeat(6);
    std::fs::write(
        &path,
        format!("\u{feff}//! Header.\r\nfn first() {{{body}}}\r\nfn second() {{{body}}}\r\n"),
    )
    .unwrap();
    let expected = format!(
        "exact duplicate body (30 tokens): {}:2, {}:3",
        path.display(),
        path.display()
    );
    assert_eq!(check_workspace(workspace.root.path()), Err(expected));
}

#[test]
fn shebang_sources_keep_duplicate_coordinates_and_shared_owner_control() {
    for prefix in ["#!/usr/bin/env rustx\n", "#![allow(dead_code)]\n"] {
        let body = "let _ = 1;".repeat(6);
        let source = format!("{prefix}{}", copied(&body));
        let error = check(&[member("maestro-settings", &[("src/lib.rs", source)])])
            .expect_err("prefixed production duplicates must be rejected");
        assert_eq!(
            error,
            "exact duplicate body (30 tokens): crates/maestro-settings/src/lib.rs:2, crates/maestro-settings/src/lib.rs:3"
                .replace('/', std::path::MAIN_SEPARATOR_STR)
        );
        let source = format!(
            "{prefix}fn owner() {{{body}}} fn first() {{owner();}} fn second() {{owner();}}"
        );
        assert_eq!(
            check(&[member("maestro-settings", &[("src/lib.rs", source)])]),
            Ok(())
        );
    }
}

#[test]
fn earlier_convention_errors_keep_precedence() {
    let workspace = support::Workspace::new();
    workspace.foundation(&["maestro-settings"]);
    let directory = workspace.root.path().join("crates/maestro-settings");
    std::fs::write(
        directory.join("src/lib.rs"),
        copied(&"let _ = 1;".repeat(6)),
    )
    .unwrap();
    let manifest = directory.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(&manifest, text.replace("[lints]\nworkspace = true\n", "")).unwrap();
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err("maestro-settings: must inherit workspace lints".into())
    );
}
