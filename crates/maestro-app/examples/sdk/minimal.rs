//! Invoke a session with default tools and stream its text updates without a paid request.
#[path = "support/stream.rs"]
mod stream;
mod support;
use maestro_app::{PromptOptions, create_agent_session};
use maestro_models::{FauxAssistantMessageOptions, FauxResponseStep, faux_assistant_message};
use serde_json::{Value, json};
use std::rc::Rc;
use support::{Error, Sample};

/// Run the offline example and print its observed session state.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Prompt the session and return its streamed text and stored messages.
async fn run(sample: Sample) -> Result<Value, Error> {
    sample
        .provider
        .set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message(
                "The project contains AGENTS.md.",
                FauxAssistantMessageOptions::default(),
            ),
        ))]);
    let created = create_agent_session(sample.options()?).await?;
    let finish = stream::subscribe(&created.session, Rc::clone(&sample.output));
    created
        .session
        .prompt(
            "What files are in the current directory?",
            PromptOptions::default(),
        )
        .await?;
    let text = finish()?;
    writeln!(sample.output.borrow_mut())?;
    let messages = created.session.messages();
    let result = json!({
        "text": text, "messages": *messages.read().map_err(|error| error.to_string())?,
        "tools": created.session.get_active_tool_names(),
    });
    print_messages(
        &sample,
        &messages.read().map_err(|error| error.to_string())?,
    )?;
    created.session.dispose();
    Ok(result)
}

/// Show each stored text message after the live answer.
fn print_messages(
    sample: &Sample,
    messages: &[maestro_extensions::NativeAgentMessage],
) -> Result<(), Error> {
    use maestro_agent::AgentMessage;
    use maestro_models::AssistantContent;
    for message in messages {
        match message {
            AgentMessage::User(message) => {
                let message = message.read().map_err(|error| error.to_string())?;
                print_user(sample, &message.content)?;
            }
            AgentMessage::Assistant(message) => {
                let message = message.read().map_err(|error| error.to_string())?;
                for text in message.content.iter().filter_map(|block| match block {
                    AssistantContent::Text(text) => Some(&text.text),
                    _ => None,
                }) {
                    writeln!(sample.output.borrow_mut(), "Assistant: {text}")?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Print user text without treating attached images as text.
fn print_user(sample: &Sample, content: &maestro_models::UserContent) -> Result<(), Error> {
    use maestro_models::{UserBlock, UserContent};
    match content {
        UserContent::Text(text) => {
            writeln!(sample.output.borrow_mut(), "User: {text}")?;
        }
        UserContent::Blocks(blocks) => {
            for block in blocks {
                if let UserBlock::Text(text) = block {
                    writeln!(sample.output.borrow_mut(), "User: {}", text.text)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    mod stream_output;
    mod writer;
    use super::*;
    use std::cell::RefCell;

    /// Console text precedes the readable stored-message observations.
    #[test]
    fn minimal_prints_stream_then_messages() -> Result<(), Error> {
        let output = Rc::new(RefCell::new(Vec::new()));
        support::execute(|mut sample| {
            sample.output = output.clone();
            run(sample)
        })?;
        assert_eq!(
            String::from_utf8(output.borrow().clone())?,
            "The project contains AGENTS.md.\nUser: What files are in the current directory?\nAssistant: The project contains AGENTS.md.\n"
        );
        Ok(())
    }

    /// Stored user text supports both plain and mixed block content.
    #[test]
    fn minimal_prints_plain_and_mixed_user_content() -> Result<(), Error> {
        support::execute(|sample| async move {
            let output = Rc::new(RefCell::new(Vec::new()));
            let mut sample = sample;
            sample.output = output.clone();
            let messages = json!([
                {"role": "user", "content": "Plain input", "timestamp": 0},
                {"role": "user", "content": [
                    {"type": "text", "text": "Before image"},
                    {"type": "image", "data": "", "mimeType": "image/png"},
                    {"type": "text", "text": "After image"}
                ], "timestamp": 0}
            ]);
            let messages: Vec<maestro_extensions::NativeAgentMessage> =
                serde_json::from_value(messages)?;
            print_messages(&sample, &messages)?;
            assert_eq!(
                String::from_utf8(output.borrow().clone())?,
                "User: Plain input\nUser: Before image\nUser: After image\n"
            );
            Ok(Value::Null)
        })?;
        Ok(())
    }

    /// A streamed answer also becomes part of the session's public history.
    #[test]
    fn minimal_streams_and_retains_the_answer() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(value["text"], "The project contains AGENTS.md.");
        assert_eq!(value["messages"][0]["role"], "user");
        assert_eq!(value["messages"][1]["role"], "assistant");
        assert_eq!(value["messages"][1]["content"][0]["text"], value["text"]);
        assert_eq!(value["tools"], json!(["read", "bash", "edit", "write"]));
        Ok(())
    }
}
