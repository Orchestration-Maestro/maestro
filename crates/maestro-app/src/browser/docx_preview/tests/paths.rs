//! Controlled package-path observations.
use super::super::utils;
use super::fixture_cases::cases;
use serde::Deserialize;
use serde_json::json;

/// Authored path and optional URL base.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Authored spelling.
    value: String,
    /// URL base folder.
    base: Option<String>,
}
#[test]
fn package_paths_split_and_remove_one_leading_slash() {
    for case in cases::<Input>(
        include_str!("../../../../tests/fixtures/docx/paths.json"),
        "package_paths_split_and_remove_one_leading_slash",
    ) {
        assert!(case.input.base.is_none());
        let (folder, file) = utils::split_path(&case.input.value);
        assert_eq!(
            json!({"split": [folder, file], "normalized": utils::normalize_path(&case.input.value)}),
            case.expected
        );
    }
}
#[test]
fn relationship_targets_use_url_resolution() {
    for case in cases::<Input>(
        include_str!("../../../../tests/fixtures/docx/paths.json"),
        "relationship_targets_use_url_resolution",
    ) {
        assert_eq!(
            json!(utils::resolve_path(
                &case.input.value,
                case.input.base.as_deref().unwrap()
            )),
            case.expected
        );
    }
}
#[test]
fn archive_names_apply_package_normalization() {
    for case in cases::<Input>(
        include_str!("../../../../tests/fixtures/docx/paths.json"),
        "archive_names_apply_package_normalization",
    ) {
        assert!(case.input.base.is_none());
        assert_eq!(json!(utils::archive_name(&case.input.value)), case.expected);
    }
}
