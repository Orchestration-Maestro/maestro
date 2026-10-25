//! Raw viewer operands and reached failure boundaries.
#[path = "../../../tests/export_support/corpus.rs"]
mod fixtures;

#[test]
fn export_display_string_keeps_missing_null_and_invalid_distinct() {
    for row in fixtures::rows(
        "export_display_string_keeps_missing_null_and_invalid_distinct",
        "str",
    ) {
        assert_eq!(row.branch, "success");
        let result = super::values::str(row.input.get("value"));
        assert_eq!(result, row.expected["value"].as_str(), "{}", row.id);
    }
}

#[test]
fn export_path_language_reuses_only_viewer_suffixes() {
    for row in fixtures::rows(
        "export_path_language_reuses_only_viewer_suffixes",
        "language",
    ) {
        assert_eq!(row.branch, "success");
        assert_eq!(
            super::values::get_language_from_path(row.input["path"].as_str().unwrap()),
            row.expected["value"].as_str(),
            "{}",
            row.id
        );
    }
}

#[test]
fn export_tool_results_keep_first_match_and_reached_errors() {
    for row in fixtures::rows(
        "export_tool_results_keep_first_match_and_reached_errors",
        "result",
    ) {
        let mut document = super::load_document("eyJlbnRyaWVzIjpbXX0=", "", None).unwrap();
        document
            .data
            .insert("entries".to_owned(), row.input["entries"].clone());
        let result = super::values::find_tool_result(&document, row.input["id"].as_str().unwrap());
        if row.branch == "error" {
            assert_eq!(row.expected["kind"], "InvalidData");
            assert_eq!(
                result.unwrap_err().kind(),
                std::io::ErrorKind::InvalidData,
                "{}",
                row.id
            );
        } else {
            assert_eq!(row.branch, "success");
            assert_eq!(
                result.unwrap(),
                row.expected.get("value").filter(|v| !v.is_null()),
                "{}",
                row.id
            );
        }
    }
}
