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

/// Shows the shell's reported exit code and nonempty output line count.
fn bash_result(api: &ExtensionAPI, output: &str, theme: &Theme) -> ExtensionResult<String> {
    let pattern = regex::Regex::new(r"exit code: ([0-9]+)").map_err(|error| error.to_string())?;
    let code = pattern
        .captures(output)
        .and_then(|capture| capture[1].parse::<u64>().ok());
    let mut text = match code {
        Some(code) if code != 0 => theme.fg("error", &format!("exit {code}"))?,
        _ => theme.fg("success", "done")?,
    };
    text.push_str(&theme.fg("dim", &format!(" ({} lines)", output_lines(api, output)?))?);
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
    let content = result.result.content.first();
    let output = match content {
        Some(UserBlock::Text(content)) => content.text.as_str(),
        _ => "",
    };
    let text = match name {
        "read" => match content {
            Some(UserBlock::Image(_)) => theme.fg("success", "Image loaded")?,
            Some(UserBlock::Text(_)) => {
                theme.fg("success", &format!("{} lines", output.split('\n').count()))?
            }
            _ => theme.fg("error", "No content")?,
        },
        "bash" => bash_result(api, output, theme)?,
        _ if output.starts_with("Error") => {
            theme.fg("error", output.split('\n').next().unwrap_or(""))?
        }
        "edit" => theme.fg("success", "Applied")?,
        _ => theme.fg("success", "Written")?,
    };
    view(&text)
}

/// Replaces only result displays, retaining each original definition's execution.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
        let cwd = cwd.to_str().ok_or("working directory is not valid UTF-8")?;
        for name in ["read", "bash", "edit", "write"] {
            let mut tool = api.create_tool_definition(name, cwd.into(), ToolsOptions::default())?;
            let host = api.clone();
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
