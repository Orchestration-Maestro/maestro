//! Opt-in minimal tool rows with full expanded output.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    Component, ExtensionAPI, ExtensionFuture, ExtensionResult, Presence, Text, Theme, Tool,
    ToolDefinition, ToolMetadata, ToolName, ToolsOptions, tools::path_utils,
};
use serde_json::Value;
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

/// Builds an unpadded widget owned by the existing host.
fn view(text: &str) -> ExtensionResult<Component> {
    Ok((&Text::new(text, 0, 0, None)?).into())
}

/// Abbreviates the home directory, not another directory with the same prefix.
fn display_path(args: &Value, home: &str, fallback: &str) -> String {
    let path = args["path"]
        .as_str()
        .filter(|path| !path.is_empty())
        .unwrap_or(fallback);
    match path.strip_prefix(home) {
        Some(suffix) if suffix.is_empty() || suffix.starts_with(['/', '\\']) => {
            format!("~{suffix}")
        }
        _ => path.into(),
    }
}

/// Applies the native semantic title style to one complete call label.
fn title(theme: &Theme, text: &str) -> ExtensionResult<String> {
    theme.fg("toolTitle", &theme.bold(text)?)
}

/// Formats file operands, optional read bounds and the written line count.
fn file_call(name: &str, args: &Value, theme: &Theme, home: &str) -> ExtensionResult<String> {
    let path = display_path(args, home, "");
    let path = if path.is_empty() {
        theme.fg("toolOutput", "...")?
    } else {
        theme.fg("accent", &path)?
    };
    let mut text = format!("{} {path}", title(theme, name)?);
    if name == "read" && (args.get("offset").is_some() || args.get("limit").is_some()) {
        let start = args["offset"].as_f64().unwrap_or(1.0);
        let end = args["limit"].as_f64().map(|limit| start + limit - 1.0);
        let end = end
            .filter(|end| *end != 0.0)
            .map_or_else(String::new, |end| format!("-{end}"));
        text.push_str(&theme.fg("warning", &format!(":{start}{end}"))?);
    }
    if name == "write" {
        let count = args["content"]
            .as_str()
            .filter(|text| !text.is_empty())
            .map_or(0, |text| text.split('\n').count());
        if count > 0 {
            text.push_str(&theme.fg("muted", &format!(" ({count} lines)"))?);
        }
    }
    Ok(text)
}

/// Formats search patterns, authored paths, glob filters and numeric limits.
fn search_call(name: &str, args: &Value, theme: &Theme, home: &str) -> ExtensionResult<String> {
    let path = display_path(args, home, ".");
    let mut text = title(theme, name)?;
    if name == "ls" {
        text.push(' ');
        text.push_str(&theme.fg("accent", &path)?);
    } else {
        let pattern = args["pattern"].as_str().unwrap_or("");
        let pattern = if name == "grep" {
            format!("/{pattern}/")
        } else {
            pattern.into()
        };
        text.push(' ');
        text.push_str(&theme.fg("accent", &pattern)?);
        text.push_str(&theme.fg("toolOutput", &format!(" in {path}"))?);
        if name == "grep"
            && let Some(glob) = args["glob"].as_str().filter(|glob| !glob.is_empty())
        {
            text.push_str(&theme.fg("toolOutput", &format!(" ({glob})"))?);
        }
    }
    if let Some(limit) = args.get("limit") {
        let suffix = if name == "grep" {
            format!(" limit {limit}")
        } else {
            format!(" (limit {limit})")
        };
        text.push_str(&theme.fg("toolOutput", &suffix)?);
    }
    Ok(text)
}

/// Selects only the display policy; built-in execution stays with the cached tool.
fn render_call(
    name: &str,
    args: Option<Value>,
    theme: &Theme,
    home: &str,
) -> ExtensionResult<Component> {
    let args = args.unwrap_or(Value::Null);
    let text = match name {
        "bash" => {
            let command = args["command"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("...");
            let mut text = title(theme, &format!("$ {command}"))?;
            if let Some(timeout) = args["timeout"].as_f64().filter(|timeout| *timeout != 0.0) {
                text.push_str(&theme.fg("muted", &format!(" (timeout {timeout}s)"))?);
            }
            text
        }
        "read" | "write" | "edit" => file_call(name, &args, theme, home)?,
        _ => search_call(name, &args, theme, home)?,
    };
    view(&text)
}

/// Wraps one original executable without retaining a caller context.
fn minimal_tool(
    api: ExtensionAPI,
    cache: Rc<ToolCache>,
    name: ToolName,
    original: &Tool,
) -> ToolDefinition {
    let label = original.name.clone();
    let call_api = api.clone();
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
        render_call: Some(Rc::new(move |args, theme, _, _| {
            let home = path_utils::expand_path(&call_api, "~")?;
            render_call(&label, args, &theme, &home)
        })),
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
