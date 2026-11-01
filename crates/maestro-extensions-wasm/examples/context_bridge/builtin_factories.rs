//! Observes all individual native factory projections through the author facade.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    BashToolOptions, EditToolOptions, ExtensionAPI, ExtensionResult, FindToolOptions,
    GrepToolOptions, LsToolOptions, ReadToolOptions, Tool, ToolDefinition, ToolsOptions,
    WriteToolOptions,
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

/// Returns each selector's unmodified error and admits a separate custom-named definition.
pub(super) fn unknown(api: &ExtensionAPI, request: &str) -> ExtensionResult<String> {
    let names: Vec<String> = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let effects = super::builtin_selection::Effects::default();
    let mut errors = Vec::new();
    for name in names {
        let executable = api.create_tool(
            &name,
            "/absent-selector-root".into(),
            super::builtin_selection::options(&effects),
        );
        let definition = api.create_tool_definition(
            &name,
            "/absent-selector-root".into(),
            super::builtin_selection::options(&effects),
        );
        errors.push(json!([executable.err(), definition.err()]));
    }
    let mut custom = api
        .create_read_tool_definition("/absent-selector-root".into(), ReadToolOptions::default())?;
    custom.metadata.name = "custom 雪".into();
    api.register_tool(custom)?;
    Ok(json!({"errors": errors, "effects": *effects.borrow()}).to_string())
}

/// Observes ordered identities from every individual, selector and preset form.
pub(super) fn membership(api: &ExtensionAPI, cwd: &str) -> ExtensionResult<String> {
    let individual: Vec<_> = individual(api, cwd)?
        .into_iter()
        .map(|(tool, definition)| json!([tool.name, definition.metadata.name]))
        .collect();
    let mut selected = Vec::new();
    for name in ["read", "bash", "edit", "write", "grep", "find", "ls"] {
        selected.push(json!([
            api.create_tool(name, cwd.into(), ToolsOptions::default())?
                .name,
            api.create_tool_definition(name, cwd.into(), ToolsOptions::default())?
                .metadata
                .name
        ]));
    }
    let mut observation = presets(api, cwd)?;
    observation["individual"] = json!(individual);
    observation["selected"] = json!(selected);
    observation["names"] =
        json!(maestro_extensions_wasm::ALL_TOOL_NAMES.map(|name| format!("{name:?}")));
    Ok(observation.to_string())
}

/// Keeps ordered observations from both list and keyed native preset returns.
fn presets(api: &ExtensionAPI, cwd: &str) -> ExtensionResult<Value> {
    Ok(json!({
        "coding": tool_names(api.create_coding_tools(cwd.into(), ToolsOptions::default())?),
        "codingDefinitions": definition_names(api.create_coding_tool_definitions(cwd.into(), ToolsOptions::default())?),
        "readOnly": tool_names(api.create_read_only_tools(cwd.into(), ToolsOptions::default())?),
        "readOnlyDefinitions": definition_names(api.create_read_only_tool_definitions(cwd.into(), ToolsOptions::default())?),
        "all": api.create_all_tools(cwd.into(), ToolsOptions::default())?.into_iter().map(|(key, tool)| json!([format!("{key:?}"),tool.name])).collect::<Vec<_>>(),
        "allDefinitions": api.create_all_tool_definitions(cwd.into(), ToolsOptions::default())?.into_iter().map(|(key, tool)| json!([format!("{key:?}"),tool.metadata.name])).collect::<Vec<_>>(),
    }))
}

/// Projects executable names without sorting or set comparison.
fn tool_names(tools: Vec<Tool>) -> Vec<String> {
    tools.into_iter().map(|tool| tool.name).collect()
}

/// Projects definition names without sorting or set comparison.
fn definition_names(tools: Vec<ToolDefinition>) -> Vec<String> {
    tools.into_iter().map(|tool| tool.metadata.name).collect()
}

