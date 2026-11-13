use super::{Reason, Row, check_with};
use crate::source::Member;

/// The diagnostics of an example member that holds the given sources, under the given table.
fn diagnostics(sources: &[(&str, &str)], exempt: &[Row]) -> String {
    let member = Member::with_sources(
        "maestro-example",
        "/workspace/crates/example",
        sources.iter().map(|(path, contents)| {
            (
                format!("/workspace/crates/example/{path}"),
                (*contents).to_owned(),
            )
        }),
    );
    check_with(&[member], exempt, true)
        .err()
        .unwrap_or_default()
}

/// A library that declares the module `utils`, with the module's file.
const UTILS: &[(&str, &str)] = &[("src/lib.rs", "mod utils;\n"), ("src/utils.rs", "")];

#[test]
fn an_exempt_row_covers_its_module_and_no_other() {
    let row = ("maestro-example", "utils", Reason::Mirrored);
    assert_eq!(diagnostics(UTILS, &[row]), "");

    let other_module = ("maestro-example", "parsing::utils", Reason::Mirrored);
    let other_member = ("maestro-other", "utils", Reason::Mirrored);
    for row in [other_module, other_member] {
        let error = diagnostics(UTILS, &[row]);
        assert!(
            error.contains("src/lib.rs:1: module `utils` of maestro-example"),
            "{error}"
        );
        assert!(error.contains("matches no module"), "{error}");
    }
}

#[test]
fn a_row_that_matches_no_module_is_reported_with_its_reason() {
    assert_eq!(
        diagnostics(
            &[("src/lib.rs", "mod parsing;\n")],
            &[("maestro-example", "utils", Reason::Unnamed)]
        ),
        "exemption for module `utils` of maestro-example (Maestro's own module, still to be named for what it holds) matches no module; remove the row from EXEMPT in bucket_modules.rs"
    );
}

#[test]
fn a_tree_that_holds_part_of_the_workspace_reports_no_unmatched_row() {
    let member = Member::with_sources(
        "maestro-example",
        "/workspace/crates/example",
        [("/workspace/crates/example/src/lib.rs", "mod utils;\n")],
    );
    let rows = [
        ("maestro-example", "utils", Reason::Mirrored),
        ("maestro-other", "types", Reason::Mirrored),
    ];
    assert_eq!(check_with(&[member], &rows, false), Ok(()));
}

#[test]
fn rows_out_of_order_or_repeated_are_reported() {
    let sources = [("src/lib.rs", "mod types;\nmod utils;\n")];
    let types = ("maestro-example", "types", Reason::Mirrored);
    let utils = ("maestro-example", "utils", Reason::Mirrored);
    assert_eq!(diagnostics(&sources, &[types, utils]), "");

    for (rows, moved) in [([utils, types], "types"), ([utils, utils], "utils")] {
        assert_eq!(
            diagnostics(&sources, &rows),
            format!(
                "EXEMPT in bucket_modules.rs is sorted by member and module path, each row once: move the row for module `{moved}` of maestro-example"
            )
        );
    }
}

#[test]
fn a_module_is_keyed_by_its_path_below_src() {
    let sources = [
        ("src/lib.rs", "mod parsing {\n    mod helpers {}\n}\n"),
        ("src/records/shared.rs", "mod common;\n"),
        ("src/records/types/mod.rs", "mod misc;\n"),
        ("src/bin/report/util.rs", ""),
        ("tests/support/utils.rs", ""),
    ];
    let rows = [
        "bin::report::util",
        "parsing::helpers",
        "records::shared",
        "records::shared::common",
        "records::types",
        "records::types::misc",
    ]
    .map(|module| ("maestro-example", module, Reason::Mirrored));
    assert_eq!(diagnostics(&sources, &rows), "");

    let places: Vec<String> = diagnostics(&sources, &[])
        .lines()
        .map(|line| {
            line.split(" of maestro-example ")
                .next()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(
        places,
        [
            "/workspace/crates/example/src/bin/report/util.rs: module `bin::report::util`",
            "/workspace/crates/example/src/lib.rs:2: module `parsing::helpers`",
            "/workspace/crates/example/src/records/shared.rs: module `records::shared`",
            "/workspace/crates/example/src/records/shared.rs:1: module `records::shared::common`",
            "/workspace/crates/example/src/records/types: module `records::types`",
            "/workspace/crates/example/src/records/types/mod.rs:1: module `records::types::misc`",
        ]
    );
}
