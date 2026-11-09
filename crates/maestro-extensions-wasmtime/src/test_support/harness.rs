//! Built components loaded into the production engine over in-memory session owners and the
//! scripted provider.

use std::{cell::RefCell, io, rc::Rc, sync::RwLock};

use maestro_extensions::{
    AuthStorage, AuthStorageData, ConfigValueOperations, EventBusController, EventError,
    ExtensionContextActions, ExtensionExecution, ExtensionLoadError, ExtensionRunner,
    ExtensionRuntime, ExtensionWidgets, LiveTheme, Local, ModelRegistry, ModuleLoader,
    ProviderConfig, ProviderModelConfig, SessionManager, SharedHandle, create_event_bus_for,
    create_extension_runtime, load_extensions_with_runtime,
    runner::ExtensionRunnerSession,
    test_support::{
        FauxModelDefinition, FauxProviderRegistration, RegisterFauxProviderOptions, ToolkitWidgets,
        register_faux_provider,
    },
    types::ProviderActions,
};

/// The API key the scripted provider is configured with.
const FAUX_KEY: &str = "faux-key";

/// What a [`Harness`] is built over.
#[derive(Default)]
pub struct HarnessOptions {
    /// Scripted models; the scripted provider's default model when absent.
    pub models: Option<Vec<FauxModelDefinition>>,
    /// The agent directory whose component cache the engine uses; empty compiles every
    /// component without a cache.
    pub agent_dir: String,
    /// Host widgets for the components; the toolkit widgets over an unpublished live theme
    /// when absent.
    pub widgets: Option<Rc<dyn ExtensionWidgets>>,
}

/// The production engine with in-memory history, credentials and catalog, and the scripted
/// provider configured in that catalog.
///
/// Components load through the production artifact factory into one shared runtime, as an
/// application loads them. Every application action stays unbound until a test binds it, so
/// an unconfigured capability fails with the runtime's own error. Run the harness inside
/// [`local`](super::local): components keep their native work on a local task set.
pub struct Harness {
    /// Working directory the session reports.
    pub cwd: String,
    /// The session's writable history.
    pub session_manager: Rc<RefCell<SessionManager>>,
    /// The session's catalog, with the scripted provider configured.
    pub model_registry: SharedHandle<ModelRegistry>,
    /// The scripted provider and its registered models.
    pub faux: FauxProviderRegistration,
    /// The bus every loaded component shares.
    pub bus: EventBusController<Local>,
    /// The registration runtime every loaded component shares.
    pub runtime: Rc<ExtensionRuntime<Local>>,
    /// Builds each component through the production artifact factory.
    loader: ModuleLoader<Local>,
}

/// The components of one [`Harness::load`] and the runner over them.
pub struct Loaded {
    /// The runner over the components that loaded, bound with unbound application actions.
    pub runner: Rc<ExtensionRunner<Local>>,
    /// The components that failed to load, in supplied order.
    pub errors: Vec<ExtensionLoadError>,
}

impl Harness {
    /// Builds a harness whose session reports `cwd`.
    ///
    /// # Errors
    /// Returns a failure to build the in-memory history, the catalog or the toolkit widgets,
    /// or the catalog's refusal of the scripted provider.
    pub fn new(cwd: &str, options: HarnessOptions) -> Result<Self, EventError> {
        let faux = register_faux_provider(RegisterFauxProviderOptions {
            models: options.models,
            ..RegisterFauxProviderOptions::default()
        });
        let model_registry = SharedHandle::new(catalog(&faux)?);
        let widgets = match options.widgets {
            Some(widgets) => widgets,
            None => Rc::new(ToolkitWidgets::new(LiveTheme::default())?),
        };
        Ok(Self {
            cwd: cwd.to_owned(),
            session_manager: Rc::new(RefCell::new(SessionManager::in_memory(Some(cwd))?)),
            model_registry,
            faux,
            bus: create_event_bus_for::<Local>(
                Rc::new(|tail| drop(tokio::task::spawn_local(tail))),
                Rc::new(|text: &str| eprint!("{text}")),
            ),
            runtime: create_extension_runtime::<Local>(),
            loader: module_loader(widgets, options.agent_dir),
        })
    }

