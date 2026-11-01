//! Recorded changelog sections, their release order and their selection.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

#[cfg(unix)]
use crate::resource_fixtures::{child_stderr, directory_read_error, warnings};
use maestro_app::{ChangelogEntry, compare_versions, parse_changelog};
use maestro_test_directory::Scratch;
use serde::Deserialize;
use std::path::Path;

/// The recorded fields of one changelog entry.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Section {
    /// Major component.
    major: f64,
    /// Minor component.
    minor: f64,
    /// Patch component.
    patch: f64,
    /// Heading line and section body.
    content: String,
}

/// How a recorded changelog path cannot be read.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum UnreadableKind {
    /// The path names no file.
    Missing,
    /// The path names a directory.
    Directory,
    /// The path holds a byte that no path may contain.
    Nul,
}

/// One recorded changelog row.
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Row {
    /// A changelog file read through its text.
    Text {
        /// Complete file text.
        text: String,
        /// The sections the reference returned.
        expected: Vec<Section>,
    },
    /// A changelog file read through its bytes.
    Bytes {
        /// Complete file bytes.
        bytes: Vec<u8>,
        /// The sections the reference returned.
        expected: Vec<Section>,
    },
    /// A path the reference could not read.
    Unreadable {
        /// How the path fails to read.
        kind: UnreadableKind,
        /// The warning lines the reference wrote to standard error.
        warnings: usize,
    },
    /// A selection for one saved dotted version.
    Select {
        /// Entries in their input order.
        entries: Vec<Section>,
        /// The saved version text.
        last_version: String,
        /// The input positions the reference selected.
        expected: Vec<usize>,
    },
    /// A comparison of two numeric records.
    Compare {
        /// First operand.
        left: Section,
        /// Second operand.
        right: Section,
        /// The first differing numeric difference.
        expected: f64,
    },
}

/// The recorded changelog rows in the order the reference produced them.
fn rows() -> Vec<Row> {
    serde_json::from_str(include_str!("fixtures/changelog.json")).unwrap()
}

/// The public record described by a recorded section.
fn entry(section: &Section) -> ChangelogEntry {
    ChangelogEntry {
        major: section.major,
        minor: section.minor,
        patch: section.patch,
        content: section.content.clone(),
    }
}

/// The recorded section fields of parsed entries.
fn sections(entries: &[ChangelogEntry]) -> Vec<Section> {
    entries
        .iter()
        .map(|entry| Section {
            major: entry.major,
            minor: entry.minor,
            patch: entry.patch,
            content: entry.content.clone(),
        })
        .collect()
}

/// The public records described by the recorded entries of a selection row.
fn select_entries(entries: &[Section]) -> Vec<ChangelogEntry> {
    entries.iter().map(entry).collect()
}

/// The input positions of the selected entries, which are the original records.
fn positions(entries: &[ChangelogEntry], selected: &[&ChangelogEntry]) -> Vec<usize> {
    selected
        .iter()
        .map(|chosen| {
            entries
                .iter()
                .position(|entry| std::ptr::eq(entry, *chosen))
                .unwrap()
        })
        .collect()
}

/// Writes one recorded file row into its own file and parses that file.
fn parse_row(scratch: &Scratch, index: usize, row: &Row) -> Vec<ChangelogEntry> {
    let path = scratch.path().join(format!("row{index}.md"));
    let bytes: &[u8] = match row {
        Row::Text { text, .. } => text.as_bytes(),
        Row::Bytes { bytes, .. } => bytes,
        _ => panic!("row {index} is not a file row"),
    };
    std::fs::write(&path, bytes).unwrap();
    parse_changelog(&path.display().to_string())
}

/// Asserts the recorded sections of the file rows in `rows`, keeping their indexes.
fn check_file_rows(rows: &[Row], range: std::ops::Range<usize>) {
    let scratch = Scratch::new().unwrap();
    for (index, row) in rows.iter().enumerate().take(range.end).skip(range.start) {
        let (Row::Text { expected, .. } | Row::Bytes { expected, .. }) = row else {
            panic!("row {index} is not a file row");
        };
        assert_eq!(
            sections(&parse_row(&scratch, index, row)),
            *expected,
            "row {index}"
        );
    }
}

#[test]
fn changelog_sections_keep_headings_body_order_and_duplicates() {
    check_file_rows(&rows(), 0..1);
}

#[test]
fn changelog_heading_recognition_matches_delimiters() {
    check_file_rows(&rows(), 1..18);
}

#[test]
fn changelog_overlapping_heading_candidates_keep_the_section() {
    check_file_rows(&rows(), 18..19);
}

#[test]
fn changelog_nonversion_sections_end_capture() {
    check_file_rows(&rows(), 19..26);
}

#[test]
fn changelog_keeps_line_endings_and_trims_boundaries() {
    check_file_rows(&rows(), 26..35);
}

