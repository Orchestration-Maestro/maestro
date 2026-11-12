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

/// Assert one diagnostic reports the shared body and both authored sites on every host.
fn assert_pair(error: &str, first: &str, second: &str) {
    assert!(
        error.starts_with("exact duplicate body (30 tokens): "),
        "{error}"
    );
    assert!(error.contains(&support::native_path_text(first)), "{error}");
    assert!(
        error.contains(&support::native_path_text(second)),
        "{error}"
    );
}

/// Build two authored functions with the same interior.
fn copied(body: &str) -> String {
    format!("fn first() {{{body}}}\nfn second() {{{body}}}")
}

/// Spell the same source through the ordinary, test-file and inline-test layouts.
fn layouts(source: &str) -> Vec<(&'static str, String)> {
    vec![
        ("src/lib.rs", source.to_owned()),
        ("tests/case.rs", source.to_owned()),
        (
            "src/lib.rs",
            format!("#[cfg(test)]\nmod tests {{\n{source}}}\n"),
        ),
    ]
}

#[test]
fn threshold_counts_only_interior_tokens() {
    for n in [0, 1, 29, 30, 31] {
        for (path, source) in layouts(&copied(&";".repeat(n))) {
            let result = check(&[member("maestro-settings", &[(path, source)])]);
            assert_eq!(result.is_err(), n >= 30, "{n} {path}");
            if n >= 30 {
                assert!(result.unwrap_err().contains(&format!("({n} tokens)")));
            }
        }
    }
}

#[test]
fn whitespace_and_comments_preserve_body_identity() {
    let a = "foo(value);".repeat(6);
    let b = "foo /* outer /* inner */ outer */ ( value ) ; // tail\n".repeat(6);
    let source = format!("fn first() {{{a}}}\nfn second() {{{b}}}");
    for (path, source) in layouts(&source) {
        assert!(
            check(&[member("maestro-settings", &[(path, source)])]).is_err(),
            "{path}"
        );
    }
}

/// Spellings that each change one token kind of `foo(value);`: identifier, argument, raw
/// identifier, number, delimiter, operator, comment-like and raw literal, byte literal,
/// suffixed number and nested group.
const UNEQUAL_SPELLINGS: [&str; 13] = [
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
    "foo(1u8);",
    "foo((1));",
];

#[test]
fn token_spelling_and_delimiters_remain_distinct() {
    ordinary_unequal_pairs_stay_accepted();
    laid_out_unequal_pairs_stay_accepted();
    unequal_literal_arguments_stay_accepted();
    lookalike_literals_stay_duplicates();
}

/// Every changed spelling keeps its identical original accepted in ordinary source.
fn ordinary_unequal_pairs_stay_accepted() {
    let original = "foo(value);".repeat(6);
    for change in UNEQUAL_SPELLINGS {
        let source = format!(
            "fn first() {{{original}}}\nfn second() {{{}}}",
            change.repeat(6)
        );
        assert!(
            check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_ok(),
            "{change}"
        );
    }
}

/// The same unequal pairs stay accepted, one workspace per layout, when the only
/// spelling places them in test code. Each pair gets its own argument name, so no
/// body of one pair equals any body of another pair inside one workspace.
fn laid_out_unequal_pairs_stay_accepted() {
    for inline in [false, true] {
        let source: String = UNEQUAL_SPELLINGS
            .iter()
            .enumerate()
            .map(|(index, change)| {
                let name = format!("tag{index}");
                let original = "foo(tagX);".repeat(6).replace("tagX", &name);
                let spelled = change.repeat(6).replace("value", &name);
                format!(
                    "fn pair_{index}_first() {{{original}}}\nfn pair_{index}_second() {{{spelled}}}\n"
                )
            })
            .collect::<Vec<_>>()
            .concat();
        let (path, source) = if inline {
            (
                "src/lib.rs",
                format!("#[cfg(test)]\nmod tests {{\n{source}}}\n"),
            )
        } else {
            ("tests/case.rs", source)
        };
        assert!(
            check(&[member("maestro-settings", &[(path, source)])]).is_ok(),
            "unequal pairs stay accepted in {path}"
        );
    }
}

