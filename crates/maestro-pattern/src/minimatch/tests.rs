//! Native framing and the ordered brace adapter.
use super::{Platform, brace_expand, match_platform};
use serde::Deserialize;

/// Exercises windows roots keep drive and unc identity.
#[test]
fn windows_roots_keep_drive_and_unc_identity() {
    let cases = [
        ("/a", "//a", false),
        ("//a", "/a", false),
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
        (r"a\b", r"a\\b", false),
        (r"a\", r"a\", false),
        (r"x\", r"*\", false),
        (r"b\a", r"*\a", false),
        (r"a\.ts", r"*\.ts", false),
        (r"//server/share/a", r"///server/share/a", false),
    ];
    for (index, (path, pattern, expected)) in cases.into_iter().enumerate() {
        assert_eq!(
            match_platform(path, pattern, Platform::Windows, false),
            Ok(expected),
            "case {index}"
        );
    }
}

/// Exercises posix roots do not adopt windows identity.
#[test]
fn posix_roots_do_not_adopt_windows_identity() {
    let cases = [
        ("/a", "//a", true),
        ("//a", "/a", true),
        (r"c:/a", r"C:/a", false),
        (r"c:/a", r"//?/C:/a", false),
        (r"/server/share/a", r"//server/share/a", true),
        (r"a\b", r"a/b", false),
        (r"a\b", r"a\\b", true),
        (r"a\", r"a\", true),
        (r"x\", r"*\", true),
        (r"b\a", r"*\a", true),
        (r"a\.ts", r"*\.ts", true),
    ];
    for (index, (path, pattern, expected)) in cases.into_iter().enumerate() {
        assert_eq!(
            match_platform(path, pattern, Platform::Posix, false),
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
        assert_eq!(
            brace_expand(&case.pattern, None),
            case.expected,
            "case {index}"
        );
    }
}

/// Class literals retain native drive identity after separator framing.
#[test]
fn class_platform_framing_precedes_scalar_matching() {
    let groups: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
        serde_json::from_str(include_str!("../../tests/pattern_class_vectors.json")).unwrap();
    let rows = groups
        .get("class_platform_framing_precedes_scalar_matching")
        .unwrap();
    assert!(!rows.is_empty());
    for (index, row) in rows.iter().enumerate() {
        let (pattern, input, posix, windows): (String, String, bool, bool) =
            serde_json::from_value(row.clone()).unwrap();
        assert_eq!(
            match_platform(&input, &pattern, Platform::Posix, false),
            Ok(posix),
            "posix case {index}"
        );
        assert_eq!(
            match_platform(&input, &pattern, Platform::Windows, false),
            Ok(windows),
            "windows case {index}"
        );
        let native = if cfg!(windows) { windows } else { posix };
        assert_eq!(
            super::minimatch(&input, &pattern),
            Ok(native),
            "native case {index}"
        );
    }
}

/// Native framing resolves before group recognition on both policies.
#[test]
fn group_platform_framing_precedes_matching() {
    let groups: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
        serde_json::from_str(include_str!("../../tests/pattern_group_vectors.json")).unwrap();
    let rows = &groups["group_platform_framing_precedes_matching"];
    assert_eq!(rows.len(), 122);
    for (index, row) in rows.iter().enumerate() {
        let (pattern, input, posix, windows): (String, String, bool, bool) =
            serde_json::from_value(row.clone()).unwrap();
        assert_eq!(
            match_platform(&input, &pattern, Platform::Posix, false),
            Ok(posix),
            "posix case {index}"
        );
        assert_eq!(
            match_platform(&input, &pattern, Platform::Windows, false),
            Ok(windows),
            "windows case {index}"
        );
        let expected = if cfg!(windows) { windows } else { posix };
        assert_eq!(
            super::minimatch(&input, &pattern),
            Ok(expected),
            "native case {index}"
        );
    }
}

/// One recorded observation of the case-insensitive corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NocaseRow {
    /// Platform whose framing produced the expectation.
    platform: String,
    /// Whether case-insensitive matching was selected.
    nocase: bool,
    /// Authored pattern.
    pattern: String,
    /// Operand path.
    input: String,
    /// Expected match result.
    expected: bool,
}

/// Replays every Windows-policy row through the framing the public operation uses.
#[test]
fn nocase_policy_keeps_injected_windows_framing() {
    let groups: std::collections::BTreeMap<String, Vec<NocaseRow>> =
        serde_json::from_str(include_str!("../../tests/fixtures/nocase_vectors.json")).unwrap();
    let mut replayed = 0;
    for (group, rows) in &groups {
        for (index, row) in rows.iter().enumerate() {
            let platform = match row.platform.as_str() {
                "win32" => Platform::Windows,
                _ => continue,
            };
            assert_eq!(
                match_platform(&row.input, &row.pattern, platform, row.nocase),
                Ok(row.expected),
                "{group} row {index}"
            );
            replayed += 1;
        }
    }
    assert_eq!(
        replayed,
        groups
            .values()
            .flatten()
            .filter(|r| r.platform == "win32")
            .count()
    );
    assert!(replayed > 0);
}
