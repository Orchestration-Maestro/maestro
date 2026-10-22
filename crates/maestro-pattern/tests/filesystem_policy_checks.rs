//! Filesystem-pattern policy through the compiled matcher surface.
#![cfg(test)]
use maestro_pattern::{Minimatch, MinimatchOptions, ParseReturnFiltered, Platform, minimatch};

/// Comments and leading negation are not enabled by filesystem consumers.
#[test]
fn filesystem_policy_keeps_hash_and_bang_as_syntax() {
    preparations(&["#*.ts", "!*.ts", "src/!(a)*.ts"]);
    for (pattern, positive) in [("#*.ts", "#a.ts"), ("!*.ts", "!a.ts"), ("!(a)*.ts", "b.ts")] {
        let compiled =
            Minimatch::new(pattern, MinimatchOptions::Filesystem(Platform::Posix)).unwrap();
        let ParseReturnFiltered::Pattern(segment) = &compiled.set()[0][0] else {
            panic!("magic component must compile as a pattern");
        };
        assert!(segment.test(positive), "{pattern}: positive");
    }
}

/// One pinned preparation observation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Preparation {
    /// Case identity used in failure diagnostics.
    case: String,
    /// Authored pattern.
    pattern: String,
    /// Explicit platform policy.
    platform: String,
    /// Complete prepared result.
    expected: Prepared,
}
/// Ordered components and query results.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepared {
    /// Authored component alternatives.
    glob_parts: Vec<Vec<String>>,
    /// Compiled component alternatives.
    set: Vec<Vec<Segment>>,
}
/// One compiled component's observable behavior.
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Segment {
    /// Literal lookup operand.
    Literal { text: String },
    /// Recursive wildcard.
    Globstar {},
    /// Scalar queries through the compiled evaluator.
    Pattern { matches: Vec<(String, bool)> },
}
/// Compares every selected observation without discarding sequence or variants.
fn preparations(patterns: &[&str]) {
    let rows: Vec<Preparation> =
        serde_json::from_str(include_str!("fixtures/filesystem_patterns.json")).unwrap();
    for row in rows
        .into_iter()
        .filter(|row| patterns.contains(&row.pattern.as_str()))
    {
        let platform = match row.platform.as_str() {
            "linux" => Platform::Posix,
            "darwin" => Platform::Darwin,
            "win32" => Platform::Windows,
            _ => panic!("unexpected platform"),
        };
        let compiled =
            Minimatch::new(&row.pattern, MinimatchOptions::Filesystem(platform)).unwrap();
        assert_eq!(
            compiled.glob_parts(),
            row.expected.glob_parts,
            "{}: preparation",
            row.case
        );
        assert_eq!(
            compiled.set().len(),
            row.expected.set.len(),
            "{}: alternatives",
            row.case
        );
        for (actual, expected) in compiled.set().iter().zip(&row.expected.set) {
            assert_eq!(actual.len(), expected.len(), "{}: components", row.case);
            for (actual, expected) in actual.iter().zip(expected) {
                compare_segment(actual, expected, &row.case);
            }
        }
    }
}
/// Asserts the selected variant and each retained membership query.
fn compare_segment(actual: &ParseReturnFiltered, expected: &Segment, case: &str) {
    match (actual, expected) {
        (ParseReturnFiltered::Literal(actual), Segment::Literal { text }) => {
            assert_eq!(actual, text, "{case}: literal");
        }
        (ParseReturnFiltered::Globstar, Segment::Globstar {}) => {}
        (ParseReturnFiltered::Pattern(actual), Segment::Pattern { matches }) => {
            for (input, expected) in matches {
                assert_eq!(actual.test(input), *expected, "{case}: query {input:?}");
            }
        }
        _ => panic!("{case}: component variant"),
    }
}
/// Preparation removes covered alternatives in dependency order.
#[test]
fn filesystem_patterns_prepare_ordered_alternatives() {
    for platform in [Platform::Posix, Platform::Darwin, Platform::Windows] {
        let compiled =
            Minimatch::new("{a\n,b}*.ts", MinimatchOptions::Filesystem(platform)).unwrap();
        assert_eq!(compiled.glob_parts(), &[vec!["{a\n,b}*.ts"]]);
        let ParseReturnFiltered::Pattern(component) = &compiled.set()[0][0] else {
            panic!("expected literal brace prefix");
        };
        assert!(component.test("{a\n,b}x.ts"));
        assert!(!component.test("bx.ts"));
    }
    for (end, length, last) in [
        (9_999, 9_999, "9999*.ts"),
        (10_000, 10_000, "10000*.ts"),
        (10_001, 10_000, "10000*.ts"),
    ] {
        let pattern = format!("{{1..{end}}}*.ts");
        let compiled =
            Minimatch::new(&pattern, MinimatchOptions::Filesystem(Platform::Posix)).unwrap();
        assert_eq!(compiled.set().len(), length);
        assert_eq!(compiled.glob_parts().first().unwrap(), &["1*.ts"]);
        assert_eq!(compiled.glob_parts().last().unwrap(), &[last]);
    }
    preparations(&[
        "{a,b}/*.ts",
        "{a,a}/*.ts",
        "{a,*,b}/*.ts",
        "{*,.hidden}/*.ts",
        "{a/*,*/b}/*.ts",
        "**/**/x.ts",
        "./**/*.ts",
        "a/./b/*.ts",
        "a//b/*.ts",
        "a/../**/*.ts",
        "a/*/../*.ts",
        "a/**/../b/c/*.ts",
        "**/../x",
        "**/../../x",
        "./.",
        "./",
        "",
        "?.ts",
        "?(pre-)main.ts",
        "{3..1}?.ts",
        "{*.ts,z.ts}",
    ]);
}

