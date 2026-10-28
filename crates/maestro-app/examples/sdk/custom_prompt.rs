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
    for replace in [true, false] {
        let mut options = sample.options()?;
        if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
            configure_prompt(resources, replace);
        }
        let created = maestro_app::create_agent_session(options).await?;
        prompts.push(created.session.system_prompt());
        created.session.dispose();
    }
    Ok(json!(prompts))
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

    /// Replacement removes discovered append text; appending retains the default prompt.
    #[test]
    fn custom_prompt_replaces_or_appends_instructions() -> Result<(), Error> {
        let local = std::rc::Rc::new(tokio::task::LocalSet::new());
        let sample = Sample::new(std::rc::Rc::clone(&local))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let value = runtime.block_on(local.run_until(run(sample)))?;
        let replaced = value[0].as_str().ok_or("missing replacement")?;
        let appended = value[1].as_str().ok_or("missing appended prompt")?;
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
