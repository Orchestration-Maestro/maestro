//! Collapsed text and expanded Markdown views over retained child observations.
use super::{tool_call, usage};
use crate::{
    extension::preview,
    runs::{SingleResult, SubagentDetails},
};
use maestro_extensions_wasm::{
    Component, Container, ExtensionAPI, ExtensionResult, Markdown, Text, Theme,
    ToolRenderResultLike, ToolRenderResultOptions, UserBlock, get_markdown_theme,
    tools::path_utils::expand_path,
};
use serde_json::Value;

/// Wraps one line collection in the host's retained text component.
fn text(value: &str) -> ExtensionResult<Component> {
    Ok((&Text::new(value, 0, 0, None)?).into())
}
/// Formats the incomplete arguments of each invocation mode.
pub(crate) fn call(args: Option<&Value>, theme: &Theme) -> ExtensionResult<Component> {
    let empty = Value::Null;
    let args = args.unwrap_or(&empty);
    let scope = args["agentScope"].as_str().unwrap_or("user");
    let title = theme.fg("toolTitle", &theme.bold("subagent ")?)?;
    for (key, name, unit) in [("chain", "chain", "steps"), ("tasks", "parallel", "tasks")] {
        if let Some(tasks) = args[key].as_array().filter(|tasks| !tasks.is_empty()) {
            let mut lines = vec![
                title
                    + &theme.fg("accent", &format!("{name} ({} {unit})", tasks.len()))?
                    + &theme.fg("muted", &format!(" [{scope}]"))?,
            ];
            for (index, task) in tasks.iter().take(3).enumerate() {
                lines.push(call_item(task, index, key, theme)?);
            }
            if tasks.len() > 3 {
                lines.push(theme.fg("muted", &format!("  ... +{} more", tasks.len() - 3))?);
            }
            return text(&lines.join("\n"));
        }
    }
    text(
        &(title
            + &theme.fg("accent", args["agent"].as_str().unwrap_or("..."))?
            + &theme.fg("muted", &format!(" [{scope}]"))?
            + "\n  "
            + &theme.fg("dim", &preview(args["task"].as_str().unwrap_or("..."), 60))?),
    )
}

/// Formats one of the three task previews shown beneath a multi-agent call.
fn call_item(task: &Value, index: usize, key: &str, theme: &Theme) -> ExtensionResult<String> {
    let task_text = task["task"].as_str().unwrap_or("");
    let task_text = if key == "chain" {
        task_text.replace("{previous}", "").trim().to_owned()
    } else {
        task_text.into()
    };
    let number = if key == "chain" {
        theme.fg("muted", &format!("{}. ", index + 1))?
    } else {
        String::new()
    };
    Ok(format!(
        "  {number}{}{}",
        theme.fg("accent", task["agent"].as_str().unwrap_or("..."))?,
        theme.fg("dim", &format!(" {}", preview(&task_text, 40)))?
    ))
}

/// Selects a full retained view only when the requested results are no longer parallel-pending.
pub(crate) fn result(
    result: &ToolRenderResultLike,
    options: ToolRenderResultOptions,
    theme: &Theme,
    api: &ExtensionAPI,
) -> ExtensionResult<Component> {
    let details = result
        .result
        .details
        .present()
        .and_then(|details| serde_json::from_str::<SubagentDetails>(details).ok());
    let Some(details) = details.filter(|details| !details.results.is_empty()) else {
        let first = result.result.content.first().and_then(|part| match part {
            UserBlock::Text(text) => Some(text.text.as_str()),
            UserBlock::Image(_) => None,
        });
        return text(first.unwrap_or("(no output)"));
    };
    let home = expand_path(api, "~")?;
    let running =
        details.mode == "parallel" && details.results.iter().any(|result| result.exit_code == -1);
    if options.expanded && !running {
        expanded(&details, theme, &home)
    } else {
        collapsed(&details, options, theme, &home)
    }
}

