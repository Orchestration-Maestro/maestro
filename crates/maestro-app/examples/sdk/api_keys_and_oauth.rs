//! Supply credential files, runtime keys and a catalog without a custom model file.
mod support;
use maestro_app::{AuthStorage, ModelRegistry, create_agent_session};
use maestro_credentials::{AuthCredential, ProcessConfigValueOperations};
use serde_json::{Value, json};
use std::io::Write;
use std::sync::Arc;
use support::{Error, Sample};

/// Run offline credential configuration; interactive OAuth needs an explicit provider login.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Create sessions with explicit credential owners and a non-persisted key override.
async fn run(sample: Sample) -> Result<(), Error> {
    let auth_path = sample.root.path().join("auth.json");
    let auth = Arc::new(AuthStorage::create(&auth_path.display().to_string()));
    auth.set(
        "faux",
        serde_json::from_value::<AuthCredential>(
            json!({"type": "api_key", "key": "stored-example-key"}),
        )?,
    );
    let commands = Arc::new(ProcessConfigValueOperations::new(|| {
        maestro_tools::utils::environment::get_shell_config(None)
            .map(|shell| (shell.shell, shell.args))
    }));
    let custom = custom_registry(&sample, commands.clone())?;
    for index in 0..4 {
        let selected = if index == 1 {
            Arc::clone(custom.auth_storage())
        } else {
            Arc::clone(&auth)
        };
        let mut options = sample.options()?;
        options.auth_storage = Some(Arc::clone(&selected));
        options.model_registry = (index == 1).then(|| Arc::clone(&custom));
        if index >= 2 {
            auth.set_runtime_api_key("faux", "runtime-example-key");
        }
        if index == 3 {
            let registry = Arc::new(ModelRegistry::in_memory(
                Arc::clone(&auth),
                commands.clone(),
            )?);
            options.model_registry = Some(registry);
        }
        let created = create_agent_session(options).await?;
        let label = [
            "default auth storage and model registry",
            "custom auth storage location",
            "runtime API key override",
            "only built-in models",
        ][index];
        writeln!(std::io::stdout(), "Session with {label}")?;
        created.session.dispose();
    }
    writeln!(
        std::io::stdout(),
        "Resolved key: {:?}",
        auth.get_api_key("faux", false, &*commands).await?
    )?;
    auth.remove_runtime_api_key("faux");
    writeln!(
        std::io::stdout(),
        "Key after removing the override: {:?}",
        auth.get_api_key("faux", false, &*commands).await?
    )?;
    Ok(())
}

/// Load credentials and model descriptors from a second directory.
fn custom_registry(
    sample: &Sample,
    commands: std::sync::Arc<maestro_credentials::ProcessConfigValueOperations>,
) -> Result<std::sync::Arc<maestro_app::ModelRegistry>, Error> {
    let custom_dir = sample.root.path().join("custom");
    std::fs::create_dir(&custom_dir)?;
    let custom = Arc::new(AuthStorage::create(
        &custom_dir.join("auth.json").display().to_string(),
    ));
    let mut models: Value =
        serde_json::from_slice(&std::fs::read(sample.root.path().join("models.json"))?)?;
    models["providers"]["faux"]["models"][0]["name"] = json!("Custom location model");
    let path = custom_dir.join("models.json");
    std::fs::write(&path, serde_json::to_vec(&models)?)?;
    Ok(Arc::new(maestro_app::ModelRegistry::create(
        custom,
        path.display().to_string(),
        commands,
    )?))
}
