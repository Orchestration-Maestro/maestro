//! Select a catalog model explicitly and request a thinking level.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the model-selection sample with the scripted protocol.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Resolve a model from the catalog and invoke it through the created session.
async fn run(sample: Sample) -> Result<Value, Error> {
    use maestro_app::{ModelRegistry, PromptOptions, create_agent_session};
    use maestro_credentials::ProcessConfigValueOperations;
    use maestro_models::{
        FauxAssistantMessageOptions, FauxResponseStep, ModelThinkingLevel, faux_assistant_message,
    };
    use std::sync::Arc;
    let mut options = sample.options()?;
    let auth = Arc::clone(options.auth_storage.as_ref().ok_or("missing credentials")?);
    let commands = Arc::new(ProcessConfigValueOperations::new(|| {
        maestro_tools::utils::shell::get_shell_config(None).map(|shell| (shell.shell, shell.args))
    }));
    let registry = Arc::new(ModelRegistry::create(
        auth,
        sample.root.path().join("models.json").display().to_string(),
        commands,
    )?);
    let built_in = registry.find("anthropic", "claude-opus-4-5").is_some();
    let available: Vec<String> = registry
        .get_available()?
        .iter()
        .map(|model| {
            let model = model.read().map_err(|error| error.to_string())?;
            Ok(format!("{}/{}", model.provider, model.id))
        })
        .collect::<Result<_, Error>>()?;
    options.model = Some(
        registry
            .find("faux", "faux-1")
            .ok_or("missing sample model")?,
    );
    options.model_registry = Some(Arc::clone(&registry));
    options.thinking_level = Some(ModelThinkingLevel::Medium);
    let created = create_agent_session(options).await?;
    sample
        .provider
        .set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message("Hello.", FauxAssistantMessageOptions::default()),
        ))]);
    created
        .session
        .prompt("Say hello in one sentence.", PromptOptions::default())
        .await?;
    let model = created.session.model();
    let model = model.read().map_err(|error| error.to_string())?;
    let messages = serde_json::to_value(
        &*created
            .session
            .messages()
            .read()
            .map_err(|error| error.to_string())?,
    )?;
    let result = json!({"selected": format!("{}/{}", model.provider, model.id),
        "thinking": created.session.thinking_level(), "sameCatalog": Arc::ptr_eq(created.session.model_registry(), &registry),
        "builtInFound": built_in, "available": available, "answer": messages[1]["content"][0]["text"]});
    created.session.dispose();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Catalog observations appear before the streamed greeting.
    #[test]
    fn custom_model_prints_observations_in_order() -> Result<(), Error> {
        let output = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        support::execute(|mut sample| {
            sample.output = output.clone();
            run(sample)
        })?;
        let text = String::from_utf8(output.borrow().clone())?;
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines[0], "Found model: anthropic/claude-opus-4-5");
        assert_eq!(lines[1], "Found custom model: faux/faux-1");
        assert!(lines[2].starts_with("Available models: "));
        assert!(lines[2].contains("faux/faux-1"));
        assert_eq!(lines[3..], ["Hello."]);
        Ok(())
    }

    /// Explicit model selection keeps the caller's catalog and clamps unsupported thinking.
    #[test]
    fn custom_model_selects_and_prompts_the_catalog_model() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(value["selected"], "faux/faux-1");
        assert_eq!(value["thinking"], "off");
        assert_eq!(value["sameCatalog"], true);
        assert_eq!(value["builtInFound"], true);
        assert!(
            value["available"]
                .as_array()
                .ok_or("missing available models")?
                .contains(&json!("faux/faux-1"))
        );
        assert_eq!(value["answer"], "Hello.");
        Ok(())
    }
}
