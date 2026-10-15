//! Shared content and search through the application's session-tree boundary.
#![cfg(test)]
use maestro_app::{SessionTreeNode as InputNode, export_html::template};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{collections::HashSet, fmt::Debug, io};

/// Strict oracle envelope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Independently recorded complete invocations.
    cases: Vec<Case>,
}
/// One recorded invocation with raw, intentionally lazy operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Diagnostic name.
    name: String,
    /// Owning public witness.
    group: String,
    /// Public operation selector.
    operation: String,
    /// Unmodified raw operands.
    input: Value,
    /// Complete expected observation.
    expected: Value,
}
/// Strict success or native failure observation.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Outcome<T> {
    /// Successful complete value.
    Success { value: T },
    /// Reached invalid-data read.
    Error { error: bool },
}
/// Checks success values and exact native errors without hiding either branch.
fn check<T>(actual: io::Result<T>, expected: Value, name: &str, error_text: &str)
where
    T: DeserializeOwned + Debug + PartialEq,
{
    match serde_json::from_value::<Outcome<T>>(expected).unwrap() {
        Outcome::Success { value } => assert_eq!(actual.unwrap(), value, "{name}"),
        Outcome::Error { error } => {
            assert!(error, "{name}");
            let actual = actual.unwrap_err();
            assert_eq!(actual.kind(), io::ErrorKind::InvalidData, "{name}");
            assert_eq!(actual.to_string(), error_text, "{name}");
        }
    }
}
/// Loads a named group while rejecting duplicates and unknown/unowned cases.
fn cases(group: &str) -> Vec<Case> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("fixtures/session-content-search.json")).unwrap();
    let groups = [
        ("complete_content_keeps_blocks_and_raw_defaults", 12),
        ("terminal_prefix_keeps_cutoff_and_late_reads", 14),
        ("content_eligibility_keeps_whitespace_and_short_circuit", 14),
        ("complete_content_reports_reached_invalid_blocks", 7),
        ("search_surfaces_keep_their_own_fields_and_lengths", 25),
        ("literal_search_keeps_unicode_tokens_and_empty_fast_path", 9),
        ("search_optional_fields_keep_defaults_and_read_phase", 107),
        ("shared_search_observes_retained_label_updates", 1),
        ("plain_prefix_keeps_connectors_and_gutter_precedence", 14),
        (
            "viewer_paths_keep_last_identity_and_missing_parent_policy",
            11,
        ),
        ("viewer_paths_reuse_finite_cycle_correction", 3),
    ];
    let mut seen = HashSet::new();
    let mut names = HashSet::new();
    for case in &corpus.cases {
        assert!(groups.iter().any(|(name, _)| *name == case.group));
        assert!(seen.insert((case.operation.clone(), case.input.to_string())));
        assert!(names.insert(&case.name));
    }
    for (name, count) in groups {
        assert_eq!(
            corpus.cases.iter().filter(|c| c.group == name).count(),
            count,
            "{name}"
        );
    }
    corpus
        .cases
        .into_iter()
        .filter(|case| case.group == group)
        .collect()
}
/// Complete blocks concatenate without preview truncation or normalization.
#[test]
fn complete_content_keeps_blocks_and_raw_defaults() {
    for case in cases("complete_content_keeps_blocks_and_raw_defaults") {
        assert_eq!(case.operation, "extract");
        check(
            template::extract_content(&case.input["content"]),
            case.expected,
            &case.name,
            "",
        );
    }
}
/// Terminal prefix stops at its cutoff before reading later malformed fields.
#[test]
fn terminal_prefix_keeps_cutoff_and_late_reads() {
    for case in cases("terminal_prefix_keeps_cutoff_and_late_reads") {
        assert_eq!(case.operation, "prefix");
        check(
            template::extract_content_prefix(&case.input["content"], 200),
            case.expected,
            &case.name,
            "display field is not text",
        );
    }
}
/// Eligibility short-circuits after the first useful text with shared whitespace.
#[test]
fn content_eligibility_keeps_whitespace_and_short_circuit() {
    for case in cases("content_eligibility_keeps_whitespace_and_short_circuit") {
        assert_eq!(case.operation, "presence");
        check(
            template::has_text_content(&case.input["content"]),
            case.expected,
            &case.name,
            "text block is not a string",
        );
    }
}
/// Complete extraction reaches malformed blocks that a prefix could avoid.
#[test]
fn complete_content_reports_reached_invalid_blocks() {
    for case in cases("complete_content_reports_reached_invalid_blocks") {
        assert_eq!(case.operation, "extract");
        let null_block = case.input["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(Value::is_null);
        check(
            template::extract_content(&case.input["content"]),
            case.expected,
            &case.name,
            if null_block {
                "null content block"
            } else {
                "display field is not text"
            },
        );
    }
}
/// Retains a raw entry without eagerly reading message fields.
fn node(input: &Value) -> template::SessionTreeNode {
    let source = InputNode {
        entry: &input["entry"],
        children: vec![],
        label: input.get("label"),
        label_timestamp: None,
    };
    template::retain(&[&source]).remove(0)
}
/// Independently recorded observations for the two search surfaces.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchResults {
    /// Complete viewer text or its reached error.
    viewer: Value,
    /// Literal terminal predicate or its reached error.
    terminal: Value,
}
/// Chooses the exact existing diagnostic for a reached message receiver.
fn search_error(input: &Value) -> &'static str {
    if input["entry"]["type"] == "message" && input["entry"]["message"].is_null() {
        "missing message receiver"
    } else if input["entry"]["message"]["content"]
        .as_array()
        .is_some_and(|blocks| blocks.iter().any(Value::is_null))
    {
        "null content block"
    } else {
        "display field is not text"
    }
}
/// Each search operation asserts both surfaces independently, including failures.
fn check_search(case: Case) {
    let node = node(&case.input);
    let viewer = || template::get_searchable_text(&case.input["entry"], case.input.get("label"));
    let terminal = || template::matches_search(&node, case.input["query"].as_str().unwrap());
    match case.operation.as_str() {
        "search" => {
            let Outcome::Success { value } =
                serde_json::from_value::<Outcome<SearchResults>>(case.expected).unwrap()
            else {
                panic!("combined observations must remain independent")
            };
            check(
                viewer(),
                value.viewer,
                &case.name,
                search_error(&case.input),
            );
            check(
                terminal(),
                value.terminal,
                &case.name,
                search_error(&case.input),
            );
        }
        "viewer_search" => check(
            viewer(),
            case.expected,
            &case.name,
            search_error(&case.input),
        ),
        "terminal_search" => check(
            terminal(),
            case.expected,
            &case.name,
            search_error(&case.input),
        ),
        _ => panic!("unconsumed search operation"),
    }
}
/// Complete viewer content and terminal preview/bookkeeping remain distinct.
#[test]
fn search_surfaces_keep_their_own_fields_and_lengths() {
    for case in cases("search_surfaces_keep_their_own_fields_and_lengths") {
        check_search(case);
    }
}
/// Tokens are lowercase literals with the shared Unicode whitespace policy.
#[test]
fn literal_search_keeps_unicode_tokens_and_empty_fast_path() {
    for case in cases("literal_search_keeps_unicode_tokens_and_empty_fast_path") {
        check_search(case);
    }
}
/// Raw optional fields default or fail only at their consuming read phase.
#[test]
fn search_optional_fields_keep_defaults_and_read_phase() {
    for case in cases("search_optional_fields_keep_defaults_and_read_phase") {
        check_search(case);
    }
}
/// Complete before/after alias observations.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct LabelResults {
    /// Match before the shared slot edit.
    before: bool,
    /// Match after the shared slot edit.
    after: bool,
}
/// Search observes edits through another handle to the same retained slot.
#[test]
fn shared_search_observes_retained_label_updates() {
    for case in cases("shared_search_observes_retained_label_updates") {
        assert_eq!(case.operation, "label_search");
        let original = node(&case.input);
        let alias = original.clone();
        let query = case.input["query"].as_str().unwrap();
        let before = template::matches_search(&alias, query).unwrap();
        *original.label_mut() = Some(case.input["replacement"].clone());
        let after = template::matches_search(&alias, query).unwrap();
        check(
            Ok(LabelResults { before, after }),
            case.expected,
            &case.name,
            "",
        );
    }
}
/// Strict oracle geometry retains ordered, potentially duplicate gutters.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Row {
    /// Three-cell indentation level.
    indent: usize,
    /// Branch-start flag.
    show_connector: bool,
    /// Last sibling flag.
    is_last: bool,
    /// Ordered continuation observations.
    gutters: Vec<Gutter>,
    /// Virtual-root connector flag.
    is_virtual_root_child: bool,
    /// Virtual-root indentation suppression.
    multiple_roots: MultipleRoots,
}
/// Strict virtual-root policy, distinct from the row geometry flags.
#[derive(Deserialize)]
#[serde(transparent)]
struct MultipleRoots(
    /// Whether a virtual root suppresses one level.
    bool,
);
/// A supplied continuation at its original level.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Gutter {
    /// Logical level.
    position: usize,
    /// Visible continuation flag.
    show: bool,
}
/// Plain prefixes preserve gutters before connectors, without fold policy.
#[test]
fn plain_prefix_keeps_connectors_and_gutter_precedence() {
    for case in cases("plain_prefix_keeps_connectors_and_gutter_precedence") {
        assert_eq!(case.operation, "prefix_geometry");
        let row: Row = serde_json::from_value(case.input["row"].clone()).unwrap();
        let geometry = template::FlatNode {
            indent: row.indent,
            show_connector: row.show_connector,
            is_last: row.is_last,
            virtual_child: row.is_virtual_root_child,
            gutters: row
                .gutters
                .into_iter()
                .map(|g| template::GutterInfo {
                    position: g.position,
                    show: g.show,
                })
                .collect(),
        };
        check(
            Ok(template::build_tree_prefix(&geometry, row.multiple_roots.0)),
            case.expected,
            &case.name,
            "",
        );
    }
}
/// Raw path input retains each original object and duplicate identity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathInput {
    /// Original records in viewer index order.
    entries: Vec<Value>,
    /// Explicit target without history defaults.
    target: Option<String>,
}
/// Expected original positions and owned membership.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathResults {
    /// Root-first positions, not copied record equality.
    path: Vec<usize>,
    /// Membership only, without iteration order.
    active: HashSet<String>,
}
/// Compares original addresses and independent active membership per invocation.
fn check_paths(group: &str) {
    for case in cases(group) {
        assert_eq!(case.operation, "path");
        let input: PathInput = serde_json::from_value(case.input).unwrap();
        let Outcome::Success { value: expected } =
            serde_json::from_value::<Outcome<PathResults>>(case.expected).unwrap()
        else {
            panic!("path fixtures must have finite results")
        };
        let index: std::collections::HashMap<_, _> = input
            .entries
            .iter()
            .map(|entry| (entry["id"].as_str().unwrap(), entry))
            .collect();
        let lookup = |id: &str| index.get(id).copied();
        let path = template::get_path(input.target.as_deref(), lookup).unwrap();
        assert_eq!(path.len(), expected.path.len(), "{}", case.name);
        for (entry, position) in path.iter().zip(expected.path) {
            assert!(
                std::ptr::eq(*entry, &raw const input.entries[position]),
                "{}",
                case.name
            );
        }
        let active = template::build_active_path_ids(input.target.as_deref(), lookup).unwrap();
        drop(path);
        drop(index);
        drop(input);
        assert_eq!(active, expected.active, "{}", case.name);
    }
}
/// Viewer paths use last identities, stopping at missing, empty or self parents.
#[test]
fn viewer_paths_keep_last_identity_and_missing_parent_policy() {
    check_paths("viewer_paths_keep_last_identity_and_missing_parent_policy");
}
/// Admitted raw cycles terminate through the existing bounded ancestry owner.
#[test]
fn viewer_paths_reuse_finite_cycle_correction() {
    check_paths("viewer_paths_reuse_finite_cycle_correction");
}
