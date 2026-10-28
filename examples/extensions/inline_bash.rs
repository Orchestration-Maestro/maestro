//! An opt-in input hook that expands `!{command}` with shell output before submission.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    ExecOptions, ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult,
    ExtensionFuture, ExtensionResult, InputEventResult, InputTransform, NotificationType,
};
use std::rc::Rc;

#[path = "support/trim.rs"]
mod trim;
use trim::trim;

/// Reads a nonempty expression ending at this original fragment's closing brace.
fn expression(fragment: &str) -> Option<(&str, &str)> {
    let (prefix, tail) = fragment.split_once("!{")?;
    let command = tail.strip_suffix('}')?;
    (!command.is_empty()).then_some((prefix, command))
}

/// Executes one command and retains its full replacement beside its notification preview.
///
/// # Errors
/// Returns host whitespace-classification failures when trimming command output.
async fn expand(api: &ExtensionAPI, command: &str) -> ExtensionResult<(String, String)> {
    let args = ["-c".into(), command.into()];
    let executed = api
        .exec(
            "bash",
            &args,
            Some(ExecOptions {
                timeout: Some(30_000.0),
                ..ExecOptions::default()
            }),
        )
        .await;
    let result = match executed {
        Ok(result) => result,
        Err(error) => {
            return Ok((
                format!("[error: {error}]"),
                format!("!{{{command}}} ({error}) -> \"\""),
            ));
        }
    };
    let selected = if result.stdout.is_empty() {
        &result.stderr
    } else {
        &result.stdout
    };
    let output = trim(api, selected)?.to_owned();
    let status = if result.code != 0 && !result.stderr.is_empty() {
        format!(" (exit code {})", result.code)
    } else {
        String::new()
    };
    let preview: String = output.chars().take(50).collect();
    let suffix = if preview.len() < output.len() {
        "..."
    } else {
        ""
    };
    let summary = format!("!{{{command}}}{status} -> \"{preview}{suffix}\"");
    Ok((output, summary))
}

/// Expands original input fragments without rescanning command output.
///
/// # Errors
/// Returns host whitespace-classification, UI availability and notification failures.
async fn input(
    api: &ExtensionAPI,
    event: &mut ExtensionEvent,
    ctx: &ExtensionContext,
) -> ExtensionResult<Option<ExtensionEventResult>> {
    let ExtensionEvent::Input(event) = event else {
        return Ok(None);
    };
    let text = trim(api, &event.text)?;
    if text.starts_with('!') && !text.starts_with("!{") {
        return Ok(Some(ExtensionEventResult::Input(
            InputEventResult::Continue,
        )));
    }
    let mut result = String::new();
    let mut summaries = Vec::new();
    for fragment in event.text.split_inclusive('}') {
        let Some((prefix, command)) = expression(fragment) else {
            result.push_str(fragment);
            continue;
        };
        let (output, summary) = expand(api, command).await?;
        result.push_str(prefix);
        result.push_str(&output);
        summaries.push(summary);
    }
    let decision = if summaries.is_empty() {
        InputEventResult::Continue
    } else {
        if ctx.has_ui()? {
            ctx.ui()?.notify(
                &format!(
                    "Expanded {} inline command(s):\n{}",
                    summaries.len(),
                    summaries.join("\n")
                ),
                Some(NotificationType::Info),
            )?;
        }
        InputEventResult::Transform(InputTransform {
            text: result,
            images: event.images.clone(),
        })
    };
    Ok(Some(ExtensionEventResult::Input(decision)))
}

/// Registers the input hook through the supplied host.
///
/// # Errors
/// The returned future reports input-handler registration failures.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let handler = api.clone();
        api.on(
            "input",
            Rc::new(move |event, ctx| {
                let api = handler.clone();
                Box::pin(async move { input(&api, event, &ctx).await })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
