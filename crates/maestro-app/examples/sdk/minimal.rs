//! Invoke a session with default tools and stream its text updates without a paid request.
mod support;
use maestro_agent::AgentEvent;
use maestro_app::{AgentSessionEvent, PromptOptions, create_agent_session};
use maestro_models::{
    AssistantMessageEvent, FauxAssistantMessageOptions, FauxResponseStep, faux_assistant_message,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
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
    let text = Rc::new(RefCell::new(String::new()));
    let streamed = Rc::clone(&text);
    let unsubscribe = created.session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && let Ok(event) = event.read()
            && let AgentEvent::MessageUpdate {
                assistant_message_event: AssistantMessageEvent::TextDelta { delta, .. },
                ..
            } = &*event
        {
            streamed.borrow_mut().push_str(delta);
        }
    }));
    created
        .session
        .prompt(
            "What files are in the current directory?",
            PromptOptions::default(),
        )
        .await?;
    unsubscribe();
    let messages = created.session.messages();
    let result = json!({
        "text": *text.borrow(), "messages": *messages.read().map_err(|error| error.to_string())?,
        "tools": created.session.get_active_tool_names(),
    });
    created.session.dispose();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

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