/// Case equivalence follows the selected platform and class mode.
#[test]
fn filesystem_case_selection_follows_platform_policy() {
    for platform in [Platform::Posix, Platform::Darwin, Platform::Windows] {
        let compiled =
            Minimatch::new("*[![:alpha:]]", MinimatchOptions::Filesystem(platform)).unwrap();
        let ParseReturnFiltered::Pattern(component) = &compiled.set()[0][0] else {
            panic!("expected complement predicate");
        };
        assert!(
            component.test("x\n"),
            "complement retains a linebreak scalar"
        );
        assert!(!component.test("xa"), "complement rejects a letter");
    }
    preparations(&[
        "[[:alpha:]]*.ts",
        "*.TS",
        "?k",
        "[k]*",
        "[[:alpha:]k]*",
        "@(K|s)*",
        "*[!a]",
        "*.İ",
    ]);
}

/// Preparation keeps native drive and UNC roots across parent operands.
#[test]
fn filesystem_patterns_preserve_native_root_identity() {
    preparations(&[
        "/a//*/",
        "//host/share/*.ts",
        "C:/a/*.ts",
        "//?/C:/a/*.ts",
        "C:/../*.ts",
        "//host/share/../*.ts",
        "//?/C:/../*.ts",
    ]);
}

/// Existing scalar and group corrections also apply under filesystem policy.
#[test]
fn filesystem_patterns_reuse_corrected_group_and_scalar_semantics() {
    /// One retained correction with a public membership witness.
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Correction {
        /// Authored component.
        pattern: String,
        /// Distinguishable scalar input.
        input: String,
        /// Corrected membership.
        expected: bool,
    }
    let rows: Vec<Correction> =
        serde_json::from_str(include_str!("fixtures/filesystem_corrections.json")).unwrap();
    for row in rows {
        for platform in [Platform::Posix, Platform::Darwin, Platform::Windows] {
            let compiled =
                Minimatch::new(&row.pattern, MinimatchOptions::Filesystem(platform)).unwrap();
            let result = match &compiled.set()[0][0] {
                ParseReturnFiltered::Pattern(pattern) => pattern.test(&row.input),
                ParseReturnFiltered::Literal(text) => *text == row.input,
                ParseReturnFiltered::Globstar => panic!("component must not be recursive"),
            };
            assert_eq!(result, row.expected, "{}: existing correction", row.pattern);
        }
    }
}
/// The public default operation retains comments, negation and level-one preparation.
#[test]
fn default_matching_stays_independent_of_filesystem_policy() {
    for (input, pattern, expected) in [
        ("#a.ts", "#*.ts", false),
        ("a.ts", "!b.ts", true),
        ("b.ts", "!b.ts", false),
        ("A.TS", "*.ts", false),
        ("a.ts", "*.ts", true),
        ("a.ts", "x/../*.ts", true),
        ("x/a.ts", "x/./*.ts", false),
        ("", "", true),
        ("a", "", false),
    ] {
        assert_eq!(
            minimatch(input, pattern).unwrap(),
            expected,
            "{pattern}: default policy"
        );
    }
    assert!(!minimatch("a", &"a".repeat(65_536)).unwrap());
    assert_eq!(
        minimatch("a", &"a".repeat(65_537)).unwrap_err().to_string(),
        "pattern is too long"
    );
}
/// A folded literal does not acquire the dot exclusion of a wildcard.
#[test]
fn filesystem_casefold_keeps_literal_token_identity() {
    let compiled = Minimatch::new(".Ab*", MinimatchOptions::Filesystem(Platform::Darwin)).unwrap();
    let ParseReturnFiltered::Pattern(pattern) = &compiled.set()[0][0] else {
        panic!("expected magic component");
    };
    assert!(pattern.test(".aBcd"));
    assert!(!pattern.test("aBcd"));
    let compiled = Minimatch::new("*Ab", MinimatchOptions::Filesystem(Platform::Darwin)).unwrap();
    let ParseReturnFiltered::Pattern(pattern) = &compiled.set()[0][0] else {
        panic!("expected magic component");
    };
    assert!(!pattern.test(".ab"));
}
/// Rewriting reaches a fixed point while each covering operand keeps its own suffix.
#[test]
fn filesystem_preparation_reaches_fixed_point_and_merges_borrowed_suffixes() {
    /// Pinned preparation without component queries.
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Row {
        /// Authored pattern.
        pattern: String,
        /// Platform spelling.
        platform: String,
        /// Ordered alternatives, including native root correction.
        expected: Vec<Vec<String>>,
    }
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("fixtures/filesystem_preparation.json")).unwrap();
    for row in rows {
        let platform = match row.platform.as_str() {
            "linux" => Platform::Posix,
            "darwin" => Platform::Darwin,
            "win32" => Platform::Windows,
            _ => panic!("unknown platform"),
        };
        let compiled =
            Minimatch::new(&row.pattern, MinimatchOptions::Filesystem(platform)).unwrap();
        assert_eq!(
            compiled.glob_parts(),
            row.expected,
            "{}: {}",
            row.platform,
            row.pattern
        );
    }
}

