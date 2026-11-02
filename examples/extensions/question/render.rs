//! Compact call and result rows using the host's unpadded text widget.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AgentToolResult, Component, ExtensionResult, Text, Theme, UserBlock,
};
use serde_json::Value;

/// Paints the question title and the numbered choices, including custom input.
pub(super) fn call(args: Option<&Value>, theme: &Theme) -> ExtensionResult<Component> {
    let question = args
        .and_then(|a| a["question"].as_str())
        .unwrap_or_default();
    let mut text =
        theme.fg("toolTitle", &theme.bold("question ")?)? + &theme.fg("muted", question)?;
    if let Some(options) = args
        .and_then(|a| a["options"].as_array())
        .filter(|options| !options.is_empty())
    {
        let mut labels: Vec<_> = options
            .iter()
            .map(|o| o["label"].as_str().unwrap_or_default())
            .collect();
        labels.push("Type something.");
        let numbered = labels
            .iter()
            .enumerate()
            .map(|(i, label)| format!("{}. {label}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        text.push('\n');
        text.push_str(&theme.fg("dim", &format!("  Options: {numbered}"))?);
    }
    Ok((&Text::new(&text, 0, 0, None)?).into())
}
/// Reads only the first content block when no structured details were supplied.
fn first_text(result: &AgentToolResult) -> &str {
    match result.content.first() {
        Some(UserBlock::Text(text)) => &text.text,
        _ => "",
    }
}
/// Paints cancellation, custom input or the first matching supplied option number.
pub(super) fn result(result: &AgentToolResult, theme: &Theme) -> ExtensionResult<Component> {
    let details = result
        .details
        .present()
        .map(|details| serde_json::from_str::<Value>(details))
        .transpose()
        .map_err(|e| e.to_string())?
        .filter(maestro_json::is_truthy);
    let Some(details) = details else {
        return Ok((&Text::new(first_text(result), 0, 0, None)?).into());
    };
    let text = if details["answer"].is_null() {
        theme.fg("warning", "Cancelled")?
    } else {
        let answer = details["answer"].as_str().unwrap_or_default();
        let mut text = theme.fg("success", "✓ ")?;
        if details["wasCustom"].as_bool() == Some(true) {
            text.push_str(&theme.fg("muted", "(wrote) ")?);
            text.push_str(&theme.fg("accent", answer)?);
        } else {
            let index = details["options"].as_array().and_then(|options| {
                options
                    .iter()
                    .position(|option| option.as_str() == Some(answer))
            });
            let display = index.map_or_else(|| answer.into(), |i| format!("{}. {answer}", i + 1));
            text.push_str(&theme.fg("accent", &display)?);
        }
        text
    };
    Ok((&Text::new(&text, 0, 0, None)?).into())
}
