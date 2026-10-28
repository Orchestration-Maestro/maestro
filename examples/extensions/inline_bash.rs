//! An opt-in input hook that expands `!{command}` with shell output before submission.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    ExecOptions, ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult,
    ExtensionFuture, ExtensionResult, InputEventResult, InputTransform, NotificationType, utils,
};
use std::rc::Rc;

/// Trims edges using the host's whitespace classification.
fn trim<'a>(api: &ExtensionAPI, text: &'a str) -> ExtensionResult<&'a str> {
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        if !utils::is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        start = index + ch.len_utf8();
    }
    let mut end = text.len();
    for ch in text[start..].chars().rev() {
        if !utils::is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        end -= ch.len_utf8();
    }
    Ok(&text[start..end])
}

/// Reads a nonempty expression ending at this original fragment's closing brace.
fn expression(fragment: &str) -> Option<(&str, &str)> {
    let (prefix, tail) = fragment.split_once("!{")?;
    let command = tail.strip_suffix('}')?;
    (!command.is_empty()).then_some((prefix, command))
}

/// Expands original input fragments without rescanning command output.
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
        let args = ["-c".into(), command.into()];
        let output = api
            .exec(
                "bash",
                &args,
                Some(ExecOptions {
                    timeout: Some(30_000.0),
                    ..ExecOptions::default()
                }),
            )
            .await?;
        let output = trim(api, &output.stdout)?;
        result.push_str(prefix);
        result.push_str(output);
        summaries.push(format!("!{{{command}}} -> \"{output}\""));
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
