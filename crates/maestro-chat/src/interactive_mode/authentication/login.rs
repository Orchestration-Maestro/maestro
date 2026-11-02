//! The `/login` navigation: the sign-in method, then the provider to configure.

use super::{InteractiveMode, selector_cancellation};
use crate::{
    AuthSelectorProvider, AuthType, ExtensionSelectorComponent, ExtensionSelectorOptions,
    OAuthSelectorComponent, OAuthSelectorMode, OAuthSelectorOptions,
};
use std::{
    error::Error,
    rc::{Rc, Weak},
    sync::Arc,
};

/// The method choice for subscription sign-in.
const SUBSCRIPTION_LABEL: &str = "Use a subscription";
/// The method choice for API-key sign-in.
const API_KEY_LABEL: &str = "Use an API key";
/// The provider whose credentials come from its own cloud configuration.
const BEDROCK_PROVIDER_ID: &str = "amazon-bedrock";

impl InteractiveMode {
    /// Shows the sign-in method choice; a choice opens its provider picker and cancelling
    /// puts the editor back.
    ///
    /// # Errors
    /// Returns the published theme's or the key hint's failure.
    pub(in crate::interactive_mode) fn show_login_auth_type_selector(
        self: &Rc<Self>,
    ) -> Result<(), Box<dyn Error>> {
        let mode = Rc::downgrade(self);
        self.show_selector(|done| {
            let selector = Rc::new(ExtensionSelectorComponent::new(
                "Select authentication method:".to_owned(),
                vec![SUBSCRIPTION_LABEL.to_owned(), API_KEY_LABEL.to_owned()],
                login_method_selection(mode.clone(), Rc::clone(&done)),
                selector_cancellation(mode, done),
                ExtensionSelectorOptions {
                    tui: None,
                    timeout: None,
                    runtime: Rc::clone(&self.tool_environment.runtime),
                    theme: self.themes.theme(),
                    on_toggle_tools_expanded: None,
                },
            )?);
            Ok((selector.clone(), selector))
        })
    }

    /// Shows the providers offering `auth_type`, or says there are none.
    ///
    /// # Errors
    /// Returns the option list's collator failure, the picker's row failure or the
    /// published theme's failure.
    pub(in crate::interactive_mode) fn show_login_provider_selector(
        self: &Rc<Self>,
        auth_type: AuthType,
    ) -> Result<(), Box<dyn Error>> {
        let options = Rc::new(self.get_login_provider_options(Some(auth_type))?);
        if options.is_empty() {
            let empty = match auth_type {
                AuthType::OAuth => "No subscription providers available.",
                AuthType::ApiKey => "No API key providers available.",
            };
            return self.show_status(empty).map_err(Into::into);
        }
        let mode = Rc::downgrade(self);
        self.show_selector(|done| {
            let session = self.runtime_host.session();
            let storage = Arc::clone(session.model_registry().auth_storage());
            let host = Rc::clone(&self.runtime_host);
            let selector = Rc::new(OAuthSelectorComponent::new(
                storage,
                options.as_ref().clone(),
                login_selection(mode.clone(), Rc::clone(&options), Rc::clone(&done)),
                login_cancellation(mode, done),
                OAuthSelectorOptions {
                    mode: OAuthSelectorMode::Login,
                    theme: self.themes.theme(),
                    get_auth_status: Some(Rc::new(move |id| {
                        let session = host.session();
                        Ok(session.model_registry().get_provider_auth_status(id)?)
                    })),
                    runtime: Rc::clone(&self.tool_environment.runtime),
                },
            )?);
            Ok((selector.clone(), selector))
        })
    }
}

/// The method choice's selection: the editor comes back, then the method's providers.
fn login_method_selection(mode: Weak<InteractiveMode>, done: Rc<dyn Fn()>) -> Rc<dyn Fn(&str)> {
    Rc::new(move |option| {
        done();
        let auth_type = if option == SUBSCRIPTION_LABEL {
            AuthType::OAuth
        } else {
            AuthType::ApiKey
        };
        if let Some(mode) = mode.upgrade() {
            mode.report(mode.show_login_provider_selector(auth_type));
        }
    })
}

/// The provider picker's selection: the editor comes back first, then the chosen
/// provider's sign-in starts, on the terminal runtime when it waits for input.
fn login_selection(
    mode: Weak<InteractiveMode>,
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
        if option.auth_type == AuthType::ApiKey && option.id == BEDROCK_PROVIDER_ID {
            return mode.report(mode.show_bedrock_setup_dialog(&option.id, &option.name));
        }
        let option = option.clone();
        let runtime = Rc::clone(&mode.tool_environment.runtime);
        runtime.spawn_local(Box::pin(async move {
            match option.auth_type {
                AuthType::OAuth => mode.show_login_dialog(&option.id, &option.name).await,
                AuthType::ApiKey => {
                    mode.show_api_key_login_dialog(&option.id, &option.name)
                        .await
                }
            }
        }));
    })
}

/// The provider picker's cancellation: the editor comes back, then the method choice.
fn login_cancellation(mode: Weak<InteractiveMode>, done: Rc<dyn Fn()>) -> Rc<dyn Fn()> {
    Rc::new(move || {
        done();
        if let Some(mode) = mode.upgrade() {
            mode.report(mode.show_login_auth_type_selector());
        }
    })
}
