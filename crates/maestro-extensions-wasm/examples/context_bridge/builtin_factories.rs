//! Observes all individual native factory projections through the author facade.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    BashToolOptions, EditToolOptions, ExtensionAPI, ExtensionResult, FindToolOptions,
    GrepToolOptions, LsToolOptions, ReadToolOptions, Tool, ToolDefinition, WriteToolOptions,
};
use serde_json::{Value, json};

/// Executes each context-free projection and admits its independently constructed definition.
pub(super) async fn wrapped(api: &ExtensionAPI, request: &str) -> ExtensionResult<String> {
    let request: Value = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let cwd = request["cwd"].as_str().ok_or("missing factory cwd")?;
    let tools = individual(api, cwd)?;
    let mut observations = Vec::new();
    for (index, (tool, mut definition)) in tools.into_iter().enumerate() {
        let result = (tool.execute)(
            "wrapped-factory".into(),
            request["args"][index].clone(),
            None,
            None,
        )
        .await;
        let mut observation = json!({
            "metadata": super::builtin::metadata(&tool)?,
            "prepares": tool.prepare_arguments.is_some(),
            "definition": definition.metadata,
            "definitionPrepares": definition.prepare_arguments.is_some(),
            "renderers": [definition.render_call.is_some(), definition.render_result.is_some()],
        });
        match result {
            Ok(result) => {
                observation["result"] =
                    serde_json::to_value(result).map_err(|error| error.to_string())?;
            }
            Err(error) => observation["error"] = json!(error),
        }
        definition.metadata.name = format!("factory-{}", tool.name);
        api.register_tool(definition)?;
        observations.push(observation);
    }
    Ok(json!(observations).to_string())
}

/// Runs repeated default native effects without installing replacement operations.
pub(super) async fn defaults(api: &ExtensionAPI, request: &str) -> ExtensionResult<String> {
    let request: Value = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let cwd = request["cwd"].as_str().ok_or("missing default cwd")?;
    let tool = match request["tool"].as_str() {
        Some("write") => api.create_write_tool(cwd.into(), WriteToolOptions::default())?,
        Some("find") => api.create_find_tool(cwd.into(), FindToolOptions::default())?,
        _ => return Err("unknown default observation".into()),
    };
    let mut results = Vec::new();
    for args in request["args"]
        .as_array()
        .ok_or("missing default arguments")?
    {
        results.push((tool.execute)("default-factory".into(), args.clone(), None, None).await?);
    }
    serde_json::to_string(&results).map_err(|error| error.to_string())
}

/// Constructs every pair without probing paths or requiring a callback context.
fn individual(api: &ExtensionAPI, cwd: &str) -> ExtensionResult<Vec<(Tool, ToolDefinition)>> {
    Ok(vec![
        (
            api.create_read_tool(cwd.into(), ReadToolOptions::default())?,
            api.create_read_tool_definition(cwd.into(), ReadToolOptions::default())?,
        ),
        (
            api.create_bash_tool(cwd.into(), BashToolOptions::default())?,
            api.create_bash_tool_definition(cwd.into(), BashToolOptions::default())?,
        ),
        (
            api.create_edit_tool(cwd.into(), EditToolOptions::default())?,
            api.create_edit_tool_definition(cwd.into(), EditToolOptions::default())?,
        ),
        (
            api.create_write_tool(cwd.into(), WriteToolOptions::default())?,
            api.create_write_tool_definition(cwd.into(), WriteToolOptions::default())?,
        ),
        (
            api.create_grep_tool(cwd.into(), GrepToolOptions::default())?,
            api.create_grep_tool_definition(cwd.into(), GrepToolOptions::default())?,
        ),
        (
            api.create_find_tool(cwd.into(), FindToolOptions::default())?,
            api.create_find_tool_definition(cwd.into(), FindToolOptions::default())?,
        ),
        (
            api.create_ls_tool(cwd.into(), LsToolOptions::default())?,
            api.create_ls_tool_definition(cwd.into(), LsToolOptions::default())?,
        ),
    ])
}
