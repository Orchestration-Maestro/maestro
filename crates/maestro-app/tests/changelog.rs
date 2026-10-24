//! Recorded changelog sections, their release order and their selection.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::{ChangelogEntry, compare_versions};
use serde::Deserialize;

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

/// One recorded changelog row.
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Row {
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

#[test]
fn changelog_comparison_returns_first_component_difference() {
    for (index, row) in rows().iter().enumerate() {
        let Row::Compare {
            left,
            right,
            expected,
        } = row;
        assert_eq!(
            compare_versions(&entry(left), &entry(right)).to_bits(),
            expected.to_bits(),
            "row {index}"
        );
    }
}
