//! A ripgrep tool that retains full output when the host's output limits are reached.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AgentToolResult, Component, ExtensionAPI, ExtensionFuture, ExtensionResult, Presence, Text,
    TextContent, Theme, ToolDefinition, ToolMetadata, ToolRenderResultLike,
    ToolRenderResultOptions, UserBlock,
    child_process::{SpawnOptions, Stdio},
    streams::{Readable, read_to_end},
    tools, utils,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::rc::Rc;

/// Search operands, also retained in the result details.
#[derive(Deserialize, Serialize)]
struct RgParams {
    /// Search pattern interpreted by ripgrep, not a shell.
    pattern: String,
    /// Search directory, defaulting to the current directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// Optional file-name filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    glob: Option<String>,
}

/// Returns one text block with the original search operands and match count.
fn result(
    params: &RgParams,
    text: String,
    count: usize,
    overflow: Option<(tools::TruncationResult, String)>,
) -> ExtensionResult<AgentToolResult> {
    let mut details = serde_json::to_value(params).map_err(|error| error.to_string())?;
    details["matchCount"] = json!(count);
    if let Some((truncation, path)) = overflow {
        details["truncation"] =
            serde_json::to_value(truncation).map_err(|error| error.to_string())?;
        details["fullOutputPath"] = json!(path);
    }
    Ok(AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(text))],
        details: Presence::Present(details.to_string().into()),
        terminate: Presence::Missing,
    })
}

