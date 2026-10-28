//! A companion extension exercising the terminal client's RPC UI methods.
// maestro-component: MAESTRO_RPC_DEMO_COMPONENT
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionCommandContext, ExtensionContext, ExtensionEvent,
    ExtensionEventResult, ExtensionFuture, ExtensionHandler, ExtensionResult, NotificationType,
    Presence, SessionBeforeSwitchReason, SessionBeforeSwitchResult, SessionEvent,
    SessionStartReason, ToolCallEventResult,
};
use regex::RegexSet;
use serde::Deserialize;
use std::{cell::Cell, rc::Rc};

/// ASCII word boundaries with the authored whitespace set, independent of Unicode regex defaults.
const DANGEROUS: [&str; 2] = [
    r"(?i-u:\brm)[\t-\r \u{00a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]+(?i-u:-rf?|--recursive)",
    r"(?i-u:\bsudo\b)",
];

/// The bash operand inspected without interpreting the shell program.
#[derive(Deserialize)]
struct BashInput {
    /// The command as supplied by the tool caller.
    command: String,
}

/// Publishes startup metadata and a turn counter retained by this loaded extension.
fn lifecycle(
    event: &ExtensionEvent,
    ctx: &ExtensionContext,
    count: &Cell<u64>,
) -> ExtensionResult<()> {
    let ui = ctx.ui()?;
    match event {
        ExtensionEvent::Session(SessionEvent::Start(event)) => {
            ui.set_title(if event.reason == SessionStartReason::New {
                "Maestro RPC Demo (new session)"
            } else {
                "Maestro RPC Demo"
            })?;
            ui.set_widget(
                "rpc-demo",
                Some(&[
                    "--- RPC Extension UI Demo ---".into(),
                    "Loaded and ready.".into(),
                ]),
                None,
            )?;
            ui.set_status("rpc-demo", Some(&format!("Turns: {}", count.get())))?;
        }
        ExtensionEvent::TurnStart(_) => {
            count.set(count.get() + 1);
            ui.set_status(
                "rpc-demo",
                Some(&format!("Turn {} running...", count.get())),
            )?;
        }
        ExtensionEvent::TurnEnd(_) => {
            ui.set_status("rpc-demo", Some(&format!("Turn {} done", count.get())))?;
        }
        _ => {}
    }
    Ok(())
}

/// Requires explicit permission for the two illustrative dangerous-command patterns.
fn tool_call(
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
    patterns: Rc<RegexSet>,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let ExtensionEvent::ToolCall(event) = event else {
            return Ok(None);
        };
        if event.tool_name != "bash" {
            return Ok(None);
        }
        let BashInput { command } = event
            .input
            .decode::<BashInput>()
            .map_err(|error| error.to_string())?;
        if !patterns.is_match(&command) {
            return Ok(None);
        }
        let reason = if ctx.has_ui()? {
            let ui = ctx.ui()?;
            let choice = ui
                .select(
                    format!("Dangerous command: {command}"),
                    vec!["Allow".into(), "Block".into()],
                    None,
                )
                .await?;
            if choice.as_deref() == Some("Allow") {
                ui.notify("Command allowed", Some(NotificationType::Info))?;
                return Ok(None);
            }
            ui.notify("Command blocked by user", Some(NotificationType::Warning))?;
            "Blocked by user"
        } else {
            "Dangerous command blocked (no UI)"
        };
        Ok(Some(ExtensionEventResult::ToolCall(ToolCallEventResult {
            block: Presence::Present(true),
            reason: Presence::Present(reason.into()),
        })))
    })
}

/// Confirms only new-session replacement, retaining a declined session unchanged.
fn before_switch(
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let ExtensionEvent::Session(SessionEvent::BeforeSwitch(event)) = event else {
            return Ok(None);
        };
        if event.reason != SessionBeforeSwitchReason::New || !ctx.has_ui()? {
            return Ok(None);
        }
        let ui = ctx.ui()?;
        if ui
            .confirm(
                "Clear session?".into(),
                "All messages will be lost.".into(),
                None,
            )
            .await?
        {
            return Ok(None);
        }
        ui.notify("Clear cancelled", Some(NotificationType::Info))?;
        Ok(Some(ExtensionEventResult::SessionBeforeSwitch(
            SessionBeforeSwitchResult {
                cancel: Presence::Present(true),
            },
        )))
    })
}

/// Demonstrates input, multiline editing and editor prefill without invoking a model.
async fn command(name: &str, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let ui = ctx.ui()?;
    let message = match name {
        "rpc-input" => match ui
            .input(
                "Enter a value".into(),
                Some("type something...".into()),
                None,
            )
            .await?
            .filter(|text| !text.is_empty())
        {
            Some(value) => format!("You entered: {value}"),
            None => "Input cancelled".into(),
        },
        "rpc-editor" => match ui
            .editor(
                "Edit some text".into(),
                Some("Line 1\nLine 2\nLine 3".into()),
            )
            .await?
            .filter(|text| !text.is_empty())
        {
            Some(text) => format!("Editor submitted ({} lines)", text.split('\n').count()),
            None => "Editor cancelled".into(),
        },
        _ => {
            ui.set_editor_text("This text was set by the rpc-demo extension.")?;
            "Editor prefilled".into()
        }
    };
    ui.notify(&message, Some(NotificationType::Info))
}

/// Registers the companion's lifecycle hooks and interactive commands.
///
/// # Errors
/// Returns a registration failure or a regular-expression construction error.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let count = Rc::new(Cell::new(0));
        let lifecycle_handler: ExtensionHandler = Rc::new(move |event, ctx| {
            let count = count.clone();
            Box::pin(async move {
                lifecycle(event, &ctx, &count)?;
                Ok(None)
            })
        });
        for name in ["session_start", "turn_start", "turn_end"] {
            api.on(name, lifecycle_handler.clone())?;
        }
        let patterns = Rc::new(RegexSet::new(DANGEROUS).map_err(|error| error.to_string())?);
        api.on(
            "tool_call",
            Rc::new(move |event, ctx| tool_call(event, ctx, patterns.clone())),
        )?;
        api.on("session_before_switch", Rc::new(before_switch))?;
        for (name, description) in [
            (
                "rpc-input",
                "Prompt for text input (demonstrates ctx.ui.input in RPC)",
            ),
            (
                "rpc-editor",
                "Open multi-line editor (demonstrates ctx.ui.editor in RPC)",
            ),
            (
                "rpc-prefill",
                "Prefill the input editor (demonstrates ctx.ui.set_editor_text in RPC)",
            ),
        ] {
            api.register_command(
                name,
                CommandOptions {
                    description: Some(description.into()),
                    get_argument_completions: None,
                    handler: Rc::new(move |_, ctx| Box::pin(command(name, ctx))),
                },
            )?;
        }
        Ok(())
    })
}

maestro_extensions_wasm::export_extension!(factory);
