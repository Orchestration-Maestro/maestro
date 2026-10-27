//! An opt-in file watcher that sends external triggers to the conversation.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CustomMessageInput, ExtensionAPI, ExtensionFuture, ExtensionResult, NotificationType, Presence,
    SendMessageOptions, UserContent, fs::FsWatcher,
};
use std::{cell::RefCell, rc::Rc};

/// The fixed file external writers update after the host starts watching.
const TRIGGER_FILE: &str = "/tmp/agent-trigger.txt";

/// Sends nonempty text, clearing the file only after the host accepts the message.
fn inject(api: &ExtensionAPI) -> ExtensionResult<()> {
    let bytes = std::fs::read(TRIGGER_FILE).map_err(|error| error.to_string())?;
    let content = String::from_utf8_lossy(&bytes);
    let content = content.trim();
    if !content.is_empty() {
        api.send_message(
            CustomMessageInput {
                custom_type: "file-trigger".into(),
                content: UserContent::Text(format!("External trigger: {content}")),
                display: true,
                details: Presence::Missing,
            },
            Some(SendMessageOptions {
                trigger_turn: Some(true),
                deliver_as: None,
            }),
        )?;
        std::fs::write(TRIGGER_FILE, "").map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Registers one retained watch, replaced at session start and released at shutdown.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let watch = Rc::new(RefCell::new(None::<Rc<dyn FsWatcher>>));
        let active = watch.clone();
        let sender = api.clone();
        api.on(
            "session_start",
            Rc::new(move |_, ctx| {
                let (watch, api) = (active.clone(), sender.clone());
                Box::pin(async move {
                    watch.borrow_mut().take();
                    let sender = api.clone();
                    *watch.borrow_mut() = Some(api.native_io().watch(
                        TRIGGER_FILE.into(),
                        Rc::new(move |_| {
                            let _ = inject(&sender);
                            Ok(())
                        }),
                    )?);
                    if ctx.has_ui()? {
                        ctx.ui()?.notify(
                            &format!("Watching {TRIGGER_FILE}"),
                            Some(NotificationType::Info),
                        )?;
                    }
                    Ok(None)
                })
            }),
        )?;
        api.on(
            "session_shutdown",
            Rc::new(move |_, _| {
                watch.borrow_mut().take();
                Box::pin(async { Ok(None) })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
