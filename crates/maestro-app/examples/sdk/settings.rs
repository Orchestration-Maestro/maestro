//! Override session preferences and flush a persistent settings change.
mod support;
use serde_json::json;
use std::io::Write;
use support::{Error, Sample};

/// Run the settings sample without touching user preferences.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Print the live overrides, flush a durable preference and use in-memory preferences.
async fn run(sample: Sample) -> Result<(), Error> {
    use maestro_app::SettingsManager;
    use std::{cell::RefCell, rc::Rc};
    let options = configured_options(&sample).await?;
    let created = maestro_app::create_agent_session(options).await?;
    writeln!(std::io::stdout(), "Session created with custom settings")?;
    let manager = created.session.settings_manager();
    manager
        .borrow_mut()
        .set_default_thinking_level(maestro_settings::ThinkingLevel::Low);
    let flushed = manager.borrow().flush();
    flushed.await;
    let errors = manager.borrow_mut().drain_errors();
    for error in &errors {
        writeln!(
            std::io::stdout(),
            "Warning ({} settings): {}",
            error.scope.label(),
            error.error
        )?;
    }
    created.session.dispose();
    let mut options = sample.options()?;
    options.settings_manager = Some(Rc::new(RefCell::new(SettingsManager::in_memory(
        serde_json::from_value(
            json!({"compaction": {"enabled": false}, "retry": {"enabled": false}}),
        )?,
    ))));
    let memory = maestro_app::create_agent_session(options).await?;
    writeln!(
        std::io::stdout(),
        "Test session created with in-memory settings"
    )?;
    memory.session.dispose();
    Ok(())
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
    writeln!(
        std::io::stdout(),
        "Current settings: {}",
        serde_json::to_string_pretty(&settings.borrow().get_global_settings())?
    )?;
    settings.borrow_mut().apply_overrides(serde_json::from_value(json!({
        "compaction": {"enabled": false}, "retry": {"enabled": true, "maxRetries": 5, "baseDelayMs": 1000}
    }))?);
    options.resources = SessionResources::Loader(loader);
    options.settings_manager = Some(settings);
    Ok(options)
}
