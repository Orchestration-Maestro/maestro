//! Model selection: the `/model` and `/scoped-models` commands, the model and scope
//! panels, model candidates, the footer's provider count and the subscription billing
//! notice.
use super::InteractiveMode;
use crate::components::scoped_models_selector::{
    ModelsCallbacks, ModelsConfig, ScopedModelsSelectorComponent,
};
use crate::{ModelSelectorCallbacks, ModelSelectorComponent, ModelSelectorOptions};
use maestro_app::{
    AuthCredential, Model, ModelResolverOutput, ScopedModel, SettingsListEntry,
    find_exact_model_reference_match, model_id_comparator, resolve_model_scope,
};
use maestro_tui::Component;
use std::{
    collections::HashSet,
    error::Error,
    io,
    rc::Rc,
    sync::{Arc, PoisonError, RwLock},
};

/// The notice shown once when a provider subscription pays for usage.
const ANTHROPIC_SUBSCRIPTION_AUTH_WARNING: &str = "Anthropic subscription auth is active. Third-party harness usage draws from extra usage and is billed per token, not your Claude plan limits. Manage extra usage at https://claude.ai/settings/usage.";

/// Whether `api_key` is a subscription token.
fn is_anthropic_subscription_auth_key(api_key: Option<&str>) -> bool {
    api_key.is_some_and(|key| key.starts_with("sk-ant-oat"))
}

impl InteractiveMode {
    /// Applies the unique exact match of `search_term` among the candidates, or opens the
    /// model panel with that search; no or an empty term opens the panel unfiltered.
    ///
    /// # Errors
    /// Returns the catalog refresh failure, the panel's failure or the published theme's.
    pub(super) async fn handle_model_command(
        self: &Rc<Self>,
        search_term: Option<String>,
    ) -> Result<(), Box<dyn Error>> {
        let Some(search_term) = search_term.filter(|term| !term.is_empty()) else {
            return self.show_model_selector(None);
        };
        match self.find_exact_model_match(&search_term)? {
            Some(model) => self.apply_model_selection(model, None).await,
            None => self.show_model_selector(Some(search_term)),
        }
    }

    /// The unique exact reference match of `search_term` among the candidates.
    ///
    /// # Errors
    /// Returns the catalog refresh failure.
    fn find_exact_model_match(
        &self,
        search_term: &str,
    ) -> Result<Option<Arc<RwLock<Model>>>, Box<dyn Error>> {
        Ok(find_exact_model_reference_match(
            search_term,
            &self.get_model_candidates()?,
        ))
    }

    /// The session's scoped models in order, or else the refreshed catalog's available
    /// models, empty when availability cannot be read.
    ///
    /// # Errors
    /// Returns the catalog refresh failure.
    pub(super) fn get_model_candidates(&self) -> Result<Vec<Arc<RwLock<Model>>>, Box<dyn Error>> {
        let session = self.runtime_host.session();
        let scoped = session.scoped_models();
        if !scoped.is_empty() {
            return Ok(scoped.into_iter().map(|scoped| scoped.model).collect());
        }
        let registry = session.model_registry();
        registry.refresh()?;
        Ok(registry.get_available().unwrap_or_default())
    }

    /// Gives the footer the number of distinct providers among the candidates.
    ///
    /// # Errors
    /// Returns the catalog refresh failure.
    pub(super) fn update_available_provider_count(&self) -> Result<(), Box<dyn Error>> {
        let mut providers = HashSet::new();
        let count = self
            .get_model_candidates()?
            .iter()
            .fold(0.0, |count, model| {
                let model = model.read().unwrap_or_else(PoisonError::into_inner);
                if providers.insert(model.provider.clone()) {
                    count + 1.0
                } else {
                    count
                }
            });
        self.footer_data_provider
            .set_available_provider_count(count);
        Ok(())
    }

