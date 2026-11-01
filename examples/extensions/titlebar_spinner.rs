//! Animates the terminal title while the agent works, restoring it when work stops.
// maestro-component: MAESTRO_TITLEBAR_SPINNER_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{rc::Rc, time::Duration};

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEventResult, ExtensionFuture, ExtensionResult,
    Subscription, tools::path_utils,
};

/// Frames in display order, one every 80 milliseconds.
const BRAILLE_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// The example's channel for handing periodic work to the host's event-tail executor.
const CHANNEL: &str = "maestro.example.titlebar-spinner";

#[path = "support/tail.rs"]
mod tail;
use tail::Tail;

/// Reads the current name and directory, without caching either between frames.
fn base_title(api: &ExtensionAPI, context: &ExtensionContext) -> ExtensionResult<String> {
    let basename = path_utils::basename(api, &context.cwd()?)?;
    // Matches maestro-chat/src/interactive_mode/events.rs:489-497 and configuration/config/mod.rs:24.
    Ok(
        match api.get_session_name()?.filter(|name| !name.is_empty()) {
            Some(name) => format!("Maestro - {name} - {basename}"),
            None => format!("Maestro - {basename}"),
        },
    )
}

/// Waits before each frame, leaving scheduling and timer disposal with native I/O.
fn animate(api: ExtensionAPI, context: ExtensionContext) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let io = api.native_io();
        for frame in BRAILLE_FRAMES.iter().cycle() {
            io.sleep(Duration::from_millis(80), None).await?;
            context
                .ui()?
                .set_title(&format!("{frame} {}", base_title(&api, &context)?))?;
        }
        Ok(())
    })
}

/// Applies one lifecycle event to the animation slot shared by all three hooks.
fn handle(
    api: ExtensionAPI,
    current: Tail,
    subscription: Subscription,
    event: &'static str,
    context: ExtensionContext,
) -> ExtensionFuture<'static, Option<ExtensionEventResult>> {
    Box::pin(async move {
        current.stop();
        if event == "session_shutdown" {
            subscription.unsubscribe()?;
        }
        context.ui()?.set_title(&base_title(&api, &context)?)?;
        if event == "agent_start" {
            current.start(animate(api.clone(), context));
            if let Err(error) = api.events().emit(CHANNEL, serde_json::Value::Null).await {
                current.stop();
                return Err(error);
            }
        }
        Ok(None)
    })
}

/// Registers start, end and shutdown hooks using host-owned event tails for periodic work.
///
/// # Errors
/// Returns registration failures unchanged.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let current = Tail::default();
        let subscription = api.events().on(CHANNEL, current.listener())?;
        for event in ["agent_start", "agent_end", "session_shutdown"] {
            let (owner, current, subscription) =
                (api.clone(), current.clone(), subscription.clone());
            api.on(
                event,
                Rc::new(move |_, context| {
                    handle(
                        owner.clone(),
                        current.clone(),
                        subscription.clone(),
                        event,
                        context,
                    )
                }),
            )?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
