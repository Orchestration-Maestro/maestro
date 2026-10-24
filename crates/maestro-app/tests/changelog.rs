//! Recorded changelog sections, their release order and their selection.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::{ChangelogEntry, compare_versions, parse_changelog};
use maestro_test_directory::Scratch;
use serde::Deserialize;
use std::path::PathBuf;

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

/// Writes one recorded file row into its own file and parses that file.
fn parse_row(scratch: &Scratch, index: usize, row: &Row) -> Vec<ChangelogEntry> {
    let path = scratch.path().join(format!("row{index}.md"));
    let bytes = match row {
        Row::Text { text, .. } => text.clone().into_bytes(),
        Row::Bytes { bytes, .. } => bytes.clone(),
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
fn changelog_nonversion_sections_end_capture() {
    check_file_rows(&rows(), 18..25);
}

#[test]
fn changelog_keeps_line_endings_and_trims_boundaries() {
    check_file_rows(&rows(), 25..34);
}

#[test]
fn changelog_decodes_invalid_utf8_lossily() {
    check_file_rows(&rows(), 34..35);
}

#[test]
fn changelog_missing_or_unreadable_is_empty() {
    let rows = rows();
    let scratch = Scratch::new().unwrap();
    let directory = scratch.path().join("directory.md");
    std::fs::create_dir(&directory).unwrap();
    for (index, row) in rows.iter().enumerate().take(38).skip(35) {
        let Row::Unreadable { kind } = row else {
            panic!("row {index} is not a read failure");
        };
        let path = match kind {
            UnreadableKind::Missing => scratch.path().join("missing.md"),
            UnreadableKind::Directory => directory.clone(),
            UnreadableKind::Nul => PathBuf::from("nul\0name"),
        };
        assert!(
            parse_changelog(&path.display().to_string()).is_empty(),
            "row {index}"
        );
    }
}

#[test]
fn changelog_comparison_returns_first_component_difference() {
    let rows = rows();
    for (index, row) in rows.iter().enumerate().take(44).skip(38) {
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
}
