use super::{Exemption, check_with, denied_words, segments};
use crate::source::Member;
use std::path::Path;

/// A member holding one source file with the given contents.
fn member(contents: &str) -> Member {
    Member::with_sources(
        "maestro-example",
        "/workspace/crates/example",
        [("/workspace/crates/example/src/lib.rs", contents)],
    )
}

#[test]
fn names_split_at_separators_and_case_changes() {
    assert_eq!(segments("ssf_load-table"), ["ssf", "load", "table"]);
    assert_eq!(segments("SsfLoadTable"), ["ssf", "load", "table"]);
    assert_eq!(segments("HTMLParser"), ["html", "parser"]);
    assert_eq!(segments("docx.preview/x"), ["docx", "preview", "x"]);
}

#[test]
fn a_denied_word_matches_only_whole_segments() {
    assert_eq!(denied_words("parse_ssf_value"), ["ssf"]);
    assert_eq!(denied_words("Ssf"), ["ssf"]);
    assert_eq!(denied_words("DocxPreview"), ["docx_preview"]);
    assert_eq!(denied_words("docx_preview_browser"), ["docx_preview"]);
    assert!(denied_words("successful").is_empty());
    assert!(denied_words("plugins").is_empty());
    assert!(denied_words("docx_body").is_empty());
}

#[test]
fn an_exemption_that_matches_a_name_hides_it() {
    let exempt = [Exemption {
        path: "crates/example/src/lib.rs",
        word: "ssf",
        reason: "a stored file carries the name",
    }];
    let members = [member("fn ssf_value() {}\n")];
    assert_eq!(
        check_with(Path::new("/workspace"), &members, &exempt),
        Ok(())
    );
}

#[test]
fn an_exemption_that_matches_nothing_is_reported() {
    let exempt = [Exemption {
        path: "crates/example/src/lib.rs",
        word: "ssf",
        reason: "a stored file carries the name",
    }];
    let members = [member("fn value() {}\n")];
    let error = check_with(Path::new("/workspace"), &members, &exempt).unwrap_err();
    assert!(
        error.contains("exemption for `crates/example/src/lib.rs` and `ssf`")
            && error.contains("remove the row from EXEMPT"),
        "{error}"
    );
}
