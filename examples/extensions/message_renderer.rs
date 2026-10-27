//! A `/status` command posts status notes, and the renderer it registers draws each one: the
//! level in its theme color over the custom message background, and the host's local send
//! time once the message is expanded.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use maestro_extensions_wasm::{
    CommandOptions, Component, CustomMessage, CustomMessageInput, ExtensionAPI, ExtensionFuture,
    ExtensionResult, Presence, Style, Text, Theme, UserBlock, UserContent,
    utils::local_time_string,
};
use num_traits::ToPrimitive;
use serde_json::{Value, json};

/// Whitespace as the command trims and splits it: Unicode white space except U+0085, and the
/// byte order mark.
fn is_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

/// Sends the arguments as a status message; a leading `warn` or `error` word sets its level.
fn send(api: &ExtensionAPI, args: &str) -> ExtensionResult<()> {
    let text = args.trim_matches(is_space);
    let mut words = text.split(is_space).filter(|word| !word.is_empty());
    let (level, content) = match words.next() {
        Some(level @ ("warn" | "error")) => {
            let content = words.collect::<Vec<_>>().join(" ");
            let content = if content.is_empty() {
                "Status report".to_owned()
            } else {
                content
            };
            (level, content)
        }
        _ => ("info", text.to_owned()),
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    api.send_message(
        CustomMessageInput {
            custom_type: "status-update".to_owned(),
            content: UserContent::Text(content),
            display: true,
            details: Presence::Present(
                json!({ "level": level, "timestamp": timestamp })
                    .to_string()
                    .into(),
            ),
        },
        None,
    )
}

/// Draws a status message: its level colored by severity, its text and, when expanded and
/// timestamped, the local time it was sent.
fn render(
    api: &ExtensionAPI,
    message: &CustomMessage,
    expanded: bool,
    theme: Theme,
) -> ExtensionResult<Component> {
    let details: Option<Value> = message
        .details
        .present()
        .map(|details| serde_json::from_str(details))
        .transpose()
        .map_err(|error| error.to_string())?;
    let field = |name: &str| details.as_ref().and_then(|details| details.get(name));
    let level = field("level").and_then(Value::as_str).unwrap_or("info");
    let color = match level {
        "error" => "error",
        "warn" => "warning",
        _ => "success",
    };
    let content = match &message.content {
        UserContent::Text(text) => text.clone(),
        UserContent::Blocks(blocks) => UserBlock::text_output(blocks),
    };
    let prefix = theme.fg(color, &format!("[{}]", level.to_uppercase()))?;
    let mut text = format!("{prefix} {content}");
    if expanded
        && let Some(timestamp) = field("timestamp")
            .and_then(Value::as_f64)
            .filter(|timestamp| *timestamp != 0.0)
    {
        // Past the integer range is past the calendar too, which the host reads as invalid.
        let time = local_time_string(api, timestamp.to_i64().unwrap_or(i64::MAX))?;
        text = format!("{text}\n{}", theme.fg("dim", &format!("  sent {time}"))?);
    }
    let background = Style::new(move |text| theme.bg("customMessageBg", text));
    let view = maestro_extensions_wasm::Box::new(1, 1, Some(&background))?;
    view.add_child(&Component::from(&Text::new(&text, 0, 0, None)?))?;
    Ok(Component::from(&view))
}

/// Registers the `status-update` renderer and the `/status` command that sends such messages.
///
/// # Errors
/// Returns the host's failure when it rejects a registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let helpers = api.clone();
        api.register_message_renderer(
            "status-update",
            Rc::new(move |message, options, theme, _| {
                render(&helpers, &message, options.expanded, theme).map(Some)
            }),
        )?;
        let sender = api.clone();
        api.register_command(
            "status",
            CommandOptions {
                description: Some("Post a status note: /status [warn|error] <text>".to_owned()),
                get_argument_completions: None,
                handler: Rc::new(move |args, _| {
                    let sender = sender.clone();
                    Box::pin(async move { send(&sender, &args) })
                }),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
