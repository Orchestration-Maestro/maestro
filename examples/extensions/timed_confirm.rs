//! Timed confirmation, selection and caller-owned cancellation through the host UI.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::future::poll_fn;
use std::rc::Rc;
use std::task::Poll;
use std::time::Duration;

use maestro_extensions_wasm::{
    AbortController, CommandHandler, CommandOptions, ExtensionAPI, ExtensionCommandContext,
    ExtensionFuture, ExtensionResult, ExtensionUIContext, ExtensionUIDialogOptions,
    NotificationType,
};

/// The confirmation shared by the automatic and caller-cancelled dialogs.
const TITLE: &str = "Timed Confirmation";
/// The confirmation's literal question.
const MESSAGE: &str = "This dialog will auto-cancel in 5 seconds. Confirm?";

/// Cancels the pending confirmation with a caller-owned signal after five seconds.
async fn timed_signal(api: &ExtensionAPI, ui: &ExtensionUIContext) -> ExtensionResult<()> {
    let controller = AbortController::new();
    let signal = controller.signal();
    let io = api.native_io();
    let mut delay = io.sleep(Duration::from_secs(5), None);
    ui.notify(
        "Dialog will auto-cancel in 5 seconds...",
        Some(NotificationType::Info),
    )?;
    let mut dialog = ui.confirm(
        TITLE.into(),
        MESSAGE.into(),
        Some(ExtensionUIDialogOptions {
            signal: Some(signal.clone()),
            timeout: None,
        }),
    );
    let mut elapsed = false;
    let confirmed = poll_fn(|cx| {
        if let Poll::Ready(result) = dialog.as_mut().poll(cx) {
            return Poll::Ready(result);
        }
        if !elapsed && let Poll::Ready(result) = delay.as_mut().poll(cx) {
            result?;
            elapsed = true;
            controller.abort()?;
            return dialog.as_mut().poll(cx);
        }
        Poll::Pending
    })
    .await?;
    drop(delay);
    let (message, kind) = if confirmed {
        ("Confirmed by user!", NotificationType::Info)
    } else if signal.aborted() {
        (
            "Dialog timed out (auto-cancelled)",
            NotificationType::Warning,
        )
    } else {
        ("Cancelled by user", NotificationType::Info)
    };
    ui.notify(message, Some(kind))
}

/// Runs the requested dialog and reports the host's decision.
async fn run(name: &str, api: &ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let ui = ctx.ui()?;
    match name {
        "timed" => {
            let confirmed = ui
                .confirm(
                    TITLE.into(),
                    MESSAGE.into(),
                    Some(ExtensionUIDialogOptions {
                        timeout: Some(5000.0),
                        signal: None,
                    }),
                )
                .await?;
            ui.notify(
                if confirmed {
                    "Confirmed by user!"
                } else {
                    "Cancelled or timed out"
                },
                Some(NotificationType::Info),
            )
        }
        "timed-select" => {
            let choice = ui
                .select(
                    "Pick an option".into(),
                    ["Option A", "Option B", "Option C"]
                        .map(str::to_owned)
                        .into(),
                    Some(ExtensionUIDialogOptions {
                        timeout: Some(10000.0),
                        signal: None,
                    }),
                )
                .await?;
            let message = choice.filter(|value| !value.is_empty()).map_or_else(
                || "Selection cancelled or timed out".into(),
                |choice| format!("Selected: {choice}"),
            );
            ui.notify(&message, Some(NotificationType::Info))
        }
        _ => timed_signal(api, &ui).await,
    }
}

/// Retains the factory's host for each independently invoked command.
fn handler(name: &'static str, api: ExtensionAPI) -> CommandHandler {
    Rc::new(move |_, ctx| {
        let api = api.clone();
        Box::pin(async move { run(name, &api, ctx).await })
    })
}

/// Registers the three opt-in timed-dialog commands.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        for (name, description) in [
            (
                "timed",
                "Show a timed confirmation dialog (auto-cancels in 5s with countdown)",
            ),
            (
                "timed-select",
                "Show a timed select dialog (auto-cancels in 10s with countdown)",
            ),
            (
                "timed-signal",
                "Show a timed confirm using AbortSignal (manual approach)",
            ),
        ] {
            api.register_command(
                name,
                CommandOptions {
                    description: Some(description.into()),
                    get_argument_completions: None,
                    handler: handler(name, api.clone()),
                },
            )?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
