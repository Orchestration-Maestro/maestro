//! Shared identity of native tool records.
use maestro_agent::{AgentToolResult, ToolExecutionMode};

#[test]
fn native_tool_aliases_use_shared_records() {
    /// Non-default caller data.
    struct Details;
    let module_result: maestro_agent::types::AgentToolResult<Details> = AgentToolResult {
        content: vec![],
        details: Some(Details),
        terminate: Some(false),
        usage: None,
    };
    let shared: maestro_tool::types::AgentToolResult<Option<Details>, Option<bool>> = module_result;
    assert!(shared.details.is_some());
    assert_eq!(shared.terminate, Some(false));
    let mode: maestro_tool::types::ToolExecutionMode = ToolExecutionMode::Parallel;
    let copied = mode;
    assert_eq!(mode, copied);
    let module_mode: maestro_agent::types::ToolExecutionMode = copied;
    assert_eq!(module_mode, ToolExecutionMode::Parallel);
}