/// Closes every construction owner before observing the complete admission transcript.
pub(super) async fn fresh(
    api: &ExtensionAPI,
    context: &maestro_extensions_wasm::ExtensionCommandContext,
    cwd: &str,
) -> ExtensionResult<String> {
    let effects = super::builtin_selection::Effects::default();
    for _ in 0..2 {
        drop(individual(api, cwd)?);
        drop(membership(api, cwd)?);
        drop(configured(api, cwd, &effects)?);
        for name in ["read", "bash", "edit", "write", "grep", "find", "ls"] {
            drop(api.create_tool(
                name,
                cwd.into(),
                super::builtin_selection::options(&effects),
            )?);
            drop(api.create_tool_definition(
                name,
                cwd.into(),
                super::builtin_selection::options(&effects),
            )?);
        }
        configured_presets(api, cwd, &effects)?;
    }
    let closed = effects.borrow().clone();
    let first = super::builtin_selection::Effects::default();
    let second = super::builtin_selection::Effects::default();
    let tools = configured(api, cwd, &first)?;
    drop(configured(api, cwd, &second)?);
    let failed = exercise_fresh(tools, context).await;
    Ok(
        json!({"construction":closed,"first":*first.borrow(),"second":*second.borrow(),
        "failed":failed})
        .to_string(),
    )
}

/// Executes retained pairs only after the other complete set has been released.
async fn exercise_fresh(
    tools: Vec<(Tool, ToolDefinition)>,
    context: &maestro_extensions_wasm::ExtensionCommandContext,
) -> Vec<bool> {
    let args = [
        json!({"path":"fresh.txt"}),
        json!({"command":"unused"}),
        json!({"path":"fresh.txt","edits":[{"oldText":"a","newText":"b"}]}),
        json!({"path":"fresh.txt","content":"unused"}),
        json!({"pattern":"unused","path":"fresh.txt"}),
        json!({"pattern":"unused","path":"fresh.txt"}),
        json!({"path":"fresh.txt"}),
    ];
    let mut failed = Vec::new();
    for ((tool, definition), args) in tools.into_iter().zip(args) {
        failed.push(
            (tool.execute)("fresh-tool".into(), args.clone(), None, None)
                .await
                .is_err(),
        );
        drop(tool);
        failed.push(
            (definition.execute)(
                "fresh-definition".into(),
                args,
                None,
                None,
                (**context).clone(),
            )
            .await
            .is_err(),
        );
        drop(definition);
    }
    failed
}

/// Independent retained read owners outlive the other construction's release.
fn fresh_read(
    api: &ExtensionAPI,
    cwd: &str,
    effects: &super::builtin_selection::Effects,
) -> ExtensionResult<(Tool, ToolDefinition)> {
    Ok((
        api.create_read_tool(cwd.into(), super::builtin_selection::options(effects).read)?,
        api.create_read_tool_definition(
            cwd.into(),
            super::builtin_selection::options(effects).read,
        )?,
    ))
}

/// Supplies every callback to individual factories without invoking it.
fn configured(
    api: &ExtensionAPI,
    cwd: &str,
    effects: &super::builtin_selection::Effects,
) -> ExtensionResult<Vec<(Tool, ToolDefinition)>> {
    use super::builtin_selection::options;
    macro_rules! pair {
        ($tool:ident, $definition:ident, $field:ident) => {
            (
                api.$tool(cwd.into(), options(effects).$field)?,
                api.$definition(cwd.into(), options(effects).$field)?,
            )
        };
    }
    Ok(vec![
        fresh_read(api, cwd, effects)?,
        pair!(create_bash_tool, create_bash_tool_definition, bash),
        pair!(create_edit_tool, create_edit_tool_definition, edit),
        pair!(create_write_tool, create_write_tool_definition, write),
        pair!(create_grep_tool, create_grep_tool_definition, grep),
        pair!(create_find_tool, create_find_tool_definition, find),
        pair!(create_ls_tool, create_ls_tool_definition, ls),
    ])
}

/// Every native preset retains the same all-family forbidden effect transcript.
fn configured_presets(
    api: &ExtensionAPI,
    cwd: &str,
    effects: &super::builtin_selection::Effects,
) -> ExtensionResult<()> {
    use super::builtin_selection::options;
    drop(api.create_coding_tools(cwd.into(), options(effects))?);
    drop(api.create_coding_tool_definitions(cwd.into(), options(effects))?);
    drop(api.create_read_only_tools(cwd.into(), options(effects))?);
    drop(api.create_read_only_tool_definitions(cwd.into(), options(effects))?);
    drop(api.create_all_tools(cwd.into(), options(effects))?);
    drop(api.create_all_tool_definitions(cwd.into(), options(effects))?);
    Ok(())
}
