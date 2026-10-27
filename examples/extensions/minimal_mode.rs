//! Opt-in minimal tool rows with full expanded output.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionFuture, ExtensionResult, Presence, Tool, ToolDefinition, ToolMetadata,
    ToolName, ToolsOptions, tools::path_utils,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Original built-in executables in their native insertion order.
type BuiltInTools = indexmap::IndexMap<ToolName, Tool>;

/// Original built-in executables retained once per authored working directory.
type ToolCache = RefCell<HashMap<String, Rc<BuiltInTools>>>;

/// Returns the native owners for a directory without holding a borrow across host calls.
fn builtin_tools(
    api: &ExtensionAPI,
    cache: &ToolCache,
    cwd: String,
) -> ExtensionResult<Rc<BuiltInTools>> {
    if let Some(tools) = cache.borrow().get(&cwd).cloned() {
        return Ok(tools);
    }
    let tools = Rc::new(api.create_all_tools(cwd.clone(), ToolsOptions::default())?);
    cache.borrow_mut().insert(cwd, Rc::clone(&tools));
    Ok(tools)
}

/// Wraps one original executable without retaining a caller context.
fn minimal_tool(
    api: ExtensionAPI,
    cache: Rc<ToolCache>,
    name: ToolName,
    original: &Tool,
) -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: original.name.clone(),
            label: original.label.clone(),
            description: original.description.clone(),
            parameters: original.parameters.clone(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: original.execution_mode,
        },
        prepare_arguments: original.prepare_arguments.clone(),
        execute: Rc::new(move |id, params, signal, update, ctx| {
            let (api, cache) = (api.clone(), cache.clone());
            Box::pin(async move {
                let tools = builtin_tools(&api, &cache, ctx.cwd()?)?;
                let tool = tools.get(&name).ok_or("missing native tool")?;
                (tool.execute)(id, params, signal, update).await
            })
        }),
        render_call: None,
        render_result: None,
    }
}

/// Registers minimal displays while retaining the host's built-in execution.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let cache = Rc::new(RefCell::new(HashMap::new()));
        let initial = builtin_tools(&api, &cache, path_utils::resolve(&api, &["."])?)?;
        for name in [
            ToolName::Read,
            ToolName::Bash,
            ToolName::Write,
            ToolName::Edit,
            ToolName::Find,
            ToolName::Grep,
            ToolName::Ls,
        ] {
            let original = initial.get(&name).ok_or("missing native tool")?;
            api.register_tool(minimal_tool(api.clone(), cache.clone(), name, original))?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
