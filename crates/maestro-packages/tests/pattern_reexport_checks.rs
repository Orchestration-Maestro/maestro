//! The package pattern API retains the shared operation and error identity.
use maestro_packages::matches_glob;
use maestro_pattern::PatternError;

/// Existing callers receive the shared type without an error conversion.
#[test]
fn package_pattern_exports_use_the_shared_types() {
    assert!(matches_glob("nested/a.md", "**/*.md").unwrap());
    assert!(!matches_glob("nested/a.rs", "**/*.md").unwrap());
    let error: PatternError = matches_glob("file", &"?".repeat(65_537)).unwrap_err();
    assert_eq!(error, PatternError::TooLong);
    assert_eq!(error.to_string(), "pattern is too long");
}
