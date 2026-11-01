//! Controlled settings, catalogs and stored histories shared by the startup tests.

use maestro_app::{
    AuthStorage, AuthStorageData, ConfigValueOperations, ModelCost, ModelInput, ModelRegistry,
    ProviderConfigInput, ProviderModelInput, Settings, SettingsManager,
};
use std::cmp::Ordering;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Parse the authored arguments of one startup case.
pub fn args(tokens: &[&str]) -> maestro_cli::input::args::Args {
    maestro_cli::input::args::parse_args(
        &tokens
            .iter()
            .map(|token| (*token).to_owned())
            .collect::<Vec<_>>(),
    )
}

/// A current-thread runtime for one asynchronous startup case.
pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread runtime")
}

/// Configuration commands these catalogs never run.
pub struct NoCommands;

impl ConfigValueOperations for NoCommands {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        None
    }
}

/// A settings manager seeded with the supplied global members.
pub fn settings(entries: &[(&str, serde_json::Value)]) -> SettingsManager {
    let mut members = serde_json::Map::new();
    for (name, value) in entries {
        members.insert((*name).to_owned(), value.clone());
    }
    SettingsManager::in_memory(Settings(members))
}

/// The plain identifier comparison these cases resolve through.
pub fn comparator() -> impl Fn(&str, &str) -> Ordering {
    |left: &str, right: &str| left.cmp(right)
}

/// One model definition of a controlled provider.
fn definition(id: &str) -> ProviderModelInput {
    ProviderModelInput {
        id: id.to_owned(),
        name: id.to_owned(),
        api: None,
        base_url: None,
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 200_000.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

/// An offline catalog holding the supplied provider and identifier pairs.
pub fn registry(providers: &[(&str, &[&str])]) -> ModelRegistry {
    let catalog = ModelRegistry::in_memory(
        Arc::new(AuthStorage::in_memory(AuthStorageData::new())),
        Arc::new(NoCommands),
    )
    .expect("offline catalog");
    for (provider, ids) in providers {
        let definitions = ids.iter().map(|id| definition(id)).collect();
        catalog
            .register_provider(
                provider,
                Arc::new(RwLock::new(configuration(
                    Some(provider),
                    "controlled key",
                    definitions,
                ))),
            )
            .expect("registered provider");
    }
    catalog
}

/// Build one provider registration over the supplied definitions.
pub fn configuration(
    name: Option<&str>,
    api_key: &str,
    definitions: Vec<ProviderModelInput>,
) -> ProviderConfigInput {
    ProviderConfigInput {
        name: name.map(str::to_owned),
        base_url: Some("https://example.test".to_owned()),
        api_key: Some(api_key.to_owned()),
        api: Some("openai-completions".to_owned()),
        headers: None,
        auth_header: None,
        models: Some(definitions),
        callbacks: maestro_app::ProviderCallbacks::default(),
    }
}

/// Write a one-header history with the given identifier and working directory.
pub fn stored_history(directory: &Path, name: &str, id: &str, cwd: &str) -> String {
    std::fs::create_dir_all(directory).unwrap();
    let path = directory.join(format!("{name}.jsonl"));
    let header = serde_json::json!({
        "type": "session",
        "version": 3,
        "id": id,
        "cwd": cwd,
        "timestamp": "2025-01-01T00:00:00.000Z"
    });
    std::fs::write(&path, format!("{header}\n")).unwrap();
    path.to_str().unwrap().to_owned()
}

/// Set a stored history's modification time to a whole epoch second.
pub fn set_modified(path: &str, seconds: u64) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .expect("an open history")
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
        .expect("a set modification time");
}
