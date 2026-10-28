//! Supply credential files, runtime keys and a catalog without a custom model file.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run offline credential configuration; interactive OAuth needs an explicit provider login.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Create sessions with explicit credential owners and observe a non-persisted key override.
async fn run(sample: Sample) -> Result<Value, Error> {
    use maestro_app::{AuthStorage, ModelRegistry, create_agent_session};
    use maestro_credentials::{AuthCredential, ProcessConfigValueOperations};
    use std::sync::Arc;
    let auth_path = sample.root.path().join("auth.json");
    let auth = Arc::new(AuthStorage::create(&auth_path.display().to_string()));
    auth.set(
        "faux",
        serde_json::from_value::<AuthCredential>(
            json!({"type": "api_key", "key": "stored-example-key"}),
        )?,
    );
    let custom_dir = sample.root.path().join("custom");
    std::fs::create_dir(&custom_dir)?;
    let custom = Arc::new(AuthStorage::create(
        &custom_dir.join("auth.json").display().to_string(),
    ));
    let commands = Arc::new(ProcessConfigValueOperations::new(|| {
        maestro_tools::utils::shell::get_shell_config(None).map(|shell| (shell.shell, shell.args))
    }));
    let mut result = json!({"sessions": 0});
    for index in 0..4 {
        let selected = if index == 1 {
            Arc::clone(&custom)
        } else {
            Arc::clone(&auth)
        };
        let mut options = sample.options()?;
        options.auth_storage = Some(Arc::clone(&selected));
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
        result["sameAuth"] = json!(Arc::ptr_eq(
            created.session.model_registry().auth_storage(),
            &selected
        ));
        result["sessions"] = json!(index + 1);
        created.session.dispose();
    }
    result["resolved"] = json!(auth.get_api_key("faux", false, &*commands).await?);
    result["stored"] =
        serde_json::from_slice::<Value>(&std::fs::read(&auth_path)?)?["faux"]["key"].clone();
    auth.remove_runtime_api_key("faux");
    result["restored"] = json!(auth.get_api_key("faux", false, &*commands).await?);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A runtime key wins without overwriting the credential file or replacing its owner.
    #[test]
    fn api_keys_keeps_runtime_overrides_out_of_persistent_credentials() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(value["stored"], "stored-example-key");
        assert_eq!(value["resolved"], "runtime-example-key");
        assert_eq!(value["restored"], "stored-example-key");
        assert_eq!(value["sameAuth"], true);
        assert_eq!(value["withoutCustomModel"], true);
        assert_eq!(value["sessions"], 4);
        Ok(())
    }
}