    /// Shows the subscription billing notice once for this mode when `model`, or else the
    /// session's model, belongs to the subscription provider and a stored sign-in or a
    /// subscription key pays for it. A disabled notice, another provider, an ordinary or
    /// missing key and a failed key lookup show nothing and leave the check open.
    ///
    /// # Errors
    /// Returns the published theme's failure.
    pub(super) async fn maybe_warn_about_anthropic_subscription_auth(
        &self,
        model: Option<Arc<RwLock<Model>>>,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        let warnings = session.settings_manager().borrow().get_warnings();
        if warnings.anthropic_extra_usage() == Some(false)
            || self.anthropic_subscription_warning_shown.get()
        {
            return Ok(());
        }
        let model = model.unwrap_or_else(|| session.model());
        let provider = model
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .provider
            .clone();
        if provider != "anthropic" {
            return Ok(());
        }
        let registry = session.model_registry();
        let signed_in = matches!(
            registry.auth_storage().get("anthropic"),
            Some(AuthCredential::OAuth(_))
        );
        if !signed_in {
            let Ok(key) = registry.get_api_key_for_provider(&provider).await else {
                return Ok(());
            };
            if !is_anthropic_subscription_auth_key(key.as_deref()) {
                return Ok(());
            }
        }
        // A concurrent check may have shown the notice while the key was looked up.
        if self.anthropic_subscription_warning_shown.replace(true) {
            return Ok(());
        }
        Ok(self.show_warning(ANTHROPIC_SUBSCRIPTION_AUTH_WARNING)?)
    }

    /// Selects `model` on the current session. On success it refreshes the footer and
    /// border, runs `done`, shows the model and checks the subscription notice; on refusal
    /// it runs `done` and shows the session's error. The status and the notice read the
    /// selected descriptor after the session's listeners ran.
    ///
    /// # Errors
    /// Returns the published theme's failure.
    async fn apply_model_selection(
        self: &Rc<Self>,
        model: Arc<RwLock<Model>>,
        done: Option<Rc<dyn Fn()>>,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        let refused = session.set_model(Arc::clone(&model)).await.err();
        if refused.is_none() {
            self.footer.invalidate();
            self.update_editor_border_color()?;
        }
        if let Some(done) = done {
            done();
        }
        if let Some(error) = refused {
            return Ok(self.show_error(&error.message)?);
        }
        let id = model
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .id
            .clone();
        self.show_status(&format!("Model: {id}"))?;
        self.maybe_warn_about_anthropic_subscription_auth(Some(model))
            .await
    }

    /// The model panel's callbacks, which do not keep the mode alive: a selection is
    /// applied on the terminal runtime, owning the mode until it completes; cancelling
    /// puts the editor back and requests a frame.
    fn model_selector_callbacks(self: &Rc<Self>, done: Rc<dyn Fn()>) -> ModelSelectorCallbacks {
        let mode = Rc::downgrade(self);
        let selected = Rc::clone(&done);
        let cancelled = mode.clone();
        ModelSelectorCallbacks {
            on_select: Rc::new(move |model| {
                let Some(mode) = mode.upgrade() else {
                    return;
                };
                let done = Rc::clone(&selected);
                let runtime = Rc::clone(&mode.tool_environment.runtime);
                runtime.spawn_local(Box::pin(async move {
                    mode.apply_model_selection(model, Some(done)).await
                }));
            }),
            on_cancel: Rc::new(move || {
                done();
                if let Some(mode) = cancelled.upgrade() {
                    mode.ui.request_render(false);
                }
            }),
        }
    }

    /// Shows the model panel over the current session's settings, catalog, model and
    /// scope, with `initial_search_input` as its search.
    ///
    /// # Errors
    /// Returns the panel's catalog refresh, locale or key-hint failure.
    pub(super) fn show_model_selector(
        self: &Rc<Self>,
        initial_search_input: Option<String>,
    ) -> Result<(), Box<dyn Error>> {
        self.show_selector(|done| {
            let session = self.runtime_host.session();
            let selector = Rc::new(ModelSelectorComponent::new(
                &self.ui,
                Rc::clone(session.settings_manager()),
                session.model_registry(),
                self.model_selector_callbacks(done),
                ModelSelectorOptions {
                    current_model: Some(session.model()),
                    scoped_models: session.scoped_models(),
                    initial_search_input,
                    theme: self.themes.theme(),
                },
            )?);
            Ok((selector.clone(), selector))
        })
    }

    /// Shows the scope panel over the refreshed catalog's available models, starting from
    /// the session's scope or else the saved patterns, or says no model is available.
    ///
    /// # Errors
    /// Returns the catalog refresh or availability failure, a saved pattern that is not
    /// text, or the scope resolution failure.
    pub(super) fn show_models_selector(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        session.model_registry().refresh()?;
        let all_models = session.model_registry().get_available()?;
        if all_models.is_empty() {
            return self.show_status("No models available").map_err(Into::into);
        }
        let enabled_model_ids = self.initial_enabled_model_ids(session.scoped_models())?;
        let available_count = all_models.len();
        self.show_selector(|done| {
            let selector = Rc::new(ScopedModelsSelectorComponent::new(
                ModelsConfig {
                    all_models,
                    enabled_model_ids,
                },
                self.scope_callbacks(available_count, done),
                self.themes.theme(),
                Rc::clone(&self.tool_environment.runtime),
            ));
            Ok((selector.clone(), selector))
        })
    }

