//! A command that emits events for other extensions, and a listener that reports them.
// maestro-component: MAESTRO_EVENT_BUS_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionContext, ExtensionFuture, NotificationType,
};
use serde::Deserialize;
use serde_json::json;

/// The string members the notification listener reports.
#[derive(Deserialize)]
struct Notification {
    /// Literal message.
    message: String,
    /// Literal sender.
    from: String,
}

/// The options of the command that emits its argument as a notification.
fn emit_command(api: ExtensionAPI) -> CommandOptions {
    CommandOptions {
        description: Some("Emit my:notification event (usage: /emit message)".into()),
        get_argument_completions: None,
        handler: Rc::new(move |args, _| {
            let bus = api.events();
            Box::pin(async move {
                let message = args.trim();
                let message = if message.is_empty() { "hello" } else { message };
                bus.emit(
                    "my:notification",
                    json!({"message":message,"from":"/emit command"}),
                )
                .await
            })
        }),
    }
}

/// Registers the emit command, the listener retained across sessions, and the emission at
/// session start.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let current = Rc::new(RefCell::new(None::<ExtensionContext>));
        let stored = Rc::clone(&current);
        api.on(
            "session_start",
            Rc::new(move |_, context| {
                stored.replace(Some(context));
                Box::pin(async { Ok(None) })
            }),
        )?;
        api.events().on(
            "my:notification",
            Rc::new(move |data, _| {
                let notification: Notification =
                    serde_json::from_value(data).map_err(|error| error.to_string())?;
                if let Some(context) = current.borrow().clone() {
                    context.ui()?.notify(
                        &format!("Event from {}: {}", notification.from, notification.message),
                        Some(NotificationType::Info),
                    )?;
                }
                Ok(None)
            }),
        )?;
        api.register_command("emit", emit_command(api.clone()))?;
        let startup = api.clone();
        api.on(
            "session_start",
            Rc::new(move |_, _| {
                let bus = startup.events();
                Box::pin(async move {
                    bus.emit(
                        "my:notification",
                        json!({"message":"Session started","from":"event-bus-example"}),
                    )
                    .await?;
                    Ok(None)
                })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