/// Checks each scalar with the host's whitespace definition.
fn has_text(api: &ExtensionAPI, text: &str) -> ExtensionResult<bool> {
    for scalar in text.chars() {
        if !utils::is_whitespace_char(api, &scalar.to_string())? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Exit code and decoded output streams of one search process.
struct SearchOutput {
    /// Numeric exit code, absent when a signal ended the process.
    code: Option<i32>,
    /// Matches, decoded lossily.
    stdout: String,
    /// Diagnostics, decoded lossily.
    stderr: String,
}

/// Reads an optional output pipe, decoding its bytes lossily.
///
/// # Errors
/// Returns native pipe-read errors.
async fn read_text(pipe: Option<Rc<dyn Readable>>) -> ExtensionResult<String> {
    let bytes = match pipe {
        Some(pipe) => read_to_end(pipe).await?,
        None => Vec::new(),
    };
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Starts ripgrep once with literal search operands in the callback's supplied working
/// directory; a process that cannot start is an error.
async fn search_output(
    api: &ExtensionAPI,
    params: &RgParams,
    cwd: String,
) -> ExtensionResult<SearchOutput> {
    let mut args = vec!["--line-number".into(), "--color=never".into()];
    if let Some(glob) = params.glob.as_ref().filter(|glob| !glob.is_empty()) {
        args.extend(["--glob".into(), glob.clone()]);
    }
    args.extend([
        "--".into(),
        params.pattern.clone(),
        params
            .path
            .clone()
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| ".".into()),
    ]);
    let options = SpawnOptions {
        cwd: Some(cwd),
        stdio: [Stdio::Ignore, Stdio::Pipe, Stdio::Pipe],
        ..SpawnOptions::default()
    };
    let child = api.native_io().spawn("rg".into(), args, options).await?;
    let stdout = read_text(child.stdout()).await?;
    let stderr = read_text(child.stderr()).await?;
    let code = child.wait().await?.code;
    Ok(SearchOutput {
        code,
        stdout,
        stderr,
    })
}

/// Saves complete output in a private directory, awaiting the native file queue.
async fn save_output(api: &ExtensionAPI, output: String) -> ExtensionResult<String> {
    let directory = tools::path_utils::make_temp_dir(api, "maestro-rg-")?;
    let path = std::path::Path::new(&directory).join("output.txt");
    let path = path
        .to_str()
        .ok_or("temporary path is not valid UTF-8")?
        .to_owned();
    let target = path.clone();
    api.with_file_mutation_queue(path.clone(), move || {
        Box::pin(async move { std::fs::write(target, output).map_err(|error| error.to_string()) })
    })
    .await?;
    Ok(path)
}

/// Describes exactly which lines and bytes were omitted and where they were saved.
fn notice(
    api: &ExtensionAPI,
    truncation: &tools::TruncationResult,
    path: &str,
) -> ExtensionResult<String> {
    Ok(format!(
        "\n\n[Output truncated: showing {} of {} lines ({} of {}). {} lines ({}) omitted. Full output saved to: {path}]",
        truncation.output_lines,
        truncation.total_lines,
        tools::format_size(api, truncation.output_bytes)?,
        tools::format_size(api, truncation.total_bytes)?,
        truncation.total_lines - truncation.output_lines,
        tools::format_size(api, truncation.total_bytes - truncation.output_bytes)?,
    ))
}

/// Reports empty searches and failures, applying the host's output limits to matches.
async fn execute(
    api: ExtensionAPI,
    params: RgParams,
    cwd: String,
) -> ExtensionResult<AgentToolResult> {
    let output = search_output(&api, &params, cwd)
        .await
        .map_err(|error| format!("ripgrep failed: {error}"))?;
    if output.code == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
        return result(&params, "No matches found".into(), 0, None);
    }
    if output.code != Some(0) {
        return Err(format!("ripgrep failed: {}", output.stderr));
    }
    if !has_text(&api, &output.stdout)? {
        return result(&params, "No matches found".into(), 0, None);
    }
    let count = output.stdout.split('\n').try_fold(0, |count, line| {
        has_text(&api, line).map(|nonempty| count + usize::from(nonempty))
    })?;
    let truncation =
        tools::truncate_head(&api, &output.stdout, tools::TruncationOptions::default())?;
    let mut text = truncation.content.clone();
    let overflow = if truncation.truncated {
        let path = save_output(&api, output.stdout).await?;
        text.push_str(&notice(&api, &truncation, &path)?);
        Some((truncation, path))
    } else {
        None
    };
    result(&params, text, count, overflow)
}

/// Builds an unpadded host text widget.
fn view(text: &str) -> ExtensionResult<Component> {
    Ok((&Text::new(text, 0, 0, None)?).into())
}

/// Shows the pattern and optional directory and glob with the supplied theme.
fn render_call(args: Option<Value>, theme: &Theme) -> ExtensionResult<Component> {
    let args = args.unwrap_or(Value::Null);
    let mut text = theme.fg("toolTitle", &theme.bold("rg ")?)?;
    let pattern = args["pattern"].as_str().unwrap_or("undefined");
    text.push_str(&theme.fg("accent", &format!("\"{pattern}\""))?);
    if let Some(path) = args["path"].as_str().filter(|path| !path.is_empty()) {
        text.push_str(&theme.fg("muted", &format!(" in {path}"))?);
    }
    if let Some(glob) = args["glob"].as_str().filter(|glob| !glob.is_empty()) {
        text.push_str(&theme.fg("dim", &format!(" --glob {glob}"))?);
    }
    view(&text)
}

/// Shows search progress, match counts and up to twenty expanded content lines.
fn render_result(
    result: &ToolRenderResultLike,
    options: ToolRenderResultOptions,
    theme: &Theme,
) -> ExtensionResult<Component> {
    if options.is_partial {
        return view(&theme.fg("warning", "Searching...")?);
    }
    let details: Value = result
        .result
        .details
        .present()
        .map(|details| serde_json::from_str(details))
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or(Value::Null);
    let count = details["matchCount"].as_u64().unwrap_or(0);
    if count == 0 {
        return view(&theme.fg("dim", "No matches found")?);
    }
    let mut text = theme.fg("success", &format!("{count} matches"))?;
    if details["truncation"]["truncated"] == true {
        text.push_str(&theme.fg("warning", " (truncated)")?);
    }
    if options.expanded {
        if let Some(UserBlock::Text(content)) = result.result.content.first() {
            for line in content.text.split('\n').take(20) {
                text.push('\n');
                text.push_str(&theme.fg("dim", line)?);
            }
            if content.text.split('\n').count() > 20 {
                text.push('\n');
                text.push_str(&theme.fg("muted", "... (use read tool to see full output)")?);
            }
        }
        if let Some(path) = details["fullOutputPath"]
            .as_str()
            .filter(|path| !path.is_empty())
        {
            text.push('\n');
            text.push_str(&theme.fg("dim", &format!("Full output: {path}"))?);
        }
    }
    view(&text)
}

/// Registers the search tool through the supplied extension host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let executor = api.clone();
        api.register_tool(ToolDefinition {
            metadata: ToolMetadata {
                name: "rg".into(),
                label: "ripgrep".into(),
                description: "Search file contents using ripgrep. Output is truncated to 2000 lines or 50.0KB (whichever is hit first). If truncated, full output is saved to a temp file.".into(),
                parameters: json!({"type":"object","required":["pattern"],"properties":{
                    "pattern":{"type":"string","description":"Search pattern (regex)"},
                    "path":{"type":"string","description":"Directory to search (default: current directory)"},
                    "glob":{"type":"string","description":"File glob pattern, e.g. '*.ts'"}
                }}).into(),
                prompt_snippet: Presence::Missing,
                prompt_guidelines: Presence::Missing,
                render_shell: Presence::Missing,
                execution_mode: Presence::Missing,
            },
            prepare_arguments: None,
            execute: Rc::new(move |_, params, _, _, ctx| {
                let api = executor.clone();
                Box::pin(async move {
                    let params = serde_json::from_value(params).map_err(|error| error.to_string())?;
                    execute(api, params, ctx.cwd()?).await
                })
            }),
            render_call: Some(Rc::new(|args, theme, _, _| render_call(args, &theme))),
            render_result: Some(Rc::new(|result, options, theme, _, _| render_result(&result, options, &theme))),
        })
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