/// Literal spellings that differ between the two argument pairs stay accepted.
fn unequal_literal_arguments_stay_accepted() {
    for (index, (a, b)) in [
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
    ]
    .into_iter()
    .enumerate()
    {
        let source = format!(
            "fn field_{index}_first() {{{}}} fn field_{index}_second() {{{}}}",
            format!("foo({a});").repeat(6),
            format!("foo({b});").repeat(6)
        );
        assert!(
            check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_ok(),
            "{a}/{b}"
        );
    }
}

/// Identical copies carrying comment-like literal text stay duplicates in every layout.
fn lookalike_literals_stay_duplicates() {
    let source = copied(&"foo(\"// text\");foo(r#\"/* text */\"#);foo('é');foo(1u8);".repeat(2));
    assert!(check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_err());
    for (path, source) in layouts(&copied(
        &"foo(\"// text\");foo(r#\"a\"#);foo('é');foo(1u8);".repeat(2),
    )) {
        let error = check(&[member("maestro-settings", &[(path, source)])]).unwrap_err();
        assert!(
            error.starts_with("exact duplicate body")
                && error.contains(&support::native_path_text(path)),
            "{path} {error}"
        );
    }
}

#[test]
fn signatures_and_bindings_do_not_exempt_exact_bodies() {
    let body = "foo(value);".repeat(6);
    let source = format!(
        "mod a {{ fn foo() {{}} fn first() {{{body}}} }} mod b {{ fn foo(_:u8) {{}} pub fn second<T>(_: T) {{{body}}} }}"
    );
    let error = check(&[member("maestro-settings", &[("src/lib.rs", source)])]).unwrap_err();
    assert!(error.contains("30 tokens"));
    let source = format!(
        "#[cfg(test)] fn first() {{{body}}} #[cfg(test)] pub fn second<T>(_: T) {{{body}}}"
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
    let body = "let _ = 1;".repeat(6);
    let source = format!(
        "#[cfg(test)]\nmod tests {{\nfn free() {{{body}}}\nstruct X;\nimpl X {{\n fn method() {{{body}}}\n}}\ntrait T {{\n fn declaration();\n fn defaulted() {{{body}}}\n}}\nfn outer() {{\n fn nested() {{{body}}}\n}}\n}}"
    );
    let error = check(&[member("maestro-settings", &[("src/lib.rs", source)])]).unwrap_err();
    let owned = [3, 6, 10, 13].map(|line| format!("crates/maestro-settings/src/lib.rs:{line}"));
    assert_eq!(
        error,
        format!(
            "exact duplicate body (30 tokens): {}",
            owned.join(", ").replace('/', std::path::MAIN_SEPARATOR_STR)
        )
    );
}

#[test]
fn copied_test_helper_fails_until_callers_delegate() {
    let body = "let _ = 1;".repeat(6);
    // One seed puts the shared owner beside a copy in a test file.
    let error = check(&[member(
        "maestro-settings",
        &[
            ("src/lib.rs", format!("pub fn shared() {{{body}}}")),
            ("tests/case.rs", format!("pub fn imitation() {{{body}}}")),
        ],
    )])
    .unwrap_err();
    assert_eq!(
        error,
        "exact duplicate body (30 tokens): crates/maestro-settings/src/lib.rs:1, crates/maestro-settings/tests/case.rs:1"
            .replace('/', std::path::MAIN_SEPARATOR_STR)
    );
    assert_eq!(
        check(&[member(
            "maestro-settings",
            &[
                ("src/lib.rs", format!("pub fn shared() {{{body}}}")),
                ("tests/case.rs", "pub fn imitation() { shared(); }".into()),
            ],
        )]),
        Ok(())
    );
    // One seed puts the shared owner beside a copy in an inline test module.
    let seeded = format!(
        "pub fn shared() {{{body}}}\n#[cfg(test)]\nmod tests {{\npub fn imitation() {{{body}}}\n}}"
    );
    let error = check(&[member("maestro-settings", &[("src/lib.rs", seeded)])]).unwrap_err();
    assert_eq!(
        error,
        "exact duplicate body (30 tokens): crates/maestro-settings/src/lib.rs:1, crates/maestro-settings/src/lib.rs:4"
            .replace('/', std::path::MAIN_SEPARATOR_STR)
    );
    let delegated = format!(
        "pub fn shared() {{{body}}}\n#[cfg(test)]\nmod tests {{\npub fn imitation() {{ shared(); }}\n}}"
    );
    assert_eq!(
        check(&[member("maestro-settings", &[("src/lib.rs", delegated)])]),
        Ok(())
    );
}

#[test]
fn test_bodies_compare_across_workspace_packages() {
    let body = "let _ = 1;".repeat(6);
    let error = check(&[
        member(
            "maestro-json",
            &[("tests/case.rs", format!("pub fn imitation() {{{body}}}"))],
        ),
        member(
            "maestro-settings",
            &[("tests/case.rs", format!("pub fn imitation() {{{body}}}"))],
        ),
    ])
    .unwrap_err();
    assert_pair(
        &error,
        "crates/maestro-json/tests/case.rs:1",
        "crates/maestro-settings/tests/case.rs:1",
    );
    // One changed body plus a shared owner with delegating callers stays accepted.
    assert_eq!(
        check(&[
            member(
                "maestro-json",
                &[
                    ("src/lib.rs", format!("pub fn shared() {{{body}}}")),
                    ("tests/case.rs", "pub fn user() { shared(); }".into()),
                ],
            ),
            member(
                "maestro-settings",
                &[("src/others.rs", "pub fn unrelated() { let _ = 2; }".into())],
            ),
        ]),
        Ok(())
    );
}

#[test]
fn test_and_production_bodies_share_one_identity_index() {
    production_beside_test_file();
    production_beside_inline_test_code();
    support_beside_another_package_test();
}

/// Ordinary production code beside a package's test file.
fn production_beside_test_file() {
    let body = "let _ = 1;".repeat(6);
    let error = check(&[
        member(
            "maestro-json",
            &[("src/lib.rs", format!("pub fn pinned() {{{body}}}"))],
        ),
        member(
            "maestro-models",
            &[("tests/case.rs", format!("pub fn reproduction() {{{body}}}"))],
        ),
    ])
    .unwrap_err();
    assert_pair(
        &error,
        "crates/maestro-json/src/lib.rs:1",
        "crates/maestro-models/tests/case.rs:1",
    );
}

/// Ordinary production code beside inline test code, with the packages reversed.
fn production_beside_inline_test_code() {
    let body = "let _ = 1;".repeat(6);
    let error = check(&[
        member(
            "maestro-json",
            &[(
                "src/own.rs",
                format!("#[cfg(test)]\npub fn gated() {{{body}}}"),
            )],
        ),
        member(
            "maestro-models",
            &[("src/ordinary.rs", format!("pub fn common() {{{body}}}"))],
        ),
    ])
    .unwrap_err();
    assert_pair(
        &error,
        "crates/maestro-json/src/own.rs:2",
        "crates/maestro-models/src/ordinary.rs:1",
    );
}

/// A test-support library beside another package's test, then reversed.
fn support_beside_another_package_test() {
    let body = "let _ = 1;".repeat(6);
    for (support_member, test_member) in [
        ("maestro-test-fixtures", "maestro-models"),
        ("maestro-models", "maestro-test-fixtures"),
    ] {
        let error = check(&[
            member(
                support_member,
                &[("src/lib.rs", format!("pub fn helper() {{{body}}}"))],
            ),
            member(
                test_member,
                &[("tests/case.rs", format!("pub fn reproduction() {{{body}}}"))],
            ),
        ])
        .unwrap_err();
        assert_pair(
            &error,
            &format!("crates/{support_member}/src/lib.rs:1"),
            &format!("crates/{test_member}/tests/case.rs:1"),
        );
    }
}

#[test]
fn parsed_test_scopes_are_checked_without_hiding_neighbours() {
    let body = "foo(value);".repeat(6);
    let scopes = [
        format!("#[cfg(test)] fn hidden() {{{body}}}"),
        format!("#[cfg(test)] mod tests {{ fn hidden() {{{body}}} }}"),
        format!("#[cfg(test)] impl X {{ fn hidden() {{{body}}} }}"),
        format!("impl X {{ #[cfg(test)] fn hidden() {{{body}}} }}"),
        format!("#[cfg(test)] trait T {{ fn hidden() {{{body}}} }}"),
        format!("trait T {{ #[cfg(test)] fn hidden() {{{body}}} }}"),
        format!("impl X {{ #[cfg(test)] const C: () = {{ fn hidden() {{{body}}} }}; }}"),
        format!("trait T {{ #[cfg(test)] const C: () = {{ fn hidden() {{{body}}} }}; }}"),
    ];
    for scope in scopes {
        let source = format!("fn visible() {{{body}}} {scope}");
        assert!(
            check(&[member(
                "maestro-settings",
                &[("src/lib.rs", source.clone())]
            )])
            .is_err(),
            "the test-owned copy is checked beside production syntax: {scope}"
        );
        let following = format!("{scope} fn neighbour() {{{body}}}");
        assert!(
            check(&[member("maestro-settings", &[("src/lib.rs", following)])]).is_err(),
            "an ordinary neighbour after a test-owned item stays checked: {scope}"
        );
    }
    let source = format!(
        "fn first() {{ let _ = \"#[cfg(test)]\"; /* #[cfg(test)] */ {body} }} fn second() {{ let _ = \"#[cfg(test)]\"; /* #[cfg(test)] */ {body} }}"
    );
    assert!(check(&[member("maestro-settings", &[("src/lib.rs", source)])]).is_err());
}

#[test]
fn test_layout_and_file_attributes_are_checked() {
    let body = copied(&";".repeat(30));
    for path in ["src/tests.rs", "src/tests/case.rs", "tests/case.rs"] {
        let result = check(&[member("maestro-settings", &[(path, body.clone())])]);
        assert!(
            result.unwrap_err().starts_with("exact duplicate body"),
            "{path}"
        );
    }
    assert!(
        check(&[member(
            "maestro-settings",
            &[("examples/test.rs", format!("#![cfg(test)]\n{body}"))]
        )])
        .unwrap_err()
        .starts_with("exact duplicate body"),
        "a file-level test attribute stays parsed"
    );
    assert!(
        check(&[member(
            "maestro-settings",
            &[("examples/example.rs", body.clone())]
        )])
        .unwrap_err()
        .starts_with("exact duplicate body")
    );
    assert!(
        check(&[member(
            "maestro-settings",
            &[(
                "src/lib.rs",
                format!("#[cfg(any(test, feature = \"x\"))]\n{body}")
            )]
        )])
        .unwrap_err()
        .starts_with("exact duplicate body"),
        "compound configurations stay parsed"
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
    // A generated owner stays excluded while its bodies carry test attributes.
    for (name, path) in [
        ("maestro-models", "src/catalog/models_generated/records.rs"),
        ("maestro-extensions-wasm", "src/bindings.rs"),
    ] {
        let body = format!("#[cfg(test)]\n{body}");
        assert!(check(&[member(name, &[(path, body)])]).is_ok());
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
    for path in ["src/lib.rs", "tests/case.rs"] {
        let source = format!(
            "fn visible() {{{body}}} macro_rules! emitted {{ () => {{ fn hidden() {{{body}}} }} }} emitted!();"
        );
        assert!(
            check(&[member("maestro-settings", &[(path, source)])]).is_ok(),
            "{path}"
        );
        let source = copied(&"invoke!({value});".repeat(6));
        assert!(
            check(&[member("maestro-settings", &[(path, source)])]).is_err(),
            "{path}"
        );
    }
}

#[test]
fn unparsed_fragments_do_not_create_function_candidates() {
    let ordinary_source = format!("{{ {} }}", copied(&";".repeat(30)));
    let ordinary = member(
        "maestro-settings",
        &[("src/fragment.rs", ordinary_source.clone())],
    );
    assert!(syn::parse_file(&ordinary.files[0].1).is_err());
    assert!(check(&[ordinary]).is_ok());
    let owned = member(
        "maestro-settings",
        &[("tests/case.rs", ordinary_source.clone())],
    );
    assert!(check(&[owned]).is_ok());
    let both = member(
        "maestro-settings",
        &[
            ("tests/case.rs", ordinary_source.clone()),
            ("src/fragment.rs", ordinary_source),
        ],
    );
    assert!(check(&[both]).is_ok());
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
            ("tests/case.rs", format!("fn fifth() {{{body}}}")),
        ],
    );
    let b = member(
        "maestro-json",
        &[("src/lib.rs", format!("fn fourth() {{{body}}}"))],
    );
    let extra = member("maestro-lock", &[("src/lib.rs", copied(&";".repeat(31)))]);
    let expected = "exact duplicate body (30 tokens): crates/maestro-json/src/lib.rs:1, crates/maestro-settings/src/a.rs:1, crates/maestro-settings/src/a.rs:1, crates/maestro-settings/src/b.rs:2, crates/maestro-settings/tests/case.rs:1\nexact duplicate body (31 tokens): crates/maestro-lock/src/lib.rs:1, crates/maestro-lock/src/lib.rs:2";
    assert_eq!(
        check(&[a, b, extra]),
        Err(expected.replace('/', std::path::MAIN_SEPARATOR_STR))
    );
}

#[test]
fn normalized_sources_keep_duplicate_locations() {
    let workspace = support::Workspace::new();
    workspace.foundation(&["maestro-settings"]);
    workspace.library_test_target("maestro-settings", true);
    let ordinary = workspace
        .root
        .path()
        .join(support::native_path("crates/maestro-settings/src/lib.rs"));
    let owned = workspace
        .root
        .path()
        .join(support::native_path("crates/maestro-settings/src/tests.rs"));
    let body = "let _ = 1;".repeat(6);
    std::fs::write(
        &ordinary,
        format!("\u{feff}//! Header.\r\nfn first() {{{body}}}\r\nfn second() {{{body}}}\r\n"),
    )
    .unwrap();
    std::fs::write(
        &owned,
        format!(
            "\u{feff}//! Suite.\r\n#[cfg(test)]\r\nmod tests {{\r\npub fn first() {{{body}}}\r\npub fn second() {{{body}}}\r\n}}\r\n"
        ),
    )
    .unwrap();
    let expected = format!(
        "exact duplicate body (30 tokens): {}:2, {}:3, {}:4, {}:5",
        ordinary.display(),
        ordinary.display(),
        owned.display(),
        owned.display()
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
    let body = "let _ = 1;".repeat(6);
    let source = format!("#!/usr/bin/env rustx\n{}", copied(&body));
    let error = check(&[member("maestro-settings", &[("tests/case.rs", source)])]).unwrap_err();
    assert_eq!(
        error,
        "exact duplicate body (30 tokens): crates/maestro-settings/tests/case.rs:2, crates/maestro-settings/tests/case.rs:3"
            .replace('/', std::path::MAIN_SEPARATOR_STR)
    );
    let source = format!(
        "#!/usr/bin/env rustx\nfn owner() {{{body}}} fn first() {{owner();}} fn second() {{owner();}}"
    );
    assert_eq!(
        check(&[member("maestro-settings", &[("tests/case.rs", source)])]),
        Ok(())
    );
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
