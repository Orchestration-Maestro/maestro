//! Disposable directories and the SDK's scripted model for offline samples.
use maestro_app::{
    CreateAgentSessionOptions, ResourceLoaderOptions, SessionManager, SessionResources,
};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_models::{
    FauxProviderRegistration, RegisterFauxProviderOptions, register_faux_provider,
};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{cell::RefCell, future::Future, rc::Rc, sync::Arc};
use tokio::task::LocalSet;

/// Errors returned by sample setup or public SDK operations.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// The real file tree, model script and local executor retained for one sample.
pub struct Sample {
    /// Private project and configuration files, removed after the sample exits.
    pub root: Scratch,
    /// The SDK's opt-in scripted protocol provider.
    pub provider: FauxProviderRegistration,
    /// The local executor that owns session background work.
    pub local: Rc<LocalSet>,
}

impl Sample {
    /// Create isolated configuration without reading user credentials or calling a network.
    pub fn new(local: Rc<LocalSet>) -> Result<Self, Error> {
        let root = Scratch::new()?;
        let provider = register_faux_provider(RegisterFauxProviderOptions::default());
        std::fs::write(
            root.path().join("models.json"),
            serde_json::to_vec(&json!({
                "providers": {"faux": {"api": provider.api, "apiKey": "sample-key", "models": [{
                    "id": provider.models[0].id, "name": "Offline sample", "reasoning": false,
                    "input": ["text"], "contextWindow": 128_000, "maxTokens": 4096,
                    "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
                }]}}
            }))?,
        )?;
        Ok(Self {
            root,
            provider,
            local,
        })
    }

    /// Creation inputs with real resource discovery restricted to the disposable tree.
    pub fn options(&self) -> Result<CreateAgentSessionOptions, Error> {
        let cwd = self.root.path().display().to_string();
        let auth = Arc::new(AuthStorage::in_memory(AuthStorageData::default()));
        auth.set_runtime_api_key("faux", "sample-key");
        let loader: maestro_extensions::ModuleLoader<maestro_models::Local> = Rc::new(|path| {
            let error = format!("This sample has no component loader: {path}");
            Box::pin(async move { Err(error.into()) })
        });
        let resources = ResourceLoaderOptions::new(Rc::clone(&self.local), loader);
        Ok(CreateAgentSessionOptions {
            cwd: Some(cwd.clone()),
            agent_dir: Some(cwd.clone()),
            auth_storage: Some(auth),
            model_registry: None,
            model: Some(Arc::new(std::sync::RwLock::new(
                self.provider.models[0].clone(),
            ))),
            thinking_level: None,
            scoped_models: Vec::new(),
            no_tools: None,
            tools: None,
            custom_tools: Vec::new(),
            resources: SessionResources::Default(Box::new(resources)),
            session_manager: Some(Rc::new(RefCell::new(SessionManager::in_memory(Some(
                &cwd,
            ))?))),
            settings_manager: Some(Rc::new(RefCell::new(
                maestro_app::SettingsManager::in_memory(serde_json::from_value(
                    json!({"enableInstallTelemetry": false}),
                )?),
            ))),
            session_start_event: None,
            theme: maestro_theme::LiveTheme::default(),
            offline: Some(true),
        })
    }
}

/// Run a sample on its caller-owned local executor and print the observed result.
pub fn launch<F, Fut>(run: F) -> Result<(), Error>
where
    F: FnOnce(Sample) -> Fut,
    Fut: Future<Output = Result<Value, Error>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let local = Rc::new(LocalSet::new());
    let sample = Sample::new(Rc::clone(&local))?;
    let value = runtime.block_on(local.run_until(run(sample)))?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
