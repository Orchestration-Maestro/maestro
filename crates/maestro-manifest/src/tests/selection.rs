//! The `manifest` setting and the user's lock over the project's.
use crate::selection::{parse, project_ignored};
use serde_json::json;

const SOURCE: &str = "git:github.com/acme/manifests@v1.2.0";

#[test]
fn an_absent_empty_or_blank_selection_means_internal_mode() {
    for document in [
        json!({}),
        json!({"other": 1}),
        json!({"manifest": ""}),
        json!({"manifest": "  \t"}),
        json!({"manifest": {"source": ""}}),
        json!({"manifest": {"source": " "}}),
    ] {
        assert_eq!(parse(&document), Ok(None), "{document}");
    }
}

#[test]
fn a_source_string_or_package_entry_names_the_manifest() {
    let named = Ok(Some(SOURCE.to_owned()));
    assert_eq!(parse(&json!({"manifest": SOURCE})), named);
    assert_eq!(parse(&json!({"manifest": format!("  {SOURCE}  ")})), named);
    assert_eq!(parse(&json!({"manifest": {"source": SOURCE}})), named);
    let filtered = json!({"manifest": {
        "source": SOURCE, "extensions": [], "skills": ["a"], "prompts": ["b"], "themes": ["c"]
    }});
    assert_eq!(parse(&filtered), named);
}

#[test]
fn a_wrong_kind_is_an_invalid_selection() {
    let source = "git:github.com/acme/m@v1";
    let cases = [
        (json!({"manifest": 7}), "string or an object"),
        (json!({"manifest": true}), "string or an object"),
        (json!({"manifest": null}), "string or an object"),
        (json!({"manifest": [source]}), "string or an object"),
        (json!({"manifest": {}}), "`source`"),
        (json!({"manifest": {"source": 7}}), "`source`"),
        (
            json!({"manifest": {"source": source, "skills": "all"}}),
            "`skills`",
        ),
        (
            json!({"manifest": {"source": source, "themes": [1]}}),
            "`themes`",
        ),
    ];
    for (document, expected) in cases {
        let problem = parse(&document).expect_err(&document.to_string());
        assert!(problem.contains(expected), "{document}: {problem}");
    }
}

#[test]
fn a_user_lock_sets_aside_only_a_project_manifest_of_another_repository() {
    let user = "git:github.com/acme/manifests@v1";
    assert!(project_ignored(
        user,
        "git:github.com/other/manifests@v1",
        true
    ));
    assert!(project_ignored(
        "git@github.com:acme/manifests@v1",
        "https://github.com/acme/manifests@v1",
        true
    ));
    assert!(!project_ignored(
        user,
        "git:github.com/acme/manifests@v2",
        true
    ));
    assert!(!project_ignored(
        "git@github.com:acme/manifests@v1",
        "git@github.com:acme/manifests@v2",
        true
    ));
    assert!(!project_ignored(
        user,
        "git:github.com/other/manifests@v1",
        false
    ));
}
