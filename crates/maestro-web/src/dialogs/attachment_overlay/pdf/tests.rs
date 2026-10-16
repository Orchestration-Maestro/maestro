//! PDF diagnostic selection.
use super::failure_detail;
#[test]
fn pdf_empty_failure_detail_uses_localized_fallback() {
    for message in [None, Some("")] {
        assert_eq!(failure_detail(message, "Échec PDF".into()), "Échec PDF");
    }
    assert_eq!(
        failure_detail(Some("<literal>"), "Échec PDF".into()),
        "<literal>"
    );
}
