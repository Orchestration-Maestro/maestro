//! Original shared selector formula cases.
use serde::Deserialize;
/// Recorded operands and result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    input: Input,
    expected: Expected,
}
/// Window or home-prefix inputs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    selected: Option<usize>,
    total: Option<usize>,
    capacity: Option<usize>,
    path: Option<String>,
    home: Option<String>,
}
/// Window bounds or shortened text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    start: Option<usize>,
    end: Option<usize>,
    path: Option<String>,
}
/// Decode the independently recorded expressions.
fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!("../../tests/fixtures/selector-shared.json")).unwrap()
}
#[test]
fn shared_list_window_matches_all_existing_capacities() {
    for case in cases()
        .into_iter()
        .filter(|c| c.test == "existing_window_regressions")
    {
        assert_eq!(
            super::list_window(
                case.input.selected.unwrap(),
                case.input.total.unwrap(),
                case.input.capacity.unwrap()
            ),
            case.expected.start.unwrap()..case.expected.end.unwrap()
        );
    }
}
#[test]
fn shared_home_prefix_matches_existing_source_expression() {
    for case in cases()
        .into_iter()
        .filter(|c| c.test == "existing_home_prefix_regression")
    {
        assert_eq!(
            super::shorten_home_path(
                case.input.path.as_deref().unwrap(),
                case.input.home.as_deref()
            ),
            case.expected.path.unwrap()
        );
    }
    assert_eq!(super::shorten_home_path("relative", None), "relative");
}
