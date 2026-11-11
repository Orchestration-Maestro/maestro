//! Replace the system prompt or append instructions while keeping default discovery.
#[path = "support/stream.rs"]
mod stream;
mod support;
use std::io::Write;
use support::{Error, Sample};

/// Run both system-prompt configurations.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Prompt a session with each system-prompt configuration.
async fn run(sample: Sample) -> Result<(), Error> {
    std::fs::write(
        sample.root.path().join("APPEND_SYSTEM.md"),
        "Discovered append text",
    )?;
    for replace in [true, false] {
        let mut options = sample.options()?;
        if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
            configure_prompt(resources, replace);
        }
        let created = maestro_app::create_agent_session(options).await?;
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
        writeln!(
            std::io::stdout(),
            "=== {} prompt ===",
            if replace { "Replace" } else { "Modify" }
        )?;
        let finish = stream::subscribe(&created.session);
        created
            .session
            .prompt(question, maestro_app::PromptOptions::default())
            .await?;
        finish()?;
        writeln!(std::io::stdout(), "{}", if replace { "\n" } else { "" })?;
        created.session.dispose();
    }
    Ok(())
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
