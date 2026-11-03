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
fn shared_home_prefix_stops_at_path_component_boundaries() {
    for case in cases()
        .into_iter()
        .filter(|c| c.test == "existing_home_prefix_regression")
    {
        assert_eq!(
            maestro_path::shorten_home(
                case.input.path.as_deref().unwrap(),
                case.input.home.as_deref().unwrap_or_default()
            ),
            case.expected.path.unwrap()
        );
    }
}

#[test]
fn shared_summary_display_cleans_emitted_markdown() {
    use super::branch_summary_message::{SummaryDisplay, SummaryText};
    use maestro_tui::Component;
    use std::rc::Rc;
    let theme = maestro_theme::Theme::new(
        vec![],
        vec![],
        maestro_theme::ColorMode::Truecolor,
        maestro_theme::ThemeOptions::default(),
    )
    .unwrap();
    let live = maestro_theme::LiveTheme::from_theme(Rc::new(theme));
    let markdown = Rc::new(maestro_theme::get_markdown_theme(
        live.clone(),
        Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
        true,
    ));
    let display = SummaryDisplay::new(
        SummaryText {
            summary: "before &#27;]52;c;b3duZWQ=&#7; after".into(),
            label: "[branch]",
            header: "**Branch Summary**\n\n".into(),
            collapsed: "Branch summary (".into(),
        },
        live,
        markdown,
        maestro_tui::TerminalImage::new(|_| None, || 1),
    );
    display.set_expanded(true);
    let rows = display.panel.render(160).join("\n");
    assert!(
        !rows.contains("\x1b]52;"),
        "shared summary emits clipboard escape"
    );
    assert_eq!(rows.matches("before").count(), 1);
    assert_eq!(rows.matches("after").count(), 1);
}
