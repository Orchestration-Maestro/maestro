//! An opt-in read override with access records and illustrative sensitive-path checks.
// maestro-component: MAESTRO_TOOL_OVERRIDE_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{fs::OpenOptions, io::Write, rc::Rc, time::SystemTime};

use chrono::{DateTime, SecondsFormat, Utc};
use maestro_extensions_wasm::{
    AgentToolResult, CommandOptions, ExtensionAPI, ExtensionFuture, ExtensionResult,
    NotificationType, Presence, TextContent, ToolDefinition, ToolMetadata, UserBlock, config,
    tools::path_utils::{guest_path, resolve},
    utils::is_whitespace_char,
};
use regex::RegexSet;
use serde::Deserialize;
use serde_json::{Value, json};

/// Read arguments retain the optional numeric line bounds.
#[derive(Deserialize)]
struct ReadParams {
    /// Relative or absolute authored path.
    path: String,
    /// One-indexed starting line.
    offset: Option<f64>,
    /// Maximum selected lines.
    limit: Option<f64>,
}

/// Formats a standard wall-clock instant with UTC milliseconds.
fn timestamp(instant: SystemTime) -> String {
    DateTime::<Utc>::from(instant).to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Appends one record in the host's existing per-file mutation queue.
async fn log_access(api: &ExtensionAPI, log: &str, path: &str, blocked: bool) {
    let status = if blocked { "BLOCKED" } else { "ALLOWED" };
    let reason = if blocked {
        " (matches blocked pattern)"
    } else {
        ""
    };
    let line = format!(
        "[{}] {status}: {path}{reason}\n",
        timestamp(SystemTime::now())
    );
    let Ok(file) = guest_path(api, log) else {
        return;
    };
    let _ = api
        .with_file_mutation_queue(log.to_owned(), move || {
            Box::pin(async move {
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file)
                    .map_err(|error| error.to_string())?;
                file.write_all(line.as_bytes())
                    .map_err(|error| error.to_string())
            })
        })
        .await;
}

/// Builds a plain text result without replacing the built-in renderer.
fn result(text: String, details: &Value) -> AgentToolResult {
    AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(text))],
        details: Presence::Present(details.to_string().into()),
        terminate: Presence::Missing,
        usage: None,
    }
}

/// Selects numeric line bounds and clips output at a whole UTF-8 scalar.
fn read_content(path: &str, params: &ReadParams) -> std::io::Result<AgentToolResult> {
    let bytes = std::fs::read(path)?;
    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<_> = content.split('\n').collect();
    let count = lines.iter().fold(0.0, |n, _| n + 1.0);
    let start = (params.offset.unwrap_or(0.0) - 1.0).max(0.0);
    let end = params
        .limit
        .filter(|value| *value != 0.0)
        .map_or(count, |limit| {
            let end = (start + limit).trunc();
            if end < 0.0 {
                (count + end).max(0.0)
            } else {
                end
            }
        });
    let mut index = 0.0;
    let mut text = lines
        .iter()
        .filter(|_| {
            let selected = index >= start.trunc() && index < end;
            index += 1.0;
            selected
        })
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    if text.len() > 50 * 1024 {
        let mut boundary = 50 * 1024;
        while !text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        text.truncate(boundary);
        text.push_str("\n\n[Output truncated at 50KB]");
    }
    Ok(result(text, &json!({"lines":lines.len()})))
}

/// Registers the audited tool with the source's path patterns and schema.
fn read_tool(api: ExtensionAPI, log: String) -> ExtensionResult<ToolDefinition> {
    let blocked = RegexSet::new([
        r"\.env$",
        r"\.env\..+$",
        r"(?i)secrets?\.(json|yaml|yml|toml)$",
        r"(?i)credentials?\.(json|yaml|yml|toml)$",
        r"/\.ssh/",
        r"/\.aws/",
        r"/\.gnupg/",
    ])
    .map_err(|error| error.to_string())?;
    Ok(ToolDefinition {
        metadata: ToolMetadata {
            name: "read".into(), label: "read (audited)".into(),
            description: "Read the contents of a file with access logging. Some sensitive paths (.env, secrets, credentials) are blocked.".into(),
            parameters: json!({"type":"object","required":["path"],"properties":{
                "path":{"type":"string","description":"Path to the file to read (relative or absolute)"},
                "offset":{"type":"number","description":"Line number to start reading from (1-indexed)"},
                "limit":{"type":"number","description":"Maximum number of lines to read"}
            }}).into(),
            prompt_snippet: Presence::Missing, prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing, execution_mode: Presence::Missing,
        },
        render_call: None, render_result: None, prepare_arguments: None,
        execute: Rc::new(move |_, params, _, _, ctx| {
            let (api, log, blocked) = (api.clone(), log.clone(), blocked.clone());
            Box::pin(async move {
                let params: ReadParams = serde_json::from_value(params).map_err(|error| error.to_string())?;
                let path = resolve(&api, &[&ctx.cwd()?, &params.path])?;
                // The component spelling opens the file and separates it with `/` on every host.
                let file = guest_path(&api, &path)?;
                let denied = blocked.is_match(&file);
                log_access(&api, &log, &path, denied).await;
                if denied {
                    return Ok(result(format!("Access denied: \"{}\" matches a blocked pattern (sensitive file). This tool blocks access to .env files, secrets, credentials, and SSH/AWS/GPG directories.", params.path), &json!({"blocked":true})));
                }
                Ok(read_content(&file, &params).unwrap_or_else(|error| {
                    result(format!("Error reading file: {error}"), &json!({"error":true}))
                }))
            })
        }),
    })
}

/// Displays the last twenty records, trimming with the host's whitespace rules.
fn recent_log(api: &ExtensionAPI, log: &str) -> ExtensionResult<String> {
    let Ok(bytes) = std::fs::read(guest_path(api, log)?) else {
        return Ok("No access log found".into());
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut trimmed = text.as_ref();
    while let Some(ch) = trimmed.chars().next() {
        if !is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        trimmed = &trimmed[ch.len_utf8()..];
    }
    while let Some(ch) = trimmed.chars().next_back() {
        if !is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        trimmed = &trimmed[..trimmed.len() - ch.len_utf8()];
    }
    let lines: Vec<_> = trimmed.split('\n').collect();
    Ok(format!(
        "Recent file access:\n{}",
        lines[lines.len().saturating_sub(20)..].join("\n")
    ))
}

/// Registers the override and the access-log display command through the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let log = resolve(&api, &[&config::get_agent_dir(&api)?, "read-access.log"])?;
        api.register_tool(read_tool(api.clone(), log.clone())?)?;
        let registered = api.clone();
        api.register_command(
            "read-log",
            CommandOptions {
                description: Some("View the file access log".into()),
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| {
                    let (api, log) = (registered.clone(), log.clone());
                    Box::pin(async move {
                        ctx.ui()?
                            .notify(&recent_log(&api, &log)?, Some(NotificationType::Info))
                    })
                }),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

#[cfg(test)]
mod tests {
    #[test]
    fn audit_timestamp_uses_utc_milliseconds() {
        assert_eq!(
            super::timestamp(
                std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_704_164_645_006)
            ),
            "2024-01-02T03:04:05.006Z"
        );
    }
}
