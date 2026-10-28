//! Compact displays for the original read, bash, edit and write tools.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    Component, ExtensionAPI, ExtensionFuture, ExtensionResult, Text, Theme, ToolRenderResultLike,
    ToolRenderResultOptions, ToolsOptions, UserBlock, utils,
};
use serde_json::Value;
use std::rc::Rc;

/// Builds the same unpadded text widget for each tool display.
fn view(text: &str) -> ExtensionResult<Component> {
    Ok((&Text::new(text, 0, 0, None)?).into())
}

/// Counts nonempty shell output lines with the host's whitespace definition.
fn output_lines(api: &ExtensionAPI, output: &str) -> ExtensionResult<usize> {
    let mut count = 0;
    for line in output.split('\n') {
        for scalar in line.chars() {
            if !utils::is_whitespace_char(api, &scalar.to_string())? {
                count += 1;
                break;
            }
        }
    }
    Ok(count)
}

/// Adds at most the supplied number of dimmed content lines.
fn preview(text: &mut String, output: &str, limit: usize, theme: &Theme) -> ExtensionResult<()> {
    for line in output.split('\n').take(limit) {
        text.push('\n');
        text.push_str(&theme.fg("dim", line)?);
    }
    Ok(())
}

/// Shows image status or the read line count, with an optional fifteen-line preview.
fn read_result(
    content: Option<&UserBlock>,
    details: &Value,
    expanded: bool,
    theme: &Theme,
) -> ExtensionResult<String> {
    let output = match content {
        Some(UserBlock::Image(_)) => return theme.fg("success", "Image loaded"),
        Some(UserBlock::Text(content)) => &content.text,
        _ => return theme.fg("error", "No content"),
    };
    let count = output.split('\n').count();
    let mut text = theme.fg("success", &format!("{count} lines"))?;
    if details["truncation"]["truncated"] == true {
        text.push_str(&theme.fg(
            "warning",
            &format!(" (truncated from {})", details["truncation"]["totalLines"]),
        )?);
    }
    if expanded {
        preview(&mut text, output, 15, theme)?;
        if count > 15 {
            text.push('\n');
            text.push_str(&theme.fg("muted", &format!("... {} more lines", count - 15))?);
        }
    }
    Ok(text)
}

/// Reads the first content block only when it holds text.
fn first_text(result: &ToolRenderResultLike) -> &str {
    match result.result.content.first() {
        Some(UserBlock::Text(content)) => &content.text,
        _ => "",
    }
}

/// Shows the shell's reported exit code and at most twenty expanded output lines.
fn bash_result(
    api: &ExtensionAPI,
    result: &ToolRenderResultLike,
    details: &Value,
    options: ToolRenderResultOptions,
    theme: &Theme,
) -> ExtensionResult<String> {
    let output = first_text(result);
    let pattern = regex::Regex::new(r"(?:exit code: |Command exited with code )([0-9]+)\z")
        .map_err(|error| error.to_string())?;
    let code = pattern
        .captures(output)
        .filter(|_| result.is_error.present() != Some(&false))
        .and_then(|capture| capture[1].parse::<u64>().ok());
    let mut text = match code {
        Some(code) if code != 0 => theme.fg("error", &format!("exit {code}"))?,
        _ if result.is_error.present() == Some(&true) => {
            theme.fg("error", output.split('\n').next().unwrap_or(""))?
        }
        _ => theme.fg("success", "done")?,
    };
    text.push_str(&theme.fg("dim", &format!(" ({} lines)", output_lines(api, output)?))?);
    if details["truncation"]["truncated"] == true {
        text.push_str(&theme.fg("warning", " [truncated]")?);
    }
    if options.expanded {
        preview(&mut text, output, 20, theme)?;
        if output.split('\n').count() > 20 {
            text.push('\n');
            text.push_str(&theme.fg("muted", "... more output")?);
        }
    }
    Ok(text)
}

/// Distinguishes changed lines from diff headers and unchanged context.
fn diff_role(line: &str) -> &'static str {
    if line.starts_with('+') && !line.starts_with("+++") {
        "success"
    } else if line.starts_with('-') && !line.starts_with("---") {
        "error"
    } else {
        "dim"
    }
}