/// Chooses a shape and semantic role without relying on color alone.
fn icon(result: &SingleResult, theme: &Theme) -> ExtensionResult<String> {
    let (role, icon) = if result.exit_code == -1 {
        ("warning", "⏳")
    } else if result.failed() {
        ("error", "✗")
    } else {
        ("success", "✓")
    };
    theme.fg(role, icon)
}
/// Builds the single-agent identity and provider failure marker.
fn agent_header(result: &SingleResult, theme: &Theme) -> ExtensionResult<String> {
    let mut header = format!(
        "{} {}{}",
        icon(result, theme)?,
        theme.fg("toolTitle", &theme.bold(&result.agent)?)?,
        theme.fg("muted", &format!(" ({})", result.agent_source))?
    );
    if result.failed()
        && let Some(reason) = &result.stop_reason
    {
        header += &theme.fg("error", &format!(" [{reason}]"))?;
    }
    Ok(header)
}
/// Reports chain counts or the current parallel completion state.
fn group_header(details: &SubagentDetails, theme: &Theme) -> ExtensionResult<String> {
    if details.mode == "single" {
        return agent_header(&details.results[0], theme);
    }
    let running = details
        .results
        .iter()
        .filter(|result| result.exit_code == -1)
        .count();
    let success = details
        .results
        .iter()
        .filter(|result| result.exit_code == 0)
        .count();
    let failed = details
        .results
        .iter()
        .filter(|result| result.exit_code > 0)
        .count();
    let (role, mark) = if details.mode == "parallel" && running > 0 {
        ("warning", "⏳")
    } else if success == details.results.len() {
        ("success", "✓")
    } else if details.mode == "parallel" {
        ("warning", "◐")
    } else {
        ("error", "✗")
    };
    let status = if details.mode == "parallel" && running > 0 {
        format!(
            "{}/{} done, {running} running",
            success + failed,
            details.results.len()
        )
    } else {
        format!(
            "{success}/{} {}",
            details.results.len(),
            if details.mode == "chain" {
                "steps"
            } else {
                "tasks"
            }
        )
    };
    Ok(format!(
        "{} {}{}",
        theme.fg(role, mark)?,
        theme.fg("toolTitle", &theme.bold(&format!("{} ", details.mode))?)?,
        theme.fg("accent", &status)?
    ))
}
/// Selects assistant text and tool calls in their original order.
fn items(result: &SingleResult) -> Vec<&Value> {
    result
        .messages
        .iter()
        .filter(|message| message["role"] == "assistant")
        .filter_map(|message| message["content"].as_array())
        .flatten()
        .filter(|part| matches!(part["type"].as_str(), Some("text" | "toolCall")))
        .collect()
}
/// Formats one tool call without consuming its original arguments.
fn tool(part: &Value, theme: &Theme, home: &str) -> ExtensionResult<String> {
    Ok(theme.fg("muted", "→ ")?
        + &tool_call::format(
            part["name"].as_str().unwrap_or(""),
            &part["arguments"],
            theme,
            home,
        )?)
}
/// Produces bounded collapsed items while retaining full text during an expanded live view.
fn display(
    result: &SingleResult,
    limit: usize,
    expanded: bool,
    theme: &Theme,
    home: &str,
) -> ExtensionResult<String> {
    let items = items(result);
    let skipped = items.len().saturating_sub(limit);
    let mut lines = Vec::new();
    if skipped > 0 {
        lines.push(theme.fg("muted", &format!("... {skipped} earlier items"))?);
    }
    for part in &items[skipped..] {
        if part["type"] == "toolCall" {
            lines.push(tool(part, theme, home)?);
        } else {
            let value = part["text"].as_str().unwrap_or("");
            let value = if expanded {
                value.into()
            } else {
                value.split('\n').take(3).collect::<Vec<_>>().join("\n")
            };
            lines.push(theme.fg("toolOutput", &value)?);
        }
    }
    Ok(lines.join("\n").trim_end().into())
}
/// Builds a compact status, recent activity and aggregate or per-agent usage.
fn collapsed(
    details: &SubagentDetails,
    options: ToolRenderResultOptions,
    theme: &Theme,
    home: &str,
) -> ExtensionResult<Component> {
    let mut lines = vec![group_header(details, theme)?];
    for result in &details.results {
        let single = details.mode == "single";
        if !single {
            lines.extend([String::new(), section(result, &details.mode, theme)?]);
        }
        if single && let Some(error) = result.error_message.as_ref().filter(|_| result.failed()) {
            lines.push(theme.fg("error", &format!("Error: {error}"))?);
        } else if items(result).is_empty() {
            lines.push(theme.fg(
                "muted",
                if result.exit_code == -1 {
                    "(running...)"
                } else {
                    "(no output)"
                },
            )?);
        } else {
            lines.push(display(
                result,
                if single { 10 } else { 5 },
                options.expanded,
                theme,
                home,
            )?);
        }
        if single {
            append_usage(&mut lines, &result.usage, result.model.as_deref(), theme)?;
            if items(result).len() > 10 {
                lines.push(theme.fg("muted", "(Ctrl+O to expand)")?);
            }
        }
    }
    if details.mode != "single" {
        if !details.results.iter().any(|result| result.exit_code == -1) {
            let total = usage::label(&usage::total(&details.results), None);
            if !total.is_empty() {
                lines.push(theme.fg("dim", &format!("\nTotal: {total}"))?);
            }
        }
        if !options.expanded {
            lines.push(theme.fg("muted", "(Ctrl+O to expand)")?);
        }
    }
    text(&lines.join("\n"))
}
/// Labels a multi-agent result, including its chain position where applicable.
fn section(result: &SingleResult, mode: &str, theme: &Theme) -> ExtensionResult<String> {
    let prefix = if mode == "chain" {
        format!("─── Step {}: ", result.step.unwrap_or(0))
    } else {
        "─── ".into()
    };
    Ok(format!(
        "{}{} {}",
        theme.fg("muted", &prefix)?,
        theme.fg("accent", &result.agent)?,
        icon(result, theme)?
    ))
}
/// Appends a nonempty usage label to the caller's line collection.
fn append_usage(
    lines: &mut Vec<String>,
    value: &crate::runs::UsageStats,
    model: Option<&str>,
    theme: &Theme,
) -> ExtensionResult<()> {
    let label = usage::label(value, model);
    if !label.is_empty() {
        lines.push(theme.fg("dim", &label)?);
    }
    Ok(())
}
/// Adds one retained text row to the caller's container.
fn line(container: &Container, value: &str) -> ExtensionResult<()> {
    container.add_child(&text(value)?)
}
/// Retains the toolkit's Markdown owner for the final complete output.
fn expanded(details: &SubagentDetails, theme: &Theme, home: &str) -> ExtensionResult<Component> {
    let container = Container::new()?;
    line(&container, &group_header(details, theme)?)?;
    for result in &details.results {
        let single = details.mode == "single";
        if !single {
            line(&container, "")?;
            line(&container, &section(result, &details.mode, theme)?)?;
        }
        if let Some(error) = result.error_message.as_ref().filter(|_| result.failed()) {
            line(&container, &theme.fg("error", &format!("Error: {error}"))?)?;
        }
        if single {
            line(&container, "")?;
            line(&container, &theme.fg("muted", "─── Task ───")?)?;
            line(&container, &theme.fg("dim", &result.task)?)?;
            line(&container, "")?;
            line(&container, &theme.fg("muted", "─── Output ───")?)?;
        } else {
            line(
                &container,
                &(theme.fg("muted", "Task: ")? + &theme.fg("dim", &result.task)?),
            )?;
        }
        output(&container, result, theme, home)?;
        let label = usage::label(&result.usage, result.model.as_deref());
        if !label.is_empty() {
            if single {
                line(&container, "")?;
            }
            line(&container, &theme.fg("dim", &label)?)?;
        }
    }
    if details.mode != "single" {
        let total = usage::label(&usage::total(&details.results), None);
        if !total.is_empty() {
            line(&container, "")?;
            line(&container, &theme.fg("dim", &format!("Total: {total}"))?)?;
        }
    }
    Ok((&container).into())
}
/// Adds complete tool calls and the final Markdown without duplicating intermediate text.
fn output(
    container: &Container,
    result: &SingleResult,
    theme: &Theme,
    home: &str,
) -> ExtensionResult<()> {
    for part in items(result)
        .into_iter()
        .filter(|part| part["type"] == "toolCall")
    {
        line(container, &tool(part, theme, home)?)?;
    }
    if !result.output().is_empty() {
        line(container, "")?;
        container.add_child(&Component::from(&Markdown::new(
            result.output().trim(),
            0,
            0,
            &get_markdown_theme()?,
            None,
        )?))?;
    } else if items(result).is_empty() {
        line(container, &theme.fg("muted", "(no output)")?)?;
    }
    Ok(())
}
