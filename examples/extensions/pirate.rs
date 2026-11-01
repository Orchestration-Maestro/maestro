//! A command that toggles a pirate response style in the assembled system prompt.
// maestro-component: MAESTRO_PIRATE_COMPONENT
#![cfg(any(test, target_arch = "wasm32"))]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::Cell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    BeforeAgentStartEventResult, CommandHandler, CommandOptions, ExtensionAPI, ExtensionEvent,
    ExtensionEventResult, ExtensionFuture, ExtensionHandler, NotificationType, Presence,
};

/// The instruction appended to the assembled prompt while the mode is on.
const PIRATE_PROMPT: &str = "\n\nIMPORTANT: You are now in PIRATE MODE. You must:\n- Speak like a stereotypical pirate in all responses\n- Use phrases like \"Arrr!\", \"Ahoy!\", \"Shiver me timbers!\", \"Avast!\", \"Ye scurvy dog!\"\n- Replace \"my\" with \"me\", \"you\" with \"ye\", \"your\" with \"yer\"\n- Refer to the user as \"matey\" or \"landlubber\"\n- End sentences with nautical expressions\n- Still complete the actual task correctly, just in pirate speak\n";

/// What the command reports when it turns the mode on.
const ENABLED: &str = "Arrr! Pirate mode enabled!";

/// What the command reports when it turns the mode off.
const DISABLED: &str = "Pirate mode disabled";

/// Registers the toggle command and the prompt handler that reads its state.
#[must_use]
pub fn pirate_extension(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    let enabled = Rc::new(Cell::new(false));
    let toggled = Rc::clone(&enabled);
    let toggle: CommandHandler = Rc::new(move |_, context| {
        let now = !toggled.get();
        toggled.set(now);
        let message = if now { ENABLED } else { DISABLED };
        Box::pin(async move { context.ui()?.notify(message, Some(NotificationType::Info)) })
    });
    let prompt: ExtensionHandler = Rc::new(move |event, _| {
        let appended = enabled.get();
        Box::pin(async move {
            let ExtensionEvent::BeforeAgentStart(event) = event else {
                return Ok(None);
            };
            Ok(appended.then(|| {
                ExtensionEventResult::BeforeAgentStart(BeforeAgentStartEventResult {
                    message: Presence::Missing,
                    system_prompt: Presence::Present(format!(
                        "{}{PIRATE_PROMPT}",
                        event.system_prompt
                    )),
                })
            }))
        })
    });
    Box::pin(async move {
        api.register_command(
            "pirate",
            CommandOptions {
                description: Some("Toggle pirate mode (agent speaks like a pirate)".into()),
                get_argument_completions: None,
                handler: toggle,
            },
        )?;
        api.on("before_agent_start", prompt)
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(pirate_extension);
