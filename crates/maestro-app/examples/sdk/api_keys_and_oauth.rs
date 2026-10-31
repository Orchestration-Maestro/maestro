//! Supply credential files, runtime keys and a catalog without a custom model file.
mod support;
use maestro_app::{AuthStorage, ModelRegistry, create_agent_session};
use maestro_credentials::{AuthCredential, ProcessConfigValueOperations};
use serde_json::{Value, json};
use std::sync::Arc;
use support::{Error, Sample};

/// Run offline credential configuration; interactive OAuth needs an explicit provider login.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Create sessions with explicit credential owners and observe a non-persisted key override.
async fn run(sample: Sample) -> Result<Value, Error> {
    let auth_path = sample.root.path().join("auth.json");
    let auth = Arc::new(AuthStorage::create(&auth_path.display().to_string()));
    auth.set(
        "faux",
        serde_json::from_value::<AuthCredential>(
            json!({"type": "api_key", "key": "stored-example-key"}),
        )?,
    );
    let commands = Arc::new(ProcessConfigValueOperations::new(|| {
        maestro_tools::utils::shell::get_shell_config(None).map(|shell| (shell.shell, shell.args))
    }));
    let custom = custom_registry(&sample, commands.clone())?;
    let mut result = json!({"sessions": 0});
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
            result["withoutCustomModel"] = json!(registry.find("faux", "faux-1").is_none());
            options.model_registry = Some(registry);
        }
        let created = create_agent_session(options).await?;
        if index == 1 {
            observe_custom(&created.session, &custom, &mut result)?;
        }
        result["sameAuth"] = json!(Arc::ptr_eq(
            created.session.model_registry().auth_storage(),
            &selected
        ));
        result["sessions"] = json!(index + 1);
        let label = [
            "default auth storage and model registry",
            "custom auth storage location",
            "runtime API key override",
            "only built-in models",
        ][index];
        writeln!(sample.output.borrow_mut(), "Session with {label}")?;
        created.session.dispose();
    }
    result["resolved"] = json!(auth.get_api_key("faux", false, &*commands).await?);
    result["stored"] =
        serde_json::from_slice::<Value>(&std::fs::read(&auth_path)?)?["faux"]["key"].clone();
    auth.remove_runtime_api_key("faux");
    result["restored"] = json!(auth.get_api_key("faux", false, &*commands).await?);
    Ok(result)
}

/// Record the explicit catalog handle and its directory-specific descriptor.
fn observe_custom(
    session: &maestro_app::AgentSession,
    custom: &std::sync::Arc<maestro_app::ModelRegistry>,
    result: &mut Value,
) -> Result<(), Error> {
    result["sameCustomRegistry"] = json!(Arc::ptr_eq(session.model_registry(), custom));
    result["customModelName"] = json!(
        custom
            .find("faux", "faux-1")
            .ok_or("missing custom model")?
            .read()
            .map_err(|error| error.to_string())?
            .name
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Credential configurations are announced without printing any keys.
    #[test]
    fn api_keys_and_oauth_prints_observations_in_order() -> Result<(), Error> {
        let output = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        support::execute(|mut sample| {
            sample.output = output.clone();
            run(sample)
        })?;
        assert_eq!(
            String::from_utf8(output.borrow().clone())?,
            "Session with default auth storage and model registry\nSession with custom auth storage location\nSession with runtime API key override\nSession with only built-in models\n"
        );
        Ok(())
    }

    /// A runtime key wins without overwriting the credential file or replacing its owner.
    #[test]
    fn api_keys_keeps_runtime_overrides_out_of_persistent_credentials() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(value["sameCustomRegistry"], true);
        assert_eq!(value["customModelName"], "Custom location model");
        assert_eq!(value["stored"], "stored-example-key");
        assert_eq!(value["resolved"], "runtime-example-key");
        assert_eq!(value["restored"], "stored-example-key");
        assert_eq!(value["sameAuth"], true);
        assert_eq!(value["withoutCustomModel"], true);
        assert_eq!(value["sessions"], 4);
        Ok(())
    }
}
