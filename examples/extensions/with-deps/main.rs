//! A duration tool registered by an extension that declares its own parsing dependency.
// maestro-component: MAESTRO_WITH_DEPS_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionFuture, ExtensionResult, Presence, TextContent,
    ToolDefinition, ToolMetadata, UserBlock,
};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;

/// Accepted duration text: a signed decimal, ASCII spaces, then an optional unit.
const GRAMMAR: &str = r"(?i-u)^(-?(?:[0-9]+)?\.?[0-9]+) *(milliseconds?|msecs?|ms|seconds?|secs?|s|minutes?|mins?|m|hours?|hrs?|h|days?|d|weeks?|w|years?|yrs?|y)?$";

/// The dependency's own error for the empty text, which never reaches its grammar.
const EMPTY: &str = "val is not a non-empty string or a valid number. val=\"\"";

/// The duration text supplied to the tool.
#[derive(Deserialize)]
struct DurationParams {
    /// Duration text in the accepted grammar.
    duration: String,
}

/// Milliseconds in one of `unit`, or the millisecond factor of a bare number.
fn factor(unit: &str) -> f64 {
    match unit.to_ascii_lowercase().as_str() {
        "years" | "year" | "yrs" | "yr" | "y" => 31_557_600_000.0,
        "weeks" | "week" | "w" => 604_800_000.0,
        "days" | "day" | "d" => 86_400_000.0,
        "hours" | "hour" | "hrs" | "hr" | "h" => 3_600_000.0,
        "minutes" | "minute" | "mins" | "min" | "m" => 60_000.0,
        "seconds" | "second" | "secs" | "sec" | "s" => 1_000.0,
        _ => 1.0,
    }
}

/// The milliseconds `duration` names, or the dependency's invalid-text error.
fn parse_duration(duration: &str, pattern: &Regex) -> ExtensionResult<f64> {
    if duration.is_empty() {
        return Err(EMPTY.into());
    }
    let invalid = || format!("Invalid duration: \"{duration}\"");
    if duration.len() > 100 {
        return Err(invalid());
    }
    let captures = pattern.captures(duration).ok_or_else(invalid)?;
    let number = captures[1]
        .parse::<f64>()
        .map_err(|error| error.to_string())?;
    Ok(number * factor(captures.get(2).map_or("ms", |unit| unit.as_str())))
}

/// A tool that renders the supplied duration in milliseconds.
fn duration_tool() -> ExtensionResult<ToolDefinition> {
    let pattern = Regex::new(GRAMMAR).map_err(|error| error.to_string())?;
    Ok(ToolDefinition {
        render_call: None,
        render_result: None,
        metadata: ToolMetadata {
            name: "parse_duration".into(),
            label: "Parse Duration".into(),
            description: concat!(
                "Parse a human-readable duration string (e.g., '2 days', '1h', '5m') to ",
                "milliseconds"
            )
            .into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            parameters: json!({"type":"object","required":["duration"],"properties":{
                "duration":{"type":"string","description":"Duration string like '2 days', '1h', '5m'"}
            }})
            .into(),
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        execute: Rc::new(move |_, params, _, _, _| {
            let pattern = pattern.clone();
            Box::pin(async move {
                let params: DurationParams =
                    serde_json::from_value(params).map_err(|error| error.to_string())?;
                let milliseconds = parse_duration(&params.duration, &pattern)?;
                let rendered = maestro_json::compact_json(&json!(milliseconds))
                    .map_err(|error| error.to_string())?;
                Ok(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(format!(
                        "{} = {rendered} milliseconds",
                        params.duration
                    )))],
                    details: Presence::Present(json!({}).to_string().into()),
                    terminate: Presence::Missing,
                    usage: None,
                })
            })
        }),
    })
}

/// Registers the duration tool through the supplied extension host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.register_tool(duration_tool()?) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
