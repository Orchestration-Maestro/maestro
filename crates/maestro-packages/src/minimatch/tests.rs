//! Native framing and the ordered brace adapter.
use super::{Platform, brace_expand, match_platform};
use serde::Deserialize;

/// Exercises windows roots keep drive and unc identity.
#[test]
fn windows_roots_keep_drive_and_unc_identity() {
    let cases = [
        (r"c:/a", r"C:/a", true),
        (r"c:/a", r"C\:/a", true),
        (r"c:/a", r"//\?/C:/a", true),
        (r"//x/C:/a", r"//?/C:/a", false),
        (r"c:/a", r"C:/A", false),
        (r"D:/a", r"C:/a", false),
        (r"c:/a", r"//?/C:/a", true),
        (r"//?/C:/a", r"c:/a", true),
        (r"/server/share/a", r"//server/share/a", false),
        (r"//server/share/a", r"//server/share/a", true),
        (r"//other/share/a", r"//server/share/a", false),
        (r"a\b", r"a/b", true),
        (r"a/b", r"a\b", false),
        (r"//server/share/a", r"///server/share/a", false),
    ];
    for (index, (path, pattern, expected)) in cases.into_iter().enumerate() {
        assert_eq!(
            match_platform(path, pattern, Platform::Windows),
            Ok(expected),
            "case {index}"
        );
    }
}

/// Exercises posix roots do not adopt windows identity.
#[test]
fn posix_roots_do_not_adopt_windows_identity() {
    let cases = [
        (r"c:/a", r"C:/a", false),
        (r"c:/a", r"//?/C:/a", false),
        (r"/server/share/a", r"//server/share/a", true),
        (r"a\b", r"a/b", false),
    ];
    for (index, (path, pattern, expected)) in cases.into_iter().enumerate() {
        assert_eq!(
            match_platform(path, pattern, Platform::Posix),
            Ok(expected),
            "case {index}"
        );
    }
}

/// One ordered brace expansion.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expansion {
    /// Input pattern.
    pattern: String,
    /// Ordered alternatives including duplicates.
    expected: Vec<String>,
}

/// Exercises ordered expansions through the same adapter as matching.
#[test]
fn brace_arrays_keep_expected_order() {
    let cases: Vec<Expansion> = serde_json::from_str(include_str!("brace_vectors.json")).unwrap();
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(brace_expand(&case.pattern), case.expected, "case {index}");
    }
}
