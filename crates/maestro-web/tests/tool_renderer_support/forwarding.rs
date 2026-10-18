//! Complete recorded operands observed at the selected native callback.
#![cfg(test)]
use super::{cases, content};
use maestro_web::{
    ToolRenderer,
    browser::attachment_content::AttachmentUiError,
    register_tool_renderer, render_tool,
    tools::types::{ToolParams, ToolRenderResult, ToolRendererBinding, ToolResultMessageView},
};
use serde_json::Value;
use std::{cell::Cell, rc::Rc};
struct Forwarding {
    expected: Value,
    params: usize,
    details: Option<usize>,
    call_id: usize,
    blocks: usize,
    calls: Rc<Cell<usize>>,
}
impl ToolRenderer for Forwarding {
    fn render(
        &self,
        params: Option<&Value>,
        result: Option<ToolResultMessageView<'_>>,
        stream: bool,
    ) -> Result<ToolRenderResult, AttachmentUiError> {
        let result = result.unwrap();
        let expected = &self.expected;
        let same_params = params.map(|p| std::ptr::from_ref(p) as usize) == Some(self.params);
        let same_result = result.tool_call_id().as_ptr() as usize == self.call_id
            && result.content().as_ptr() as usize == self.blocks
            && result.details().map(|d| std::ptr::from_ref(d) as usize) == self.details;
        assert_eq!(same_params, expected["sameParams"].as_bool().unwrap());
        assert_eq!(same_result, expected["sameResult"].as_bool().unwrap());
        assert_eq!(params, Some(&expected["params"]));
        let message = &expected["result"];
        assert_eq!(result.role(), message["role"].as_str().unwrap());
        assert_eq!(
            result.tool_call_id(),
            message["toolCallId"].as_str().unwrap()
        );
        assert_eq!(result.tool_name(), message["toolName"].as_str().unwrap());
        assert_eq!(
            result.timestamp().to_bits(),
            message["timestamp"].as_f64().unwrap().to_bits()
        );
        assert_eq!(
            serde_json::to_value(result.content()).unwrap(),
            message["content"]
        );
        assert_eq!(result.details(), message.get("details"));
        assert_eq!(result.is_error(), message["isError"].as_bool().unwrap());
        assert_eq!(stream, expected["isStreaming"].as_bool().unwrap());
        self.calls.set(self.calls.get() + 1);
        Ok(content("forwarded", stream))
    }
}
/// Replay complete callback operands.
/// # Panics
/// Panics if any recorded field differs.
pub fn run() {
    let calls = Rc::new(Cell::new(0));
    let mut rows = cases()["forwarding"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 14);
    let mut normal = rows[0].clone();
    normal["input"]["result"]["isError"] = Value::Bool(false);
    normal["expected"]["result"]["isError"] = Value::Bool(false);
    rows.push(normal);
    for row in rows {
        let params = &row["input"]["params"];
        let mut message: maestro_app::ToolResultMessage =
            serde_json::from_value(row["input"]["result"].clone()).unwrap();
        message.details = row["input"]["result"].get("details").cloned();
        let renderer = Forwarding {
            expected: row["expected"].clone(),
            params: std::ptr::from_ref(params) as usize,
            details: message
                .details
                .as_ref()
                .map(|d| std::ptr::from_ref(d) as usize),
            call_id: message.tool_call_id.as_ptr() as usize,
            blocks: message.content.as_ptr() as usize,
            calls: calls.clone(),
        };
        register_tool_renderer("complete", ToolRendererBinding::from_raw(Rc::new(renderer)));
        let prepared = render_tool(
            "complete",
            Some(ToolParams::json(params)),
            Some(&message),
            row["input"]["isStreaming"].as_bool().unwrap(),
        )
        .unwrap();
        assert_eq!(
            prepared.is_custom,
            row["expected"]["isCustom"].as_bool().unwrap()
        );
    }
    assert_eq!(calls.get(), 15);
}
