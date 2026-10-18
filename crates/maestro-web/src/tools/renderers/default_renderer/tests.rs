//! Recorded default-rendering branches through pure preparation.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    rows: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    input: Input,
    expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    params: Option<Value>,
    result: Option<maestro_app::ToolResultMessage>,
    is_streaming: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    is_custom: bool,
    header: String,
    spinner: usize,
    icon_class: String,
    blocks: Vec<Block>,
    labels: Vec<String>,
}
#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct Block {
    code: String,
    language: String,
}

#[test]
fn default_preparation_matches_recorded_inputs_and_results() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/tool_renderers.json"
    ))
    .unwrap();
    assert_eq!(fixture.rows.len(), 95);
    for mut row in fixture.rows {
        if row.id.starts_with("negative-zero-") {
            row.input.params = Some(serde_json::from_str("-0.0").unwrap());
        }
        let result = row.input.result.as_ref().map(ToolResultMessageView::from);
        let selected = status(result.as_ref(), row.input.is_streaming);
        let params = row
            .input
            .params
            .as_ref()
            .map(format_params)
            .transpose()
            .unwrap()
            .flatten();
        let output = result.as_ref().map(|r| get_text_output(Some(r)));
        let prepared = prepare(
            params.as_deref(),
            output.as_deref(),
            selected,
            "(no output)",
        )
        .unwrap();
        assert!(!row.expected.is_custom, "{}", row.id);
        assert_eq!(prepared.caption, row.expected.header, "{}", row.id);
        assert_eq!(
            usize::from(selected == ToolHeaderState::Inprogress),
            row.expected.spinner,
            "{}",
            row.id
        );
        let class = match selected {
            ToolHeaderState::Inprogress => "inline-block text-foreground",
            ToolHeaderState::Complete => "inline-block text-green-600 dark:text-green-500",
            ToolHeaderState::Error => "inline-block text-destructive",
        };
        assert_eq!(class, row.expected.icon_class, "{}", row.id);
        let mut labels = Vec::new();
        let mut blocks = Vec::new();
        for (label, block) in [("Input", prepared.input), ("Output", prepared.output)] {
            if let Some(block) = block {
                labels.push(label);
                blocks.push(Block {
                    code: block.code,
                    language: block.language.into(),
                });
            }
        }
        assert_eq!(labels, row.expected.labels, "{}", row.id);
        assert_eq!(blocks, row.expected.blocks, "{}", row.id);
    }
}
