//! Artifact renderer preparation through the embedding facade.
#![cfg(test)]
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiffCase {
    old: String,
    new: String,
    parts: Vec<Part>,
}
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Part {
    value: String,
    added: bool,
    removed: bool,
}
#[test]
fn artifact_diffs_keep_ordered_runs_prefixes_and_line_endings() {
    use maestro_app::{Change, diff_lines};
    let cases: Vec<DiffCase> =
        serde_json::from_str(include_str!("fixtures/artifact_diff_runs.json")).unwrap();
    assert_eq!(cases.len(), 16);
    for (index, case) in cases.into_iter().enumerate() {
        let observed: Vec<_> = diff_lines(&case.old, &case.new)
            .into_iter()
            .map(|part| {
                let (value, added, removed) = match part {
                    Change::Unchanged(value) => (value, false, false),
                    Change::Added(value) => (value, true, false),
                    Change::Removed(value) => (value, false, true),
                };
                Part {
                    value: value.into(),
                    added,
                    removed,
                }
            })
            .collect();
        assert_eq!(observed, case.parts, "case {index}");
    }
}

fn assert_preparation(name: &str, count: usize) {
    let rows = crate::artifact_card_support::rows(name);
    assert_eq!(rows.len(), count);
    for (index, row) in rows.into_iter().enumerate() {
        let recipe = crate::artifact_card_support::prepare(&row.input).unwrap();
        assert_eq!(recipe.is_custom, row.expected.is_custom, "row {index}");
    }
}

#[test]
fn artifact_cards_cover_command_status_and_parameter_branches() {
    assert_preparation(
        "artifact_cards_cover_command_status_and_parameter_branches",
        54,
    );
}
#[test]
fn artifact_cards_keep_literal_output_and_html_console_rules() {
    assert_preparation(
        "artifact_cards_keep_literal_output_and_html_console_rules",
        288,
    );
}
#[test]
fn artifact_cards_place_pills_only_on_the_selected_header_branches() {
    assert_preparation(
        "artifact_cards_place_pills_only_on_the_selected_header_branches",
        21,
    );
}
#[test]
fn artifact_update_distinguishes_absent_and_empty_replacements() {
    assert_preparation(
        "artifact_update_distinguishes_absent_and_empty_replacements",
        27,
    );
}
#[test]
fn artifact_languages_keep_literal_suffix_rules() {
    use crate::artifact_card_support as cards;
    use serde_json::json;
    let valid: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/artifact_languages.json")).unwrap();
    crate::support::strict_record::<crate::artifact_card_support::Language>(&valid[0], &["input"]);
    crate::support::strict_record::<crate::artifact_card_support::Filename>(
        &valid[0]["input"],
        &[],
    );
    let mut sibling = valid[0].clone();
    sibling["expected"] = "browser language".into();
    assert!(serde_json::from_value::<crate::artifact_card_support::Language>(sibling).is_err());
    let languages = cards::languages();
    assert_eq!(languages.len(), 77);
    for row in languages {
        let mut params = json!({"command":"create", "content":"literal"});
        if let Some(filename) = row.input.filename {
            params["filename"] = filename.into();
        }
        assert!(!cards::value(Some(&params), "pending").unwrap().is_custom);
    }
}
#[test]
fn artifact_fields_are_read_only_by_the_reached_branch() {
    use crate::artifact_card_support::{self as cards, Branch};
    let rows = cards::admission();
    assert_eq!(rows.len(), 390);
    for row in rows {
        let actual = cards::value(Some(&row.input), &row.phase);
        let context = format!("{} {} {} {}", row.context, row.key, row.kind, row.phase);
        match row.expected {
            Branch::Prepared { is_custom } => {
                assert_eq!(actual.unwrap().is_custom, is_custom, "{context}");
            }
            Branch::Error { kind } => {
                assert_eq!(kind, "TypeError");
                assert!(actual.is_err(), "{context}");
            }
            Branch::Rendered { .. } => panic!("unexpected admission branch"),
        }
    }
    let domains = cards::code_domains();
    assert_eq!(domains.len(), 42);
    for row in domains {
        assert!(
            !cards::value(Some(&row.input.params), &row.input.phase)
                .unwrap()
                .is_custom,
            "{}",
            row.input.phase
        );
    }
    let routes = cards::value_routes();
    assert_eq!(routes.len(), 39);
    for row in routes {
        assert_eq!(
            cards::value(row.input.params.as_ref(), &row.input.phase)
                .unwrap()
                .is_custom,
            row.expected.is_custom
        );
    }
    assert_unused_diffs();
}