/// Escaped suffixes are interpreted by the parsed component evaluator.
#[test]
fn filesystem_escaped_suffixes_use_decoded_tokens() {
    for platform in [Platform::Darwin, Platform::Windows] {
        for (pattern, admitted, decoy) in [
            (r"*\.ts", "main.ts", "mainxts"),
            (r"?\.ts", "a.ts", "ab.ts"),
        ] {
            let matcher = Minimatch::new(pattern, MinimatchOptions::Filesystem(platform)).unwrap();
            let ParseReturnFiltered::Pattern(segment) = &matcher.set()[0][0] else {
                panic!("expected magic");
            };
            assert!(segment.test(admitted), "{pattern} accepts {admitted}");
            assert!(!segment.test(decoy), "{pattern} rejects {decoy}");
        }
    }
}

/// Literal operands stay exact under every filesystem policy, unlike explicit nocase matching.
#[test]
fn filesystem_preparation_keeps_literal_operands() {
    for platform in [Platform::Posix, Platform::Darwin, Platform::Windows] {
        let compiled = Minimatch::new("Dir/Case", MinimatchOptions::Filesystem(platform)).unwrap();
        assert_eq!(
            compiled.glob_parts(),
            [vec!["Dir".to_owned(), "Case".to_owned()]]
        );
        let [parts] = compiled.set() else {
            panic!("one alternative");
        };
        let literals: Vec<_> = parts
            .iter()
            .map(|part| match part {
                ParseReturnFiltered::Literal(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(literals, [Some("Dir"), Some("Case")]);
    }
    assert_eq!(
        maestro_pattern::minimatch_with_nocase("dir/case", "Dir/Case", true),
        Ok(true)
    );
    assert_eq!(minimatch("dir/case", "Dir/Case"), Ok(false));
}
