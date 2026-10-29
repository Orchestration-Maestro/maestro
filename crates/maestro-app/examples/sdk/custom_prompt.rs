//! Replace the system prompt or append instructions while keeping default discovery.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run both system-prompt configurations.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Return the assembled replacement and appended prompts.
async fn run(sample: Sample) -> Result<Value, Error> {
    std::fs::write(
        sample.root.path().join("APPEND_SYSTEM.md"),
        "Discovered append text",
    )?;
    let mut prompts = Vec::new();
    let mut messages = Vec::new();
    for replace in [true, false] {
        let mut options = sample.options()?;
        if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
            configure_prompt(resources, replace);
        }
        let created = maestro_app::create_agent_session(options).await?;
        prompts.push(created.session.system_prompt());
        let (question, answer) = if replace {
            ("What is 2 + 2?", "Four, Arrr!")
        } else {
            ("List 3 benefits of Rust.", "- Safety\n- Speed\n- Tooling")
        };
        sample
            .provider
            .set_responses(vec![maestro_models::FauxResponseStep::Message(Box::new(
                maestro_models::faux_assistant_message(
                    answer,
                    maestro_models::FauxAssistantMessageOptions::default(),
                ),
            ))]);
        created
            .session
            .prompt(question, maestro_app::PromptOptions::default())
            .await?;
        messages.push(serde_json::to_value(
            &*created
                .session
                .messages()
                .read()
                .map_err(|error| error.to_string())?,
        )?);
        created.session.dispose();
    }
    Ok(json!({"prompts": prompts, "messages": messages}))
}

/// Select replacement or appended instructions on the real resource loader.
fn configure_prompt(
    resources: &mut maestro_app::ResourceLoaderOptions<maestro_models::Local>,
    replace: bool,
) {
    if replace {
        resources.system_prompt_override = Some(Box::new(|_| {
            Some(
            "You are a helpful assistant that speaks like a pirate.\nAlways end responses with \"Arrr!\"".into()
        )
        }));
        resources.append_system_prompt_override = Some(Box::new(|_| Vec::new()));
    } else {
        resources.append_system_prompt_override = Some(Box::new(|mut base| {
            base.push("## Additional Instructions\n- Always be concise\n- Use bullet points when listing things".into());
            base
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each configuration is announced before its streamed answer.
    #[test]
    fn custom_prompt_prints_observations_in_order() -> Result<(), Error> {
        let output = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        support::execute(|mut sample| {
            sample.output = output.clone();
            run(sample)
        })?;
        assert_eq!(
            String::from_utf8(output.borrow().clone())?,
            "=== Replace prompt ===\nFour, Arrr!\n\n=== Modify prompt ===\n- Safety\n- Speed\n- Tooling\n"
        );
        Ok(())
    }

    /// Both prompt configurations perform a real conversation through the controlled model.
    #[test]
    fn custom_prompt_invokes_both_configurations() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(
            value["messages"][0][0]["content"][0]["text"],
            "What is 2 + 2?"
        );
        assert_eq!(value["messages"][0][1]["content"][0]["text"], "Four, Arrr!");
        assert_eq!(
            value["messages"][1][0]["content"][0]["text"],
            "List 3 benefits of Rust."
        );
        assert_eq!(
            value["messages"][1][1]["content"][0]["text"],
            "- Safety\n- Speed\n- Tooling"
        );
        Ok(())
    }

    /// Replacement removes discovered append text; appending retains the default prompt.
    #[test]
    fn custom_prompt_replaces_or_appends_instructions() -> Result<(), Error> {
        let value = support::execute(run)?;
        let replaced = value["prompts"][0].as_str().ok_or("missing replacement")?;
        let appended = value["prompts"][1]
            .as_str()
            .ok_or("missing appended prompt")?;
        assert!(replaced.starts_with("You are a helpful assistant that speaks like a pirate."));
        assert!(replaced.contains("Always end responses with \"Arrr!\""));
        assert!(!replaced.contains("Discovered append text"));
        assert!(appended.contains("Discovered append text"));
        assert!(appended.contains("Always be concise"));
        assert!(appended.contains("Use bullet points when listing things"));
        assert!(appended.contains("read"));
        Ok(())
    }
}
