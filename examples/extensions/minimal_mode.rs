//! Opt-in minimal tool rows with full expanded output.
// maestro-component: MAESTRO_MINIMAL_MODE_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    Component, ExtensionAPI, ExtensionFuture, ExtensionResult, Presence, Text, Theme, Tool,
    ToolDefinition, ToolMetadata, ToolName, ToolRenderResultLike, ToolRenderResultOptions,
    ToolsOptions, UserBlock, tools::path_utils, utils,
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
fn display_path(args: &Value, home: &str, fallback: &str, separator: char) -> String {
    let path = args["path"]
        .as_str()
        .filter(|path| !path.is_empty())
        .unwrap_or(fallback);
    match path.strip_prefix(home) {
        Some(suffix) if suffix.is_empty() || suffix.starts_with(['/', separator]) => {
            format!("~{suffix}")
        }
        _ => path.into(),
    }
}

/// Applies the native semantic title style to one complete call label.
fn title(theme: &Theme, text: &str) -> ExtensionResult<String> {
    theme.fg("toolTitle", &theme.bold(text)?)
}

/// Returns the displayed label for a native tool identity.
fn tool_label(name: ToolName) -> &'static str {
    match name {
        ToolName::Read => "read",
        ToolName::Bash => "bash",
        ToolName::Write => "write",
        ToolName::Edit => "edit",
        ToolName::Find => "find",
        ToolName::Grep => "grep",
        ToolName::Ls => "ls",
    }
}

/// Formats file operands, optional read bounds and the written line count.
fn file_call(
    name: ToolName,
    args: &Value,
    theme: &Theme,
    home: &str,
    separator: char,
) -> ExtensionResult<String> {
    let path = display_path(args, home, "", separator);
    let path = if path.is_empty() {
        theme.fg("toolOutput", "...")?
    } else {
        theme.fg("accent", &path)?
    };
    let mut text = format!("{} {path}", title(theme, tool_label(name))?);
    if name == ToolName::Read && (args.get("offset").is_some() || args.get("limit").is_some()) {
        let start = args["offset"].as_f64().unwrap_or(1.0);
        let end = args["limit"].as_f64().map(|limit| start + limit - 1.0);
        let end = end
            .filter(|end| *end != 0.0)
            .map_or_else(String::new, |end| format!("-{end}"));
        text.push_str(&theme.fg("warning", &format!(":{start}{end}"))?);
    }
    if name == ToolName::Write {
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
fn search_call(
    name: ToolName,
    args: &Value,
    theme: &Theme,
    home: &str,
    separator: char,
) -> ExtensionResult<String> {
    let path = display_path(args, home, ".", separator);
    let mut text = title(theme, tool_label(name))?;
    if name == ToolName::Ls {
        text.push(' ');
        text.push_str(&theme.fg("accent", &path)?);
    } else {
        let pattern = args["pattern"].as_str().unwrap_or("");
        let pattern = if name == ToolName::Grep {
            format!("/{pattern}/")
        } else {
            pattern.into()
        };
        text.push(' ');
        text.push_str(&theme.fg("accent", &pattern)?);
        text.push_str(&theme.fg("toolOutput", &format!(" in {path}"))?);
        if name == ToolName::Grep
            && let Some(glob) = args["glob"].as_str().filter(|glob| !glob.is_empty())
        {
            text.push_str(&theme.fg("toolOutput", &format!(" ({glob})"))?);
        }
    }
    if let Some(limit) = args.get("limit") {
        let suffix = if name == ToolName::Grep {
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
    name: ToolName,
    args: Option<Value>,
    theme: &Theme,
    home: &str,
    separator: char,
) -> ExtensionResult<Component> {
    let args = args.unwrap_or(Value::Null);
    let text = match name {
        ToolName::Bash => {
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
        ToolName::Read | ToolName::Write | ToolName::Edit => {
            file_call(name, &args, theme, home, separator)?
        }
        ToolName::Find | ToolName::Grep | ToolName::Ls => {
            search_call(name, &args, theme, home, separator)?
        }
    };
    view(&text)
}

/// Trims with the native text owner's whitespace definition.
fn trimmed<'a>(api: &ExtensionAPI, mut text: &'a str) -> ExtensionResult<&'a str> {
    while let Some(ch) = text.chars().next() {
        if !utils::is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        text = &text[ch.len_utf8()..];
    }
    while let Some(ch) = text.chars().next_back() {
        if !utils::is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        text = &text[..text.len() - ch.len_utf8()];
    }
    Ok(text)
}

/// Paints each output line independently, retaining the leading blank row.
fn output_lines(text: &str, theme: &Theme) -> ExtensionResult<Component> {
    let output = text
        .split('\n')
        .map(|line| theme.fg("toolOutput", line))
        .collect::<ExtensionResult<Vec<_>>>()?
        .join("\n");
    view(&format!("\n{output}"))
}

/// Hides file and shell output; searches and listings retain their nonempty row count.
fn collapsed(
    name: ToolName,
    text: &str,
    api: &ExtensionAPI,
    theme: &Theme,
) -> ExtensionResult<Component> {
    let noun = match name {
        ToolName::Find => "files",
        ToolName::Grep => "matches",
        ToolName::Ls => "entries",
        ToolName::Read | ToolName::Bash | ToolName::Write | ToolName::Edit => return view(""),
    };
    let count = trimmed(api, text)?
        .split('\n')
        .filter(|line| !line.is_empty())
        .count();
    if count == 0 {
        return view("");
    }
    view(&theme.fg("muted", &format!(" → {count} {noun}"))?)
}

/// Expands the first text block without introducing another output limit.
fn render_result(
    name: ToolName,
    result: &ToolRenderResultLike,
    options: ToolRenderResultOptions,
    theme: &Theme,
    api: &ExtensionAPI,
) -> ExtensionResult<Component> {
    let Some(text) = result.result.content.iter().find_map(|block| {
        if let UserBlock::Text(content) = block {
            Some(content.text.as_str())
        } else {
            None
        }
    }) else {
        return view("");
    };
    if !options.expanded {
        return collapsed(name, text, api, theme);
    }
    match name {
        ToolName::Read => output_lines(text, theme),
        ToolName::Write | ToolName::Edit => {
            if name == ToolName::Write && text.is_empty() {
                return view("");
            }
            let role = if result.is_error.present() == Some(&true) {
                "error"
            } else {
                "toolOutput"
            };
            view(&format!("\n{}", theme.fg(role, text)?))
        }
        ToolName::Bash | ToolName::Find | ToolName::Grep | ToolName::Ls => {
            let text = trimmed(api, text)?;
            if name == ToolName::Bash && text.is_empty() {
                return view("");
            }
            output_lines(text, theme)
        }
    }
}

/// Wraps one original executable without retaining a caller context.
fn minimal_tool(
    api: ExtensionAPI,
    cache: Rc<ToolCache>,
    name: ToolName,
    original: &Tool,
    separator: char,
) -> ToolDefinition {
    let call_api = api.clone();
    let result_api = api.clone();
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
            render_call(name, args, &theme, &home, separator)
        })),
        render_result: Some(Rc::new(move |result, options, theme, _, _| {
            render_result(name, &result, options, &theme, &result_api)
        })),
    }
}

/// Registers minimal displays while retaining the host's built-in execution.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let cache = Rc::new(RefCell::new(HashMap::new()));
        let separator = path_utils::resolve(&api, &["/"])?
            .chars()
            .next_back()
            .ok_or("native root is empty")?;
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
            api.register_tool(minimal_tool(
                api.clone(),
                cache.clone(),
                name,
                original,
                separator,
            ))?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
