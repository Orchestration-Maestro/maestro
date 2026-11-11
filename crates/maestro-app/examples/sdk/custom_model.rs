//! Select a catalog model explicitly and request a thinking level.
#[path = "support/stream.rs"]
mod stream;
mod support;
use std::io::Write;
use support::{Error, Sample};

/// Run the model-selection sample with the scripted protocol.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Resolve a model from the catalog and invoke it through the created session.
async fn run(sample: Sample) -> Result<(), Error> {
    use maestro_app::{PromptOptions, create_agent_session};
    use maestro_models::{
        FauxAssistantMessageOptions, FauxResponseStep, ModelThinkingLevel, faux_assistant_message,
    };
    use std::sync::Arc;
    let mut options = sample.options()?;
    let auth = Arc::clone(options.auth_storage.as_ref().ok_or("missing credentials")?);
    let registry = catalog(&sample, auth)?;
    observe_models(&registry)?;
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
    let finish = stream::subscribe(&created.session);
    created
        .session
        .prompt("Say hello in one sentence.", PromptOptions::default())
        .await?;
    finish()?;
    writeln!(std::io::stdout())?;
    created.session.dispose();
    Ok(())
}

/// Print each model-selection observation before invoking the selected model.
fn observe_models(registry: &maestro_app::ModelRegistry) -> Result<(), Error> {
    let built_in = registry.find("anthropic", "claude-opus-4-5").is_some();
    let available: Vec<String> = registry
        .get_available()?
        .iter()
        .map(|model| {
            let model = model.read().map_err(|error| error.to_string())?;
            Ok(format!("{}/{}", model.provider, model.id))
        })
        .collect::<Result<_, Error>>()?;
    if built_in {
        writeln!(std::io::stdout(), "Found model: anthropic/claude-opus-4-5")?;
    }
    writeln!(std::io::stdout(), "Found custom model: faux/faux-1")?;
    writeln!(
        std::io::stdout(),
        "Available models: {}",
        available.join(", ")
    )?;
    Ok(())
}

/// Load the isolated custom catalog beside the built-in descriptors.
fn catalog(
    sample: &Sample,
    auth: std::sync::Arc<maestro_credentials::AuthStorage>,
) -> Result<std::sync::Arc<maestro_app::ModelRegistry>, Error> {
    use maestro_app::ModelRegistry;
    use maestro_credentials::ProcessConfigValueOperations;
    use std::sync::Arc;
    let commands = Arc::new(ProcessConfigValueOperations::new(|| {
        maestro_tools::utils::environment::get_shell_config(None)
            .map(|shell| (shell.shell, shell.args))
    }));
    Ok(Arc::new(ModelRegistry::create(
        auth,
        sample.root.path().join("models.json").display().to_string(),
        commands,
    )?))
}
