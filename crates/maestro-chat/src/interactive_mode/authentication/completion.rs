//! What a successful sign-in changes: the catalog, the default model and the presentation.

use super::InteractiveMode;
use crate::AuthType;
use maestro_app::{
    DEFAULT_MODEL_PER_PROVIDER, Model,
    configuration::config::{get_agent_dir, paths::get_auth_path},
};
use maestro_tui::Component;
use std::{
    error::Error,
    rc::Rc,
    sync::{Arc, PoisonError, RwLock},
};

/// What signing in did to the selected model.
enum AuthenticationSelection {
    /// The prior model was kept.
    Unchanged,
    /// The provider's default model was selected; the shared descriptor is read where its
    /// identity is shown.
    Selected(Arc<RwLock<Model>>),
    /// The default model could not be selected; the text completes the action label.
    Unavailable(String),
}

/// Whether `model` is the placeholder a session starts with when no model was resolved.
fn is_unknown_model(model: &Model) -> bool {
    model.provider == "unknown" && model.id == "unknown" && model.api == "unknown"
}

impl InteractiveMode {
    /// Refreshes the catalog after signing in to `provider_id`, selects its default model
    /// when `previous_model` is still the placeholder, updates the provider count, footer
    /// and border, and reports the outcome; the subscription billing notice is checked
    /// without waiting for it.
    ///
    /// # Errors
    /// Returns the catalog refresh or availability failure, the count's refresh failure,
    /// the agent directory's failure or the published theme's.
    pub(in crate::interactive_mode) async fn complete_provider_authentication(
        self: &Rc<Self>,
        provider_id: &str,
        provider_name: &str,
        auth_type: AuthType,
        previous_model: Arc<RwLock<Model>>,
    ) -> Result<(), Box<dyn Error>> {
        self.runtime_host.session().model_registry().refresh()?;
        let action = match auth_type {
            AuthType::OAuth => format!("Logged in to {provider_name}"),
            AuthType::ApiKey => format!("Saved API key for {provider_name}"),
        };
        let selection = self
            .authenticated_model_selection(provider_id, previous_model)
            .await?;
        self.update_available_provider_count()?;
        self.footer.invalidate();
        self.update_editor_border_color()?;
        let saved = get_auth_path(&get_agent_dir()?);
        match selection {
            AuthenticationSelection::Selected(model) => {
                let id = model
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .id
                    .clone();
                self.show_status(&format!(
                    "{action}. Selected {id}. Credentials saved to {saved}"
                ))?;
                self.check_subscription_warning(Some(model));
            }
            AuthenticationSelection::Unchanged => {
                self.show_status(&format!("{action}. Credentials saved to {saved}"))?;
                self.check_subscription_warning(None);
            }
            AuthenticationSelection::Unavailable(reason) => {
                self.show_status(&format!("{action}. Credentials saved to {saved}"))?;
                self.show_error(&format!("{action}{reason}"))?;
            }
        }
        Ok(())
    }

    /// Selects the configured default model of `provider_id` when `previous_model`, read
    /// now, is the placeholder; every reason it cannot is kept for the report.
    ///
    /// # Errors
    /// Returns the catalog's availability failure.
    async fn authenticated_model_selection(
        &self,
        provider_id: &str,
        previous_model: Arc<RwLock<Model>>,
    ) -> Result<AuthenticationSelection, Box<dyn Error>> {
        let unknown = is_unknown_model(
            &previous_model
                .read()
                .unwrap_or_else(PoisonError::into_inner),
        );
        if !unknown {
            return Ok(AuthenticationSelection::Unchanged);
        }
        let session = self.runtime_host.session();
        let available = session.model_registry().get_available()?;
        let default_id = DEFAULT_MODEL_PER_PROVIDER
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(provider_id)
            .cloned();
        let Some(default_id) = default_id else {
            return Ok(AuthenticationSelection::Unavailable(format!(
                ", but no default model is configured for provider \"{provider_id}\". Use /model to select a model."
            )));
        };
        let provider_models: Vec<_> = available
            .into_iter()
            .filter(|model| {
                model
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .provider
                    == provider_id
            })
            .collect();
        if provider_models.is_empty() {
            return Ok(AuthenticationSelection::Unavailable(
                ", but no models are available for that provider. Use /model to select a model."
                    .to_owned(),
            ));
        }
        let default = provider_models
            .into_iter()
            .find(|model| model.read().unwrap_or_else(PoisonError::into_inner).id == default_id);
        let Some(model) = default else {
            return Ok(AuthenticationSelection::Unavailable(format!(
                ", but its default model \"{default_id}\" is not available. Use /model to select a model."
            )));
        };
        Ok(match session.set_model(Arc::clone(&model)).await {
            Ok(()) => AuthenticationSelection::Selected(model),
            Err(error) => AuthenticationSelection::Unavailable(format!(
                ", but selecting its default model failed: {}. Use /model to select a model.",
                error.message
            )),
        })
    }

    /// Checks the subscription billing notice for `model`, or the session's model, on the
    /// terminal runtime, so a pending key lookup does not hold up the caller.
    pub(in crate::interactive_mode) fn check_subscription_warning(
        self: &Rc<Self>,
        model: Option<Arc<RwLock<Model>>>,
    ) {
        let mode = Rc::clone(self);
        self.tool_environment
            .runtime
            .spawn_local(Box::pin(async move {
                mode.maybe_warn_about_anthropic_subscription_auth(model)
                    .await
            }));
    }
}
