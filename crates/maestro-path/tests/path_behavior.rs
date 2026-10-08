//! Behavior of the lexical path operations against recorded runtime results.
#![cfg(test)]

use maestro_path::{Cwd, posix, win32};
use serde_json::{Value, json};

/// Recorded calls: runtime provenance and one expected result per call.
const FIXTURE: &str = include_str!("fixtures/path-vectors.json");

/// Load the recorded calls.
fn fixture() -> Vec<Value> {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    fixture["cases"].as_array().unwrap().clone()
}

/// Call every recorded case of one test and require its stored result.
fn check(test: &str, expected_cases: usize) {
    let cases: Vec<Value> = fixture()
        .into_iter()
        .filter(|case| case["test"] == test)
        .collect();
    assert_eq!(cases.len(), expected_cases, "{test}: recorded cases");
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| mismatch(case, call_flavor))
        .collect();
    assert!(failures.is_empty(), "{test}:\n{}", failures.join("\n"));
}

/// Describe a case whose actual result differs from the stored one.
fn mismatch(case: &Value, call: fn(&Value) -> Value) -> Option<String> {
    let actual = call(case);
    (actual != case["expected"]).then(|| {
        format!(
            "{} {} {} -> {actual}, expected {}",
            case["id"], case["flavor"], case["args"], case["expected"]
        )
    })
}

/// The decoded inputs of one recorded call.
struct Inputs<'a> {
    /// The path operands.
    args: Vec<&'a str>,
    /// The per-drive directories, keyed by drive letter.
    drives: Vec<(char, &'a str)>,
    /// The current working directory.
    current: &'a str,
}

