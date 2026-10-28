//! Provider sign-in and saved credentials: the `/login` and `/logout` commands, their
//! pickers and their effects.

use super::InteractiveMode;
use crate::{
    AuthSelectorProvider, AuthType, OAuthSelectorComponent, OAuthSelectorMode, OAuthSelectorOptions,
};
use maestro_app::AuthCredential;
use maestro_theme::Collator;
use std::{error::Error, rc::Rc, sync::Arc};

mod callbacks;
mod completion;
mod dialogs;
mod login;
mod providers;

pub use providers::is_api_key_login_provider;

/// The notice shown when no stored credential can be removed.
const NO_STORED_CREDENTIALS: &str = "No stored credentials to remove. /logout only removes credentials saved by /login; environment variables and models.json config are unchanged.";

impl InteractiveMode {
    /// Shows the stored credentials to remove, or says there are none.
    ///
    /// # Errors
    /// Returns the option list's collator failure, the picker's row failure or the
    /// published theme's colour failure.
    pub(super) fn show_logout_selector(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        let options = Rc::new(self.logout_provider_options()?);
        if options.is_empty() {
            return self.show_status(NO_STORED_CREDENTIALS).map_err(Into::into);
        }
        let mode = Rc::downgrade(self);
        self.show_selector(|done| {
            let session = self.runtime_host.session();
            let storage = Arc::clone(session.model_registry().auth_storage());
            let selector = Rc::new(OAuthSelectorComponent::new(
                storage,
                options.as_ref().clone(),
                logout_selection(mode.clone(), Rc::clone(&options), Rc::clone(&done)),
                selector_cancellation(mode, done),
                OAuthSelectorOptions {
                    mode: OAuthSelectorMode::Logout,
                    theme: self.themes.theme(),
                    get_auth_status: None,
                    runtime: Rc::clone(&self.tool_environment.runtime),
                },
            )?);
            Ok((selector.clone(), selector))
        })
    }

    /// The stored credentials as choices, ordered by display name under the current locale.
    ///
    /// # Errors
    /// Returns the failure to construct the collator for the selected locale.
    pub(super) fn logout_provider_options(
        &self,
    ) -> Result<Vec<AuthSelectorProvider>, Box<dyn Error>> {
        let session = self.runtime_host.session();
        let registry = session.model_registry();
        let storage = registry.auth_storage();
        let options: Vec<_> = storage
            .list()
            .into_iter()
            .filter_map(|id| {
                let kind = match storage.get(&id)? {
                    AuthCredential::OAuth(_) => AuthType::OAuth,
                    AuthCredential::ApiKey(_) => AuthType::ApiKey,
                };
                Some(AuthSelectorProvider {
                    name: registry.get_provider_display_name(&id),
                    auth_type: kind,
                    id,
                })
            })
            .collect();
        sorted_by_name(options)
    }

    /// Removes the chosen saved credential on the current session, then refreshes the
    /// catalog, counts its providers and shows the chosen success text; a failure keeps the
    /// effects already reached and reports it.
    fn remove_saved_credential(
        self: Rc<Self>,
        option: &AuthSelectorProvider,
    ) -> Result<(), Box<dyn Error>> {
        match self.apply_logout(option) {
            Ok(()) => Ok(()),
            Err(error) => Ok(self.show_error(&format!("Logout failed: {error}"))?),
        }
    }

    /// The removal and everything after it, without presenting a failure.
    ///
    /// # Errors
    /// Returns the catalog refresh failure or the failure to show the success text. A
    /// catalog that cannot say which providers are available is not a failure: the
    /// footer count becomes zero.
    fn apply_logout(&self, option: &AuthSelectorProvider) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        let registry = session.model_registry();
        registry.auth_storage().logout(&option.id);
        registry.refresh()?;
        self.update_available_provider_count()?;
        self.show_status(&logout_message(option))
            .map_err(Into::into)
    }
}

/// The picker's selection: the editor comes back first, then the chosen credential is
/// removed on the terminal runtime, which keeps the mode alive until the work settles.
fn logout_selection(
    mode: std::rc::Weak<InteractiveMode>,
    options: Rc<Vec<AuthSelectorProvider>>,
    done: Rc<dyn Fn()>,
) -> Rc<dyn Fn(&str)> {
    Rc::new(move |id| {
        done();
        let Some(mode) = mode.upgrade() else {
            return;
        };
        let Some(option) = options.iter().find(|option| option.id == id) else {
            return;
        };
        let option = option.clone();
        let runtime = Rc::clone(&mode.tool_environment.runtime);
        runtime.spawn_local(Box::pin(
            async move { mode.remove_saved_credential(&option) },
        ));
    })
}

/// `options` in a stable order of display names under the current locale.
///
/// # Errors
/// Returns the failure to construct the collator for the selected locale; reading the
/// locale environment cannot fail.
fn sorted_by_name(
    mut options: Vec<AuthSelectorProvider>,
) -> Result<Vec<AuthSelectorProvider>, Box<dyn Error>> {
    let collator = Collator::new(&|key| std::env::var(key).ok())?;
    options.sort_by(|left, right| collator.compare(&left.name, &right.name));
    Ok(options)
}

/// A picker's cancellation: the editor comes back and a frame is requested.
fn selector_cancellation(mode: std::rc::Weak<InteractiveMode>, done: Rc<dyn Fn()>) -> Rc<dyn Fn()> {
    Rc::new(move || {
        done();
        if let Some(mode) = mode.upgrade() {
            mode.ui.request_render(false);
        }
    })
}

/// The success text for removing `option`.
fn logout_message(option: &AuthSelectorProvider) -> String {
    match option.auth_type {
        AuthType::OAuth => format!("Logged out of {}", option.name),
        AuthType::ApiKey => format!(
            "Removed stored API key for {}. Environment variables and models.json config are unchanged.",
            option.name
        ),
    }
}
