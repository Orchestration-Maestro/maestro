//! Invoke a session with default tools and stream its text updates without a paid request.
#[path = "support/stream.rs"]
mod stream;
mod support;
use maestro_app::{PromptOptions, create_agent_session};
use maestro_models::{FauxAssistantMessageOptions, FauxResponseStep, faux_assistant_message};
use std::io::Write;
use support::{Error, Sample};

/// Run the offline example and print its observed session state.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Prompt the session, stream the answer and print the stored messages.
async fn run(sample: Sample) -> Result<(), Error> {
    sample
        .provider
        .set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message(
                "The project contains AGENTS.md.",
                FauxAssistantMessageOptions::default(),
            ),
        ))]);
    let created = create_agent_session(sample.options()?).await?;
    let finish = stream::subscribe(&created.session);
    created
        .session
        .prompt(
            "What files are in the current directory?",
            PromptOptions::default(),
        )
        .await?;
    finish()?;
    writeln!(std::io::stdout())?;
    let messages = created.session.messages();
    print_messages(&messages.read().map_err(|error| error.to_string())?)?;
    created.session.dispose();
    Ok(())
}

/// Show each stored text message after the live answer.
fn print_messages(messages: &[maestro_extensions::NativeAgentMessage]) -> Result<(), Error> {
    use maestro_agent::AgentMessage;
    use maestro_models::AssistantContent;
    for message in messages {
        match message {
            AgentMessage::User(message) => {
                let message = message.read().map_err(|error| error.to_string())?;
                print_user(&message.content)?;
            }
            AgentMessage::Assistant(message) => {
                let message = message.read().map_err(|error| error.to_string())?;
                for text in message.content.iter().filter_map(|block| match block {
                    AssistantContent::Text(text) => Some(&text.text),
                    _ => None,
                }) {
                    writeln!(std::io::stdout(), "Assistant: {text}")?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Print user text without treating attached images as text.
fn print_user(content: &maestro_models::UserContent) -> Result<(), Error> {
    use maestro_models::{UserBlock, UserContent};
    match content {
        UserContent::Text(text) => {
            writeln!(std::io::stdout(), "User: {text}")?;
        }
        UserContent::Blocks(blocks) => {
            for block in blocks {
                if let UserBlock::Text(text) = block {
                    writeln!(std::io::stdout(), "User: {}", text.text)?;
                }
            }
        }
    }
    Ok(())
}
