//! User-message delivery: commands that send ordinary, steering, follow-up and structured
//! messages, so a user can reach the agent from an extension.
//!
//! `/ask` sends an ordinary message, which starts a turn. `/steer` sends while the agent is
//! working and interrupts that work. `/followup` queues a message that waits for the current
//! work to finish. `/askwith` sends two text blocks instead of one string.
// maestro-component: MAESTRO_SEND_USER_MESSAGE_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    CommandHandler, CommandOptions, ExtensionAPI, ExtensionCommandContext, ExtensionFuture,
    ExtensionResult, NotificationType, SendUserMessageOptions, TextContent, UserBlock, UserContent,
    UserMessageDelivery,
};

/// The delivery one registered command asks for.
#[derive(Clone, Copy)]
enum Command {
    /// An ordinary message.
    Ask,
    /// A message that interrupts the work in flight.
    Steer,
    /// A message that waits for the current work.
    Followup,
    /// An ordinary message and a second instruction.
    Askwith,
}

impl Command {
    /// The warning shown when the command has no message.
    fn usage(self) -> &'static str {
        match self {
            Self::Ask => "Usage: /ask <message>",
            Self::Steer => "Usage: /steer <message>",
            Self::Followup => "Usage: /followup <message>",
            Self::Askwith => "Usage: /askwith <message>",
        }
    }
}

/// Sends the command's message, or reports why it sends none.
fn handle(
    api: &ExtensionAPI,
    command: Command,
    args: String,
    ctx: &ExtensionCommandContext,
) -> ExtensionResult<()> {
    if args.trim().is_empty() {
        return ctx
            .ui()?
            .notify(command.usage(), Some(NotificationType::Warning));
    }
    let idle = ctx.is_idle()?;
    if !idle && matches!(command, Command::Ask | Command::Askwith) {
        let warning = if matches!(command, Command::Ask) {
            "Agent is busy. Use /steer or /followup instead."
        } else {
            "Agent is busy"
        };
        return ctx.ui()?.notify(warning, Some(NotificationType::Warning));
    }
    let delivery = match (command, idle) {
        (Command::Steer, false) => Some(UserMessageDelivery::Steer),
        (Command::Followup, false) => Some(UserMessageDelivery::FollowUp),
        _ => None,
    };
    let content = if matches!(command, Command::Askwith) {
        UserContent::Blocks(vec![
            UserBlock::Text(TextContent::new(format!("User request: {args}"))),
            UserBlock::Text(TextContent::new("Please respond concisely.".into())),
        ])
    } else {
        UserContent::Text(args)
    };
    api.send_user_message(
        content,
        delivery.map(|delivery| SendUserMessageOptions {
            deliver_as: Some(delivery),
        }),
    )?;
    if !idle && matches!(command, Command::Followup) {
        ctx.ui()?
            .notify("Follow-up queued", Some(NotificationType::Info))?;
    }
    Ok(())
}

/// The handler of one command: it sends the argument text to [`handle`] through a host handle it
/// keeps for the invocation.
fn registered(api: ExtensionAPI, command: Command) -> CommandHandler {
    Rc::new(move |args, ctx| {
        let messages = api.clone();
        Box::pin(async move { handle(&messages, command, args, &ctx) })
    })
}

/// Registers the ordinary, steering, follow-up and structured user-message commands.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        for (name, description, command) in [
            ("ask", "Send a user message to the agent", Command::Ask),
            (
                "steer",
                "Send a steering message (interrupts current processing)",
                Command::Steer,
            ),
            (
                "followup",
                "Queue a follow-up message (waits for current processing)",
                Command::Followup,
            ),
            (
                "askwith",
                "Send a user message with structured content",
                Command::Askwith,
            ),
        ] {
            api.register_command(
                name,
                CommandOptions {
                    description: Some(description.into()),
                    get_argument_completions: None,
                    handler: registered(api.clone(), command),
                },
            )?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
