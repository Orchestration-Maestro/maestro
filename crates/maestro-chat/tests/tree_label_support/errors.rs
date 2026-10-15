//! Native errors reached through panel construction and fallible rendering.
use super::*;
/// One recorded reached-error operand.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    input: Input,
    expected: Expected,
}
/// Complete constructor and render operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    nodes: Vec<tree::Node>,
    roots: Vec<usize>,
    width: usize,
    leaf: Option<String>,
    height: usize,
}
/// Failure stage and native kind.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    stage: String,
    kind: String,
}
/// Exercises the original native failure at its recorded stage.
pub(super) fn replay(name: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/tree-panel-errors.json")).unwrap();
    let cases: Vec<_> = cases.iter().filter(|c| c.test == name).collect();
    assert_eq!(cases.len(), 1);
    for case in cases {
        let host = manual_runtime::ManualRuntime::new();
        let (_, theme) = published::published(&palette::palette("dark"));
        let panel = TreeSelectorComponent::new(
            &tree::tree(&case.input.nodes, &case.input.roots),
            TreeSelectorCallbacks {
                on_select: Rc::new(|_| {}),
                on_cancel: Rc::new(|| {}),
                on_label_change: None,
            },
            TreeSelectorOptions {
                current_leaf_id: case.input.leaf.clone(),
                terminal_height: case.input.height,
                initial_selected_id: None,
                initial_filter_mode: None,
                theme,
                runtime: host.handle(),
                styles_enabled: true,
                now: None,
            },
        );
        let error = match case.expected.stage.as_str() {
            "constructor" => panel.err().unwrap(),
            "render" => *panel
                .unwrap()
                .try_render(case.input.width)
                .unwrap_err()
                .downcast::<std::io::Error>()
                .unwrap(),
            _ => panic!("unknown failure stage"),
        };
        assert_eq!(case.expected.kind, "InvalidData");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
}
