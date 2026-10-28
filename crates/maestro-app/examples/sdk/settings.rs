//! Override session preferences and flush a persistent settings change.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the settings sample without touching user preferences.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Return live overrides, durable preferences and in-memory session preferences.
async fn run(sample: Sample) -> Result<Value, Error> {
    use maestro_app::SettingsManager;
    use std::{cell::RefCell, rc::Rc};
    let options = configured_options(&sample).await?;
    let created = maestro_app::create_agent_session(options).await?;
    let mut result = {
        let settings = created.session.settings_manager().borrow();
        json!({
            "retry": settings.get_retry_enabled(), "maxRetries": settings.get_retry_settings().max_retries,
            "compaction": settings.get_compaction_enabled(),
        })
    };
    let manager = created.session.settings_manager();
    manager
        .borrow_mut()
        .set_default_thinking_level(maestro_settings::ThinkingLevel::Low);
    let flushed = manager.borrow().flush();
    flushed.await;
    result["errors"] = json!(manager.borrow_mut().drain_errors().len());
    result["saved"] =
        serde_json::from_slice(&std::fs::read(sample.root.path().join("settings.json"))?)?;
    created.session.dispose();
    let mut options = sample.options()?;
    options.settings_manager = Some(Rc::new(RefCell::new(SettingsManager::in_memory(
        serde_json::from_value(
            json!({"compaction": {"enabled": false}, "retry": {"enabled": false}}),
        )?,
    ))));
    let memory = maestro_app::create_agent_session(options).await?;
    result["memoryRetry"] = json!(
        memory
            .session
            .settings_manager()
            .borrow()
            .get_retry_enabled()
    );
    memory.session.dispose();
    Ok(result)
}

/// Reload resources before applying runtime settings overrides.
async fn configured_options(
    sample: &Sample,
) -> Result<maestro_app::CreateAgentSessionOptions, Error> {
    use maestro_app::{
        DefaultResourceLoader, DefaultResourceLoaderOptions, ResourceLoader, SessionResources,
        SettingsManager,
    };
    use std::{cell::RefCell, rc::Rc};
    let cwd = sample.root.path().display().to_string();
    let settings = Rc::new(RefCell::new(SettingsManager::create(
        &cwd, &cwd, ".maestro",
    )));
    let mut options = sample.options()?;
    let SessionResources::Default(resources) = options.resources else {
        return Err("The sample requires default resource discovery".into());
    };
    let loader = Rc::new(DefaultResourceLoader::new(DefaultResourceLoaderOptions {
        cwd: cwd.clone(),
        agent_dir: cwd,
        settings_manager: Some(Rc::clone(&settings)),
        resources: *resources,
    }));
    loader.reload().await?;
    settings.borrow_mut().apply_overrides(serde_json::from_value(json!({
        "compaction": {"enabled": false}, "retry": {"enabled": true, "maxRetries": 5, "baseDelayMs": 1000}
    }))?);
    options.resources = SessionResources::Loader(loader);
    options.settings_manager = Some(settings);
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Overrides affect the session while the explicit setter crosses the durability boundary.
    #[test]
    fn settings_applies_overrides_and_flushes_explicit_changes() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(value["retry"], true);
        assert_eq!(value["maxRetries"], 5.0);
        assert_eq!(value["compaction"], false);
        assert_eq!(value["saved"]["defaultThinkingLevel"], "low");
        assert!(value["saved"].get("retry").is_none());
        assert_eq!(value["errors"], 0);
        assert_eq!(value["memoryRetry"], false);
        Ok(())
    }
}
