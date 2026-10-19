//! Default tool rows on the native frame: subsections leave independently of the header.
#![cfg(test)]
use super::{header_controls::Canvas, message, reset};
use maestro_web::{
    render_tool,
    tools::types::{ToolParams, ToolRenderState},
};
use serde_json::{Value, json};

/// Draw the default recipe for one call; a missing result means the call is streaming.
fn frame(
    canvas: &mut Canvas,
    row: &mut ToolRenderState,
    params: Option<&Value>,
    result: Option<&maestro_app::ToolResultMessage<Value>>,
) {
    let recipe = render_tool(
        "unregistered",
        params.map(ToolParams::json),
        result,
        result.is_none(),
    )
    .unwrap();
    assert!(!recipe.is_custom);
    canvas.draw(&recipe, row, vec![]);
}

#[test]
fn default_rows_release_only_the_subsections_that_disappear() {
    reset();
    let params = json!({"path": "a.txt"});
    let result = message::<Value>(None);
    let mut canvas = Canvas::new();
    let mut row = ToolRenderState::default();
    let removed = |canvas: &Canvas| canvas.host.removed.borrow().len();
    for _ in 0..2 {
        frame(&mut canvas, &mut row, Some(&params), Some(&result));
    }
    assert_eq!(removed(&canvas), 0, "a redrawn row keeps every section");
    frame(&mut canvas, &mut row, Some(&params), None);
    assert_eq!(
        removed(&canvas),
        2,
        "output label and listing leave together"
    );
    frame(&mut canvas, &mut row, None, None);
    assert_eq!(removed(&canvas), 4, "input leaves while the header stays");
    drop(row);
    let removed = canvas.host.removed.borrow();
    assert_eq!(removed.len(), 5, "the header caption leaves with its row");
    let distinct: std::collections::HashSet<_> = removed.iter().collect();
    assert_eq!(distinct.len(), 5);
}
