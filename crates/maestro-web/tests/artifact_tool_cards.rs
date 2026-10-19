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

#[test]
fn artifact_cards_cover_command_status_and_parameter_branches() {
    use maestro_web::{ArtifactsToolRenderer, ToolRenderer};
    use serde_json::{Value, json};
    let renderer = ArtifactsToolRenderer::new(None);
    for command in [
        "create", "rewrite", "update", "get", "logs", "delete", "unknown", "",
    ] {
        for streaming in [false, true] {
            let params = json!({"command": command, "filename": "a.txt", "content": "literal", "old_str": "old", "new_str": "new"});
            let recipe = <ArtifactsToolRenderer as ToolRenderer<Value, ()>>::render(
                &renderer,
                Some(&params),
                None,
                streaming,
            )
            .unwrap();
            assert!(!recipe.is_custom);
        }
    }
}