fn assert_unused_diffs() {
    use crate::artifact_card_support::{self as cards, Branch};
    use crate::support::strict_record;
    let valid: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/artifact_unused_diff.json")).unwrap();
    strict_record::<crate::artifact_card_support::UnusedDiff>(&valid[0], &["input", "expected"]);
    strict_record::<crate::support::tool_input::Input>(&valid[0]["input"], &["isStreaming"]);
    strict_record::<crate::artifact_card_support::Branch>(
        &valid[0]["expected"],
        &["branch", "card"],
    );
    strict_record::<crate::artifact_card_support::Branch>(
        &valid[2]["expected"],
        &["branch", "kind"],
    );
    strict_record::<crate::artifact_card_support::Expected>(
        &valid[0]["expected"]["card"],
        &["isCustom"],
    );
    let mut wrong = valid[0]["expected"].clone();
    wrong["branch"] = "unknown".into();
    assert!(serde_json::from_value::<crate::artifact_card_support::Branch>(wrong).is_err());
    let mut sibling = valid[0]["expected"]["card"].clone();
    sibling["caption"] = "browser caption".into();
    assert!(serde_json::from_value::<crate::artifact_card_support::Expected>(sibling).is_err());

    for row in cards::unused_diffs() {
        let actual: Result<maestro_web::tools::types::ToolRenderResult, _> =
            maestro_web::ToolRenderer::render(
                &maestro_web::ArtifactsToolRenderer::new(None),
                row.input.params.as_ref(),
                row.input.result.as_ref().map(Into::into),
                row.input.is_streaming,
            );
        match row.expected {
            Branch::Rendered { card } => assert_eq!(actual.unwrap().is_custom, card.is_custom),
            Branch::Error { kind } => {
                assert_eq!(kind, "TypeError");
                assert!(actual.is_err());
            }
            Branch::Prepared { .. } => panic!("unexpected discarded diff branch"),
        }
    }
}

#[test]
fn artifact_recipes_own_selected_inputs_and_panel_handles() {
    use maestro_app::browser::tools::artifacts::{
        ArtifactCommand, ArtifactsParams, LocalArtifactsPanel,
    };
    use maestro_web::{ArtifactsToolRenderer, ToolRenderer};
    use std::rc::Rc;
    let host = Rc::new(crate::attachment_support::Host::default());
    let presentation = Rc::new(crate::artifact_support::presentation::Presentation::new(
        host.clone(),
    ));
    let panel = Rc::new(LocalArtifactsPanel::new_local(presentation));
    let weak = Rc::downgrade(&panel);
    let mut renderer = ArtifactsToolRenderer::new(Some(panel));
    let params = ArtifactsParams {
        command: ArtifactCommand::Create,
        filename: "literal <&> 雪.html".into(),
        content: Some("YWJj\r\n".into()),
        old_str: None,
        new_str: None,
    };
    let recipe = <ArtifactsToolRenderer as ToolRenderer<ArtifactsParams, ()>>::render(
        &renderer,
        Some(&params),
        None,
        false,
    )
    .unwrap();
    drop(params);
    renderer.artifacts_panel = None;
    drop(renderer);
    assert!(weak.upgrade().is_some());
    assert!(host.effects.borrow().is_empty());
    drop(recipe);
    assert!(weak.upgrade().is_none());
}
