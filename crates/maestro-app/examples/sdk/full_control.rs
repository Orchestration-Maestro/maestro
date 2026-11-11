//! Supply credentials, a model, preferences, tools and resources explicitly.
#[path = "support/stream.rs"]
mod stream;
mod support;
use serde_json::json;
use std::io::Write;
use support::{Error, Sample};

/// Run the explicit-configuration sample with no resource discovery.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Invoke a session built entirely from caller-selected inputs.
async fn run(sample: Sample) -> Result<(), Error> {
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
    let finish = stream::subscribe(&created.session);
    created
        .session
        .prompt(
            "List files in the current directory.",
            PromptOptions::default(),
        )
        .await?;
    finish()?;
    writeln!(std::io::stdout())?;
    created.session.dispose();
    Ok(())
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