impl<'a> Inputs<'a> {
    /// Decode the arguments and working-directory snapshot of `case`.
    fn of(case: &'a Value) -> Self {
        let drives = case["drives"]
            .as_object()
            .map(|drives| {
                drives
                    .iter()
                    .map(|(key, directory)| {
                        (
                            key.trim_start_matches('=').chars().next().unwrap(),
                            directory.as_str().unwrap(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            args: case["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|arg| arg.as_str().unwrap())
                .collect(),
            drives,
            current: case["cwd"].as_str().unwrap_or(""),
        }
    }

    /// The working-directory snapshot.
    fn cwd(&self) -> Cwd<'_> {
        Cwd {
            current: self.current,
            drive_directories: &self.drives,
        }
    }
}

/// Render the five fields of a parsed POSIX path.
fn render(parsed: posix::ParsedPath<'_>) -> Value {
    json!({
        "root": parsed.root,
        "dir": parsed.dir,
        "base": parsed.base,
        "ext": parsed.ext,
        "name": parsed.name,
    })
}

/// Run one recorded call through the public interface of its flavor.
fn call_flavor(case: &Value) -> Value {
    let inputs = Inputs::of(case);
    let args = &inputs.args;
    let cwd = inputs.cwd();
    match (
        case["flavor"].as_str().unwrap(),
        case["op"].as_str().unwrap(),
    ) {
        ("posix", "isAbsolute") => json!(posix::is_absolute(args[0])),
        ("posix", "normalize") => json!(posix::normalize(args[0])),
        ("posix", "join") => json!(posix::join(args)),
        ("posix", "dirname") => json!(posix::dirname(args[0])),
        ("posix", "relative") => json!(posix::relative(args[0], args[1], &cwd)),
        ("posix", "resolve") => json!(posix::resolve(args, &cwd)),
        ("posix", "parse") => render(posix::parse(args[0])),
        ("posix", "basename") => json!(posix::basename(args[0], args.get(1).copied())),
        ("win32", "isAbsolute") => json!(win32::is_absolute(args[0])),
        ("win32", "relative") => json!(win32::relative(args[0], args[1], &cwd)),
        ("win32", "resolve") => json!(win32::resolve(args, &cwd)),
        ("win32", "basename") => json!(win32::basename(args[0], args.get(1).copied())),
        ("win32", "dirname") => json!(win32::dirname(args[0])),
        ("win32", "join") => json!(win32::join(args)),
        ("win32", "normalize") => json!(win32::normalize(args[0])),
        (flavor, op) => unreachable!("unsupported call {flavor}.{op}"),
    }
}

#[test]
fn posix_absolute_requires_a_leading_slash() {
    check("posix_absolute_requires_a_leading_slash", 40);
}

#[test]
fn win32_absolute_distinguishes_drive_relative_paths() {
    check("win32_absolute_distinguishes_drive_relative_paths", 53);
}

#[test]
fn posix_normalize_preserves_relative_parents_and_trailing_slashes() {
    check(
        "posix_normalize_preserves_relative_parents_and_trailing_slashes",
        56,
    );
}

#[test]
fn posix_join_reduces_segments() {
    check("posix_join_reduces_segments", 61);
}

#[test]
fn posix_dirname_preserves_authored_separators() {
    check("posix_dirname_preserves_authored_separators", 55);
}

#[test]
fn posix_basename_applies_literal_suffix_rules() {
    check("posix_basename_applies_literal_suffix_rules", 193);
}

#[test]
fn posix_parse_preserves_authored_fields() {
    check("posix_parse_preserves_authored_fields", 65);
}

#[test]
fn posix_parse_keeps_root_parent_components_extensionless() {
    check("posix_parse_keeps_root_parent_components_extensionless", 3);
}

#[test]
fn posix_resolve_uses_the_supplied_working_directory() {
    check("posix_resolve_uses_the_supplied_working_directory", 45);
}

#[test]
fn posix_relative_compares_complete_segments() {
    check("posix_relative_compares_complete_segments", 36);
}

#[test]
fn win32_normalize_handles_drives_unc_and_reserved_names() {
    check("win32_normalize_handles_drives_unc_and_reserved_names", 372);
}

#[test]
fn win32_join_preserves_roots_and_devices() {
    check("win32_join_preserves_roots_and_devices", 104);
}

#[test]
fn win32_dirname_retains_root_spelling() {
    check("win32_dirname_retains_root_spelling", 83);
}

#[test]
fn win32_basename_distinguishes_drive_prefixes() {
    check("win32_basename_distinguishes_drive_prefixes", 176);
}

#[test]
fn win32_resolve_uses_supplied_drive_directories() {
    check("win32_resolve_uses_supplied_drive_directories", 89);
}

#[test]
fn win32_relative_preserves_destination_spelling() {
    check("win32_relative_preserves_destination_spelling", 50);
}

#[test]
fn win32_relative_returns_absolute_for_distinct_roots() {
    check("win32_relative_returns_absolute_for_distinct_roots", 9);
}

#[test]
fn win32_relative_separates_drive_and_unc_roots_with_equal_components() {
    let cwd = Cwd {
        current: "C:\\work",
        drive_directories: &[],
    };
    for (from, to, expected) in [
        ("C:\\a\\b", "\\\\C:\\a\\b", "\\\\C:\\a\\b"),
        ("\\\\C:\\a\\b", "C:\\a\\b", "C:\\a\\b"),
    ] {
        assert_eq!(win32::relative(from, to, &cwd), expected, "{from:?} {to:?}");
    }
}

/// Resolving a relative result from the source gives the destination whenever
/// both ends are absolute; a drive-relative source would apply its drive
/// directory a second time.
#[test]
fn win32_relative_resolves_back_to_the_destination_for_every_anchored_pair() {
    let cases: Vec<Value> = fixture()
        .into_iter()
        .filter(|case| case["flavor"] == "win32" && case["op"] == "relative")
        .collect();
    assert_eq!(cases.len(), 59, "recorded win32 relative cases");
    let mut anchored = 0;
    for case in &cases {
        let inputs = Inputs::of(case);
        let cwd = inputs.cwd();
        let from = win32::resolve(&[inputs.args[0]], &cwd);
        let destination = win32::resolve(&[inputs.args[1]], &cwd);
        if !(win32::is_absolute(&from) && win32::is_absolute(&destination)) {
            continue;
        }
        anchored += 1;
        let relative = win32::relative(inputs.args[0], inputs.args[1], &cwd);
        let rejoined = win32::resolve(&[&from, &relative], &cwd);
        assert_eq!(
            rejoined.to_lowercase(),
            destination.to_lowercase(),
            "{} {from:?} + {relative:?}",
            case["id"]
        );
    }
    assert_eq!(anchored, 58, "cases whose both ends are absolute");
}

/// Fixture rows whose inputs make the two flavors disagree, one per root operation.
#[cfg(not(windows))]
const ROOT_ROWS: [&str; 8] = [
    "p0710", "p0318", "p0482", "p1233", "p0068", "p0657", "p0692", "p1102",
];

/// Fixture rows whose inputs make the two flavors disagree, one per root operation.
#[cfg(windows)]
const ROOT_ROWS: [&str; 7] = [
    "p0958", "p0271", "p1154", "p1252", "p0890", "p0052", "p0940",
];

/// Run one recorded call through the target-default entry points.
fn call_root(case: &Value) -> Value {
    let inputs = Inputs::of(case);
    let args = &inputs.args;
    let cwd = inputs.cwd();
    match case["op"].as_str().unwrap() {
        "isAbsolute" => json!(maestro_path::is_absolute(args[0])),
        "normalize" => json!(maestro_path::normalize(args[0])),
        "join" => json!(maestro_path::join(args)),
        "dirname" => json!(maestro_path::dirname(args[0])),
        "relative" => json!(maestro_path::relative(args[0], args[1], &cwd)),
        "resolve" => json!(maestro_path::resolve(args, &cwd)),
        "basename" => json!(maestro_path::basename(args[0], args.get(1).copied())),
        #[cfg(not(windows))]
        "parse" => render(maestro_path::parse(args[0])),
        op => unreachable!("unsupported call {op}"),
    }
}

#[test]
fn target_default_exports_the_native_flavor() {
    let _: fn(&[&str]) -> String = maestro_path::join;
    let _: fn(&str) -> String = maestro_path::normalize;
    let _: for<'a, 'b> fn(&[&'a str], &Cwd<'b>) -> String = maestro_path::resolve;
    let _: for<'a, 'b> fn(&'a str, &'a str, &Cwd<'b>) -> String = maestro_path::relative;
    let _: fn(&str) -> String = maestro_path::dirname;
    let _: fn(&str, Option<&str>) -> String = maestro_path::basename;
    let _: fn(&str) -> bool = maestro_path::is_absolute;
    assert_eq!(maestro_path::SEP, if cfg!(windows) { '\\' } else { '/' });
    let cases = fixture();
    for id in ROOT_ROWS {
        let case = cases.iter().find(|case| case["id"] == id).unwrap();
        assert_eq!(mismatch(case, call_root), None, "{id}");
    }
}

#[test]
fn separator_only_paths_have_an_empty_basename_with_any_suffix() {
    for (path, suffix) in [("/", "x"), ("///", "x"), ("///", "/")] {
        assert_eq!(
            posix::basename(path, Some(suffix)),
            "",
            "posix {path:?} {suffix:?}"
        );
    }
    for (path, suffix) in [
        ("/", "x"),
        ("C:/", "x"),
        ("C:\\", "x"),
        ("\\\\", "x"),
        ("C:\\", "\\"),
    ] {
        assert_eq!(
            win32::basename(path, Some(suffix)),
            "",
            "win32 {path:?} {suffix:?}"
        );
    }
}

#[test]
fn win32_normalize_keeps_the_device_namespace_before_reserved_names() {
    for (path, expected) in [
        ("\\\\.\\COM1:foo", "\\\\.\\COM1:\\foo"),
        ("\\\\.\\COM1:..\\x", "\\\\.\\COM1:\\x"),
        ("\\\\?\\COM1:foo", "\\\\?\\COM1:\\foo"),
        ("\\\\?\\COM1:..\\x", "\\\\?\\COM1:\\x"),
    ] {
        assert_eq!(win32::normalize(path), expected, "{path:?}");
    }
}

#[test]
fn win32_normalize_prefixes_only_reserved_names_before_a_colon() {
    for (path, expected) in [
        ("CONx", "CONx"),
        ("CON😀", "CON😀"),
        ("CON:x", ".\\CON:x"),
        ("COM1:", ".\\COM1:."),
    ] {
        assert_eq!(win32::normalize(path), expected, "{path:?}");
    }
}

#[test]
fn win32_normalize_leaves_a_lone_colon_and_astral_endings_as_written() {
    for path in [":", "PRN😀", "😀"] {
        assert_eq!(win32::normalize(path), path, "{path:?}");
    }
}

#[test]
fn win32_resolve_reads_drive_directories_by_character() {
    let cwd = Cwd {
        current: "D:\\work",
        drive_directories: &[('C', "éé\\other")],
    };
    assert_eq!(win32::resolve(&["C:file"], &cwd), "C:\\file");
}

#[test]
fn relative_normalizes_an_unnormalized_working_directory() {
    let posix_cwd = Cwd {
        current: "/a//b/",
        drive_directories: &[],
    };
    assert_eq!(posix::relative("", "c", &posix_cwd), "c");
    assert_eq!(posix::relative("c", "", &posix_cwd), "..");
    let win32_cwd = Cwd {
        current: "\\\\srv\\sh\\\\dir",
        drive_directories: &[],
    };
    assert_eq!(win32::relative("", "c", &win32_cwd), "c");
    assert_eq!(win32::relative("c", "", &win32_cwd), "..");
}
