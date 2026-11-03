//! Supply credentials, a model, preferences, tools and resources explicitly.
#[path = "support/stream.rs"]
mod stream;
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the explicit-configuration sample with no resource discovery.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Invoke a session built entirely from caller-selected inputs.
async fn run(sample: Sample) -> Result<Value, Error> {
    use maestro_app::{PromptOptions, create_agent_session};
    use maestro_models::{FauxAssistantMessageOptions, FauxResponseStep, faux_assistant_message};
    std::fs::write(
        sample.root.path().join("AGENTS.md"),
        "Unwanted disk instruction",
    )?;
    let options = configured_options(&sample)?;
    let created = create_agent_session(options).await?;
    sample
        .provider
        .set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message(
                "Only the supplied read and bash tools are available.",
                FauxAssistantMessageOptions::default(),
            ),
        ))]);
    let finish = stream::subscribe(&created.session, std::rc::Rc::clone(&sample.output));
    created
        .session
        .prompt(
            "List files in the current directory.",
            PromptOptions::default(),
        )
        .await?;
    finish()?;
    writeln!(sample.output.borrow_mut())?;
    let loader = created.session.resource_loader();
    let messages = serde_json::to_value(
        &*created
            .session
            .messages()
            .read()
            .map_err(|error| error.to_string())?,
    )?;
    let result = json!({"tools": created.session.get_active_tool_names(),
        "skills": loader.get_skills().skills.len(), "templates": loader.get_prompts().prompts.len(),
        "contextFiles": loader.get_agents_files().len(), "thinking": created.session.thinking_level(),
        "prompt": created.session.system_prompt(), "answer": messages[1]["content"][0]["text"]});
    created.session.dispose();
    Ok(result)
}

/// Disable resource discovery while retaining the existing loader's public contract.
fn configured_options(sample: &Sample) -> Result<maestro_app::CreateAgentSessionOptions, Error> {
    use maestro_app::{ResourceKind, SessionResources, SettingsManager};
    use maestro_models::ModelThinkingLevel;
    use std::{cell::RefCell, rc::Rc};
    let mut options = sample.options()?;
    options.tools = Some(vec!["read".into(), "bash".into()]);
    options.thinking_level = Some(ModelThinkingLevel::Off);
    options.settings_manager = Some(Rc::new(RefCell::new(SettingsManager::in_memory(
        serde_json::from_value(
            json!({"compaction": {"enabled": false}, "retry": {"enabled": true, "maxRetries": 2}}),
        )?,
    ))));
    if let SessionResources::Default(resources) = &mut options.resources {
        resources.disabled.extend([
            ResourceKind::Extensions,
            ResourceKind::Skills,
            ResourceKind::PromptTemplates,
            ResourceKind::Themes,
            ResourceKind::ContextFiles,
        ]);
        resources.system_prompt =
            Some("You are a minimal assistant.\nAvailable: read, bash. Be concise.".into());
        resources.append_system_prompt = Some(Vec::new());
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Explicit configuration still presents the model's text as a live answer.
    #[test]
    fn full_control_prints_observations_in_order() -> Result<(), Error> {
        let output = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        support::execute(|mut sample| {
            sample.output = output.clone();
            run(sample)
        })?;
        assert_eq!(
            String::from_utf8(output.borrow().clone())?,
            "Only the supplied read and bash tools are available.\n"
        );
        Ok(())
    }

    /// Explicit resources exclude disk instructions and the tool allowlist reaches the session.
    #[test]
    fn full_control_uses_only_the_supplied_configuration() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(value["tools"], json!(["read", "bash"]));
        assert_eq!(value["skills"], 0);
        assert_eq!(value["templates"], 0);
        assert_eq!(value["contextFiles"], 0);
        assert_eq!(value["thinking"], "off");
        let prompt = value["prompt"].as_str().ok_or("missing prompt")?;
        assert!(prompt.starts_with("You are a minimal assistant."));
        assert!(!prompt.contains("Unwanted disk instruction"));
        assert_eq!(
            value["answer"],
            "Only the supplied read and bash tools are available."
        );
        Ok(())
    }
}