    /// The panel's starting models by `provider/id`: the session's scope in order, or else
    /// the saved patterns resolved against the catalog; none when neither names any.
    ///
    /// # Errors
    /// Returns a saved pattern that is not text, before any resolution, or the scope
    /// resolution failure.
    fn initial_enabled_model_ids(
        &self,
        session_scope: Vec<ScopedModel>,
    ) -> Result<Option<Vec<String>>, Box<dyn Error>> {
        let scope = if session_scope.is_empty() {
            let session = self.runtime_host.session();
            let saved = session.settings_manager().borrow().get_enabled_models();
            let patterns = saved
                .unwrap_or_default()
                .into_iter()
                .map(|entry| match entry {
                    SettingsListEntry::String(pattern) => Ok(pattern),
                    SettingsListEntry::Unknown(_) => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "model patterns must be strings",
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if patterns.is_empty() {
                return Ok(None);
            }
            self.resolve_scope(&patterns)?
        } else {
            session_scope
        };
        Ok(Some(
            scope.iter().map(|scoped| full_id(&scoped.model)).collect(),
        ))
    }

    /// Resolves `patterns` against the current catalog in order, writing each pattern
    /// warning to standard error, in yellow when the terminal takes colour.
    ///
    /// # Errors
    /// Returns the locale, availability or pattern failure.
    fn resolve_scope(&self, patterns: &[String]) -> Result<Vec<ScopedModel>, Box<dyn Error>> {
        let compare = model_id_comparator(&|key| std::env::var(key).ok())?;
        let session = self.runtime_host.session();
        let mut stderr = io::stderr();
        Ok(resolve_model_scope(
            patterns,
            session.model_registry(),
            &compare,
            &mut ModelResolverOutput {
                stdout: &mut io::sink(),
                color: maestro_theme::supports_color(),
                stderr: &mut stderr,
            },
        )?)
    }

    /// Applies a live scope edit to the session without saving it: a nonempty selection of
    /// fewer than `available_count` models becomes the scope, anything else clears it.
    /// Then the footer's provider count is refreshed and a frame requested.
    ///
    /// # Errors
    /// Returns the scope resolution or catalog refresh failure.
    fn update_session_models(
        &self,
        enabled_ids: Option<Vec<String>>,
        available_count: usize,
    ) -> Result<(), Box<dyn Error>> {
        let scope = match enabled_ids {
            Some(ids) if !ids.is_empty() && ids.len() < available_count => {
                self.resolve_scope(&ids)?
            }
            _ => Vec::new(),
        };
        self.runtime_host.session().set_scoped_models(scope);
        self.update_available_provider_count()?;
        self.ui.request_render(false);
        Ok(())
    }

    /// The scope panel's callbacks, which do not keep the mode alive: edits apply to the
    /// current session on the terminal runtime; saving writes the selection, or removes the
    /// setting when all `available_count` models are enabled; cancelling puts the editor
    /// back and requests a frame.
    pub(super) fn scope_callbacks(
        self: &Rc<Self>,
        available_count: usize,
        done: Rc<dyn Fn()>,
    ) -> ModelsCallbacks {
        let changed = Rc::downgrade(self);
        let saved = changed.clone();
        let cancelled = changed.clone();
        ModelsCallbacks {
            on_change: Rc::new(move |ids| {
                let mode = changed.upgrade()?;
                Some(Box::pin(async move {
                    mode.update_session_models(ids, available_count)
                }))
            }),
            on_persist: Rc::new(move |ids| {
                let mode = saved.upgrade()?;
                let patterns = ids
                    .filter(|ids| ids.len() != available_count)
                    .map(|ids| ids.into_iter().map(SettingsListEntry::String).collect());
                let session = mode.runtime_host.session();
                session
                    .settings_manager()
                    .borrow_mut()
                    .set_enabled_models(patterns);
                let shown = mode
                    .show_status("Model selection saved to settings")
                    .map_err(Into::into);
                Some(Box::pin(std::future::ready(shown)))
            }),
            on_cancel: Rc::new(move || {
                done();
                if let Some(mode) = cancelled.upgrade() {
                    mode.ui.request_render(false);
                }
            }),
        }
    }
}

/// A descriptor's `provider/id`.
fn full_id(model: &Arc<RwLock<Model>>) -> String {
    let model = model.read().unwrap_or_else(PoisonError::into_inner);
    format!("{}/{}", model.provider, model.id)
}
