//! Follows the macOS system appearance through the host's theme interface.
// maestro-component: MAESTRO_MAC_SYSTEM_THEME_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEventResult, ExtensionFuture, ExtensionResult,
    ThemeSelection,
};
use std::{rc::Rc, time::Duration};

#[path = "support/tail.rs"]
mod tail;
use tail::Tail;

/// Queries appearance with literal arguments; command failures fall back to light.
async fn appearance(api: &ExtensionAPI) -> &'static str {
    let args = [
        "-e".into(),
        "tell application \"System Events\" to tell appearance preferences to return dark mode"
            .into(),
    ];
    match api.exec("osascript", &args, None).await {
        Ok(result) if result.code == 0 && !result.killed && result.stdout.trim() == "true" => {
            "dark"
        }
        _ => "light",
    }
}

/// The channel handing periodic work to the host's independent tail executor.
const CHANNEL: &str = "maestro.example.mac-system-theme";

/// Waits through the native timer owner and applies only changed appearances.
async fn synchronize(
    api: ExtensionAPI,
    ctx: ExtensionContext,
    mut theme: &'static str,
) -> ExtensionResult<()> {
    let io = api.native_io();
    loop {
        io.sleep(Duration::from_millis(2000), None).await?;
        let next = appearance(&api).await;
        if next != theme {
            theme = next;
            ctx.ui()?.set_theme(ThemeSelection::Name(theme.into()))?;
        }
    }
}

/// Cancels previous polling before initial detection, then announces one new host tail.
async fn start(
    api: ExtensionAPI,
    tail: Tail,
    ctx: ExtensionContext,
) -> ExtensionResult<Option<ExtensionEventResult>> {
    tail.stop();
    let theme = appearance(&api).await;
    ctx.ui()?.set_theme(ThemeSelection::Name(theme.into()))?;
    tail.start(Box::pin(synchronize(api.clone(), ctx, theme)));
    if let Err(error) = api.events().emit(CHANNEL, serde_json::Value::Null).await {
        tail.stop();
        return Err(error);
    }
    Ok(None)
}

/// Registers initial selection, two-second change checks and shutdown cleanup.
///
/// # Errors
/// Returns registration and host interface failures unchanged.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let tail = Tail::default();
        let subscription = api.events().on(CHANNEL, tail.listener())?;
        let (owner, current) = (api.clone(), tail.clone());
        api.on(
            "session_start",
            Rc::new(move |_, ctx| Box::pin(start(owner.clone(), current.clone(), ctx))),
        )?;
        api.on(
            "session_shutdown",
            Rc::new(move |_, _| {
                tail.stop();
                let result = subscription.unsubscribe();
                Box::pin(async move { result.map(|()| None) })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
