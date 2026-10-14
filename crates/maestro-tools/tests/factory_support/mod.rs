//! Test-local typed selection and independently recorded factory observations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

mod configuration;
mod corpus;
mod execution;
mod rendering;
pub use corpus::{check_metadata, check_order, check_preparation, check_routes};
pub use execution::check_corrections;

/// Execution and rendering through selected native definitions.
pub fn check_operations() {
    execution::check_operations();
    rendering::check();
    configuration::check();
}

use maestro_models::DiagnosticErrorInfo;
use maestro_tools::*;
use std::sync::Arc;

/// Wrap only at the existing executable boundary, retaining typed definitions until then.
pub fn wrap<C: 'static>(tool: ToolDef<C>, context: Option<ToolContextFactory<C>>) -> Tool {
    match tool {
        ToolDef::Read(t) => wrap_tool_definition(t, context),
        ToolDef::Bash(t) => wrap_tool_definition(t, context),
        ToolDef::Edit(t) => wrap_tool_definition(t, context),
        ToolDef::Write(t) => wrap_tool_definition(t, context),
        ToolDef::Grep(t) => wrap_tool_definition(t, context),
        ToolDef::Find(t) => wrap_tool_definition(t, context),
        ToolDef::Ls(t) => wrap_tool_definition(t, context),
    }
}

/// Select a public factory without another production dispatch layer.
pub fn select(
    factory: &str,
    name: &str,
    cwd: String,
    options: ToolsOptions,
    manager: Arc<ToolsManager>,
) -> Result<Vec<Tool>, DiagnosticErrorInfo> {
    select_with_bin(
        factory,
        name,
        options,
        (cwd, "/unused/bash".into()),
        manager,
    )
}

/// Select through the same caller with independently supplied shell and search handles.
fn select_with_bin(
    factory: &str,
    name: &str,
    options: ToolsOptions,
    directories: (String, String),
    manager: Arc<ToolsManager>,
) -> Result<Vec<Tool>, DiagnosticErrorInfo> {
    let (cwd, bin) = directories;
    Ok(match factory {
        "createTool" => vec![create_tool(name, cwd, options, bin, manager)?],
        "createToolDefinition" => vec![wrap(
            create_tool_definition::<()>(name, cwd, options, bin, manager)?,
            None,
        )],
        "createCodingTools" => create_coding_tools(cwd, options, bin, manager),
        "createReadOnlyTools" => create_read_only_tools(cwd, options, bin, manager),
        "createAllTools" => create_all_tools(cwd, options, bin, manager)
            .into_values()
            .collect(),
        "createCodingToolDefinitions" => {
            create_coding_tool_definitions::<()>(cwd, options, bin, manager)
                .into_iter()
                .map(|t| wrap(t, None))
                .collect()
        }
        "createReadOnlyToolDefinitions" => {
            create_read_only_tool_definitions::<()>(cwd, options, bin, manager)
                .into_iter()
                .map(|t| wrap(t, None))
                .collect()
        }
        "createAllToolDefinitions" => create_all_tool_definitions::<()>(cwd, options, bin, manager)
            .into_values()
            .map(|t| wrap(t, None))
            .collect(),
        _ => panic!("unknown fixture factory"),
    })
}

/// All callers containing the requested family.
pub fn factories(name: &str) -> Vec<&'static str> {
    let mut factories = vec![
        "createTool",
        "createToolDefinition",
        "createAllTools",
        "createAllToolDefinitions",
    ];
    if ["read", "bash", "edit", "write"].contains(&name) {
        factories.extend(["createCodingTools", "createCodingToolDefinitions"]);
    }
    if ["read", "grep", "find", "ls"].contains(&name) {
        factories.extend(["createReadOnlyTools", "createReadOnlyToolDefinitions"]);
    }
    factories
}

/// The individual executable comparator, deliberately independent of the barrel.
pub fn individual(
    name: &str,
    cwd: String,
    options: ToolsOptions,
    manager: Arc<ToolsManager>,
) -> Tool {
    match name {
        "read" => create_read_tool(cwd, options.read),
        "bash" => create_bash_tool(cwd, "/unused/bash".into(), options.bash),
        "edit" => create_edit_tool(cwd, options.edit),
        "write" => create_write_tool(cwd, options.write),
        "grep" => create_grep_tool(cwd, options.grep, manager),
        "find" => create_find_tool(cwd, options.find, manager),
        "ls" => create_ls_tool(cwd, options.ls),
        _ => panic!("unknown fixture name"),
    }
}