#[test]
fn changelog_decodes_invalid_utf8_lossily() {
    check_file_rows(&rows(), 35..36);
}

/// Parses every recorded read failure under `base`, which names a directory.
fn parse_read_failures(base: &str) {
    let rows = rows();
    let missing = format!("{base}/missing.md");
    let directory = format!("{base}/unreadable.md");
    std::fs::create_dir(&directory).unwrap();
    for (index, row) in rows.iter().enumerate().take(39).skip(36) {
        let Row::Unreadable { kind, warnings } = row else {
            panic!("row {index} is not a read failure");
        };
        let path = match kind {
            UnreadableKind::Missing => missing.as_str(),
            UnreadableKind::Directory => directory.as_str(),
            UnreadableKind::Nul => "nul\0name",
        };
        assert_eq!(Path::new(path).exists(), *warnings == 1, "row {index}");
        assert!(parse_changelog(path).is_empty(), "row {index}");
    }
}

/// Compares the child's standard error with the reference's read warning.
///
/// The child parses every recorded failure, so its standard error carries the one warning.
#[cfg(unix)]
fn check_read_warning() {
    let Some((_base, stderr)) = child_stderr("changelog_missing_or_unreadable_is_empty", |base| {
        parse_read_failures(base);
    }) else {
        return;
    };
    assert_eq!(warnings(&stderr), [read_warning()]);
}

/// The warning line written for the changelog path `path`, which is a directory.
#[cfg(unix)]
fn read_warning() -> String {
    format!(
        "Warning: Could not parse changelog: {}",
        directory_read_error()
    )
}

#[test]
fn changelog_missing_or_unreadable_is_empty() {
    let scratch = Scratch::new().unwrap();
    let base = scratch.path().display().to_string();
    #[cfg(unix)]
    check_read_warning();
    if cfg!(not(unix)) {
        parse_read_failures(&base);
    }
}

#[test]
fn changelog_comparison_returns_first_component_difference() {
    let rows = rows();
    for (index, row) in rows.iter().enumerate().take(45).skip(39) {
        let Row::Compare {
            left,
            right,
            expected,
        } = row
        else {
            panic!("row {index} is not a comparison");
        };
        assert_eq!(
            compare_versions(&entry(left), &entry(right)).to_bits(),
            expected.to_bits(),
            "row {index}"
        );
    }
}

#[test]
fn changelog_exports_keep_the_configuration_path_owner() {
    let scratch = Scratch::new().unwrap();
    let package_dir = scratch.path().display().to_string();
    let owner =
        maestro_app::configuration::config::paths::get_changelog_path(&package_dir).unwrap();
    assert_eq!(
        maestro_app::get_changelog_path(&package_dir).unwrap(),
        owner
    );
    assert_eq!(
        maestro_app::utils::changelog::get_changelog_path(&package_dir).unwrap(),
        owner
    );
    std::fs::write(&owner, "## 2.0.0\nbody").unwrap();
    let root = maestro_app::parse_changelog(&owner);
    assert_eq!(root, maestro_app::utils::changelog::parse_changelog(&owner));
    let last = ChangelogEntry {
        major: 1.0,
        minor: 0.0,
        patch: 0.0,
        content: String::new(),
    };
    assert_eq!(root.len(), 1);
    assert_eq!(
        maestro_app::compare_versions(&root[0], &last).to_bits(),
        1.0_f64.to_bits()
    );
    assert_eq!(
        maestro_app::utils::changelog::compare_versions(&root[0], &last).to_bits(),
        1.0_f64.to_bits()
    );
    let newer = maestro_app::get_new_entries(&root, "1.0.0");
    assert_eq!(newer, [&root[0]]);
    let newer = maestro_app::utils::changelog::get_new_entries(&root, "1.0.0");
    assert_eq!(newer, [&root[0]]);
}

/// Asserts the recorded selection of the rows in `rows`, keeping their indexes.
fn check_selection_rows(rows: &[Row], range: std::ops::Range<usize>) {
    for (index, row) in rows.iter().enumerate().take(range.end).skip(range.start) {
        let Row::Select {
            entries,
            last_version,
            expected,
        } = row
        else {
            panic!("row {index} is not a selection");
        };
        let input = select_entries(entries);
        let selected = maestro_app::get_new_entries(&input, last_version);
        assert_eq!(positions(&input, &selected), *expected, "row {index}");
    }
}

#[test]
fn changelog_selection_is_strict_and_keeps_input_order() {
    check_selection_rows(&rows(), 45..46);
}

#[test]
fn changelog_saved_version_defaults_missing_and_invalid_parts() {
    check_selection_rows(&rows(), 46..64);
}

#[test]
fn changelog_selection_borrows_original_entries_and_leaves_them_intact() {
    check_selection_rows(&rows(), 64..66);
}
