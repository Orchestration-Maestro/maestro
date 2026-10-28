//! An opt-in input hook: `?quick <question>` requests a brief answer; `ping` and `time`
//! show immediate feedback without a model call. Extension-injected input passes through.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    ExtensionResult, InputEventResult, InputSource, InputTransform, NotificationType, Presence,
    utils,
};

/// Trims surrounding characters using the host's shared whitespace definition.
fn trim<'a>(api: &ExtensionAPI, mut text: &'a str) -> ExtensionResult<&'a str> {
    while let Some(ch) = text.chars().next() {
        if !utils::is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        text = &text[ch.len_utf8()..];
    }
    while let Some(ch) = text.chars().next_back() {
        if !utils::is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        text = &text[..text.len() - ch.len_utf8()];
    }
    Ok(text)
}

/// Returns a rewrite, local handling decision or explicit continuation for the input.
fn input(
    api: &ExtensionAPI,
    event: &mut ExtensionEvent,
    ctx: &ExtensionContext,
) -> ExtensionResult<Option<ExtensionEventResult>> {
    let ExtensionEvent::Input(event) = event else {
        return Ok(None);
    };
    let result = if event.source == InputSource::Extension {
        InputEventResult::Continue
    } else if let Some(query) = event.text.strip_prefix("?quick ") {
        let query = trim(api, query)?;
        if query.is_empty() {
            ctx.ui()?
                .notify("Usage: ?quick <question>", Some(NotificationType::Warning))?;
            InputEventResult::Handled
        } else {
            InputEventResult::Transform(InputTransform {
                text: format!("Respond briefly in 1-2 sentences: {query}"),
                images: Presence::Missing,
            })
        }
    } else if event.text.eq_ignore_ascii_case("ping") {
        ctx.ui()?.notify("pong", Some(NotificationType::Info))?;
        InputEventResult::Handled
    } else if event.text.eq_ignore_ascii_case("time") {
        ctx.ui()?.notify(
            &utils::local_date_time_string(api)?,
            Some(NotificationType::Info),
        )?;
        InputEventResult::Handled
    } else {
        InputEventResult::Continue
    };
    Ok(Some(ExtensionEventResult::Input(result)))
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
                Box::pin(async move { input(&api, event, &ctx) })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