/// Shows edit statistics and at most thirty semantically coloured diff lines.
fn edit_result(details: &Value, expanded: bool, theme: &Theme) -> ExtensionResult<String> {
    let Some(diff) = details["diff"].as_str().filter(|diff| !diff.is_empty()) else {
        return theme.fg("success", "Applied");
    };
    let lines: Vec<_> = diff.split('\n').collect();
    let additions = lines
        .iter()
        .filter(|line| diff_role(line) == "success")
        .count();
    let removals = lines
        .iter()
        .filter(|line| diff_role(line) == "error")
        .count();
    let mut text = theme.fg("success", &format!("+{additions}"))?;
    text.push_str(&theme.fg("dim", " / ")?);
    text.push_str(&theme.fg("error", &format!("-{removals}"))?);
    if expanded {
        for line in lines.iter().take(30) {
            text.push('\n');
            text.push_str(&theme.fg(diff_role(line), line)?);
        }
        if lines.len() > 30 {
            text.push('\n');
            text.push_str(&theme.fg(
                "muted",
                &format!("... {} more diff lines", lines.len() - 30),
            )?);
        }
    }
    Ok(text)
}

/// Summarizes the first content block without changing the original tool result.
fn render_result(
    api: &ExtensionAPI,
    name: &str,
    result: &ToolRenderResultLike,
    options: ToolRenderResultOptions,
    theme: &Theme,
) -> ExtensionResult<Component> {
    if options.is_partial {
        let progress = match name {
            "read" => "Reading...",
            "bash" => "Running...",
            "edit" => "Editing...",
            _ => "Writing...",
        };
        return view(&theme.fg("warning", progress)?);
    }
    let details = result
        .result
        .details
        .present()
        .map(|details| serde_json::from_str::<Value>(details))
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or(Value::Null);
    let content = result.result.content.first();
    let output = first_text(result);
    if name != "bash" && result.is_error.present() == Some(&true) {
        return view(&theme.fg("error", output.split('\n').next().unwrap_or(""))?);
    }
    let text = match name {
        "read" => read_result(content, &details, options.expanded, theme)?,
        "bash" => bash_result(api, result, &details, options, theme)?,
        _ if output.starts_with("Error") => {
            theme.fg("error", output.split('\n').next().unwrap_or(""))?
        }
        "edit" => edit_result(&details, options.expanded, theme)?,
        _ => theme.fg("success", "Written")?,
    };
    view(&text)
}

/// Clips long command strings at a whole scalar within the 77 UTF-16-unit prefix.
fn command_preview(command: &str) -> String {
    if command.encode_utf16().count() <= 80 {
        return command.into();
    }
    let mut units = 0;
    let prefix: String = command
        .chars()
        .take_while(|scalar| {
            units += scalar.len_utf16();
            units <= 77
        })
        .collect();
    format!("{prefix}...")
}

/// Shows the original tool name and only its compact call operands.
fn render_call(name: &str, args: Option<&Value>, theme: &Theme) -> ExtensionResult<Component> {
    let args = args.unwrap_or(&Value::Null);
    let title = if name == "bash" {
        "$ ".into()
    } else {
        format!("{name} ")
    };
    let mut text = theme.fg("toolTitle", &theme.bold(&title)?)?;
    let operand = if name == "bash" {
        command_preview(args["command"].as_str().unwrap_or(""))
    } else {
        args["path"].as_str().unwrap_or("undefined").into()
    };
    text.push_str(&theme.fg("accent", &operand)?);
    match name {
        "read" => {
            let parts: Vec<_> = ["offset", "limit"]
                .into_iter()
                .filter_map(|key| {
                    args[key]
                        .as_f64()
                        .filter(|number| *number != 0.0)
                        .map(|number| format!("{key}={number}"))
                })
                .collect();
            if !parts.is_empty() {
                text.push_str(&theme.fg("dim", &format!(" ({})", parts.join(", ")))?);
            }
        }
        "bash" => {
            if let Some(timeout) = args["timeout"].as_f64().filter(|number| *number != 0.0) {
                text.push_str(&theme.fg("dim", &format!(" (timeout: {timeout}s)"))?);
            }
        }
        "write" => {
            let count = args["content"].as_str().unwrap_or("").split('\n').count();
            text.push_str(&theme.fg("dim", &format!(" ({count} lines)"))?);
        }
        _ => {}
    }
    view(&text)
}

/// Replaces call and result displays while retaining the original native executions.
///
/// # Errors
/// The returned future returns the failure to read the working directory, its refusal
/// when the path is not valid UTF-8, the host's failure to create a built-in tool
/// definition, or its rejection of a registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
        let cwd = cwd.to_str().ok_or("working directory is not valid UTF-8")?;
        for name in ["read", "bash", "edit", "write"] {
            let mut tool = api.create_tool_definition(name, cwd.into(), ToolsOptions::default())?;
            let host = api.clone();
            tool.render_call = Some(Rc::new(move |args, theme, _, _| {
                render_call(name, args.as_ref(), &theme)
            }));
            tool.render_result = Some(Rc::new(move |result, options, theme, _, _| {
                render_result(&host, name, &result, options, &theme)
            }));
            api.register_tool(tool)?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
