//! A scripted-model workspace for tests that create sessions through the public SDK.
use maestro_app::{
    CreateAgentSessionOptions, ResourceLoaderOptions, SessionManager, SessionResources,
    SettingsManager,
};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_models::{
    FauxProviderRegistration, Local, RegisterFauxProviderOptions, register_faux_provider,
};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{cell::RefCell, future::Future, rc::Rc, sync::Arc, sync::RwLock};
use tokio::task::LocalSet;

/// Errors returned by workspace setup or public SDK operations.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// The disposable file tree, scripted provider and local executor of one test.
pub struct Sample {
    /// Private project and configuration files, removed when the test ends.
    pub root: Scratch,
    /// The SDK's opt-in scripted protocol provider.
    pub provider: FauxProviderRegistration,
    /// The local executor that owns session background work.
    pub local: Rc<LocalSet>,
}

impl Sample {
    /// Write a model catalog for the scripted provider into a fresh directory.
    fn new(local: Rc<LocalSet>) -> Result<Self, Error> {
        let root = Scratch::new()?;
        let provider = register_faux_provider(RegisterFauxProviderOptions::default());
        let model = &provider.models[0];
        let catalog = json!({"providers": {"faux": {
            "baseUrl": model.base_url, "api": provider.api, "apiKey": "sample-key",
            "models": [{
                "id": model.id, "name": "Offline sample", "reasoning": false,
                "input": ["text"], "contextWindow": 128_000, "maxTokens": 4096,
                "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
            }]
        }}});
        std::fs::write(
            root.path().join("models.json"),
            serde_json::to_vec(&catalog)?,
        )?;
        Ok(Self {
            root,
            provider,
            local,
        })
    }

    /// Creation inputs whose resource discovery stays inside the disposable tree.
    pub fn options(&self) -> Result<CreateAgentSessionOptions, Error> {
        let dir = self.root.path().display().to_string();
        let credentials = Arc::new(AuthStorage::in_memory(AuthStorageData::default()));
        credentials.set_runtime_api_key("faux", "sample-key");
        let no_components: maestro_extensions::ModuleLoader<Local> = Rc::new(|path| {
            let message = format!("This workspace has no component loader: {path}");
            Box::pin(async move { Err(message.into()) })
        });
        let history = SessionManager::in_memory(Some(&dir))?;
        let settings = serde_json::from_value(json!({"enableInstallTelemetry": false}))?;
        Ok(CreateAgentSessionOptions {
            cwd: Some(dir.clone()),
            agent_dir: Some(dir),
            auth_storage: Some(credentials),
            model_registry: None,
            model: Some(Arc::new(RwLock::new(self.provider.models[0].clone()))),
            thinking_level: None,
            scoped_models: Vec::new(),
            no_tools: None,
            tools: None,
            custom_tools: Vec::new(),
            resources: SessionResources::Default(Box::new(ResourceLoaderOptions::new(
                Rc::clone(&self.local),
                no_components,
            ))),
            session_manager: Some(Rc::new(RefCell::new(history))),
            settings_manager: Some(Rc::new(RefCell::new(SettingsManager::in_memory(settings)))),
            session_start_event: None,
            theme: maestro_theme::LiveTheme::default(),
            offline: Some(true),
        })
    }
}

/// Run `body` on a local executor with a fresh [`Sample`] and return its observations.
pub fn execute<F, Fut>(body: F) -> Result<Value, Error>
where
    F: FnOnce(Sample) -> Fut,
    Fut: Future<Output = Result<Value, Error>>,
{
    super::session_harness::on_task_set(|local| async move { body(Sample::new(local)?).await })
}