    /// Loads each component path into the shared runtime and binds a runner over the ones that
    /// loaded.
    ///
    /// A component whose factory fails, including through a refused registration, is reported
    /// in [`Loaded::errors`] and the next path still loads.
    ///
    /// # Errors
    /// Returns a failure to read a supplied path or to bind the runner.
    pub async fn load(&self, components: Vec<String>) -> io::Result<Loaded> {
        let loaded = load_extensions_with_runtime(
            components,
            &self.cwd,
            &self.bus,
            &self.loader,
            self.runtime.clone(),
        )
        .await?;
        let runner = Rc::new(ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(loaded.extensions)),
            loaded.runtime,
            ExtensionRunnerSession {
                cwd: self.cwd.clone(),
                session_manager: self.session_manager.clone(),
                model_registry: self.model_registry.clone(),
            },
            Rc::new(|text: &str| eprint!("{text}")),
            LiveTheme::default(),
        ));
        runner
            .bind_core(
                Local::unbound_actions(),
                ExtensionContextActions::default(),
                ProviderActions::default(),
            )
            .map_err(io::Error::other)?;
        Ok(Loaded {
            runner,
            errors: loaded.errors,
        })
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.faux.unregister();
    }
}

/// Loads each component path through the production artifact factory.
fn module_loader(widgets: Rc<dyn ExtensionWidgets>, agent_dir: String) -> ModuleLoader<Local> {
    Rc::new(move |path: &str| {
        let factory = crate::artifact_factory(
            path.to_owned(),
            widgets.clone(),
            crate::Installation {
                version: env!("CARGO_PKG_VERSION").into(),
                agent_dir: agent_dir.clone(),
            },
        );
        Box::pin(std::future::ready(Ok(Some(factory))))
    })
}

/// An in-memory catalog with the scripted provider's models and key registered.
fn catalog(faux: &FauxProviderRegistration) -> Result<ModelRegistry, EventError> {
    let storage = SharedHandle::new(AuthStorage::in_memory(AuthStorageData::default()));
    let Some(first) = faux.models.first() else {
        return Err("the scripted provider registered no model".into());
    };
    storage.set_runtime_api_key(&first.provider, FAUX_KEY);
    let registry = ModelRegistry::in_memory(storage, SharedHandle::new(NoCommands))?;
    let models = faux
        .models
        .iter()
        .map(|model| ProviderModelConfig {
            id: model.id.clone(),
            name: model.name.clone(),
            api: Some(model.api.clone()),
            base_url: Some(model.base_url.clone()),
            reasoning: model.reasoning,
            thinking_level_map: None,
            input: model.input.clone(),
            cost: model.cost.clone(),
            context_window: model.context_window,
            max_tokens: model.max_tokens,
            headers: None,
            compat: None,
        })
        .collect();
    let config = ProviderConfig {
        base_url: Some(first.base_url.clone()),
        api_key: Some(FAUX_KEY.into()),
        api: Some(faux.api.clone()),
        models: Some(models),
        ..ProviderConfig::default()
    };
    registry.register_provider(&first.provider, SharedHandle::new(RwLock::new(config)))?;
    Ok(registry)
}

/// Credential commands that never run: the harness configures literal keys only.
struct NoCommands;

impl ConfigValueOperations for NoCommands {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{FAUX_KEY, Harness, HarnessOptions};
    use maestro_extensions::{ResolvedRequestAuth, test_support::FauxModelDefinition};

    /// Every scripted model is available in the session catalog and resolves to the scripted
    /// provider's key, so an extension's model call reaches the scripted provider.
    #[test]
    fn scripted_models_are_available_with_the_scripted_key() {
        let definition = |id: &str| FauxModelDefinition {
            id: id.into(),
            ..FauxModelDefinition::default()
        };
        let options = HarnessOptions {
            models: Some(vec![definition("first"), definition("second")]),
            ..HarnessOptions::default()
        };
        let harness = Harness::new("/work", options).unwrap();
        let available = harness.model_registry.get_available().unwrap();
        let identities: Vec<_> = available
            .iter()
            .map(|model| {
                let model = model.read().unwrap();
                (model.provider.clone(), model.id.clone(), model.api.clone())
            })
            .collect();
        let api = &harness.faux.api;
        assert_eq!(
            identities,
            [
                ("faux".into(), "first".into(), api.clone()),
                ("faux".into(), "second".into(), api.clone()),
            ]
        );
        let auth = super::super::local(
            harness
                .model_registry
                .get_api_key_and_headers(&available[1]),
        )
        .unwrap();
        let ResolvedRequestAuth::Ok { api_key, .. } = auth else {
            panic!("the scripted model resolves its key");
        };
        assert_eq!(api_key.as_deref(), Some(FAUX_KEY));
    }
}
