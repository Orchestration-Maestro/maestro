//! The sign-in dialogs: cloud setup guidance, API-key entry and a subscription's choice.

use super::InteractiveMode;
use crate::{
    AuthType, ExtensionSelectorComponent, ExtensionSelectorOptions, LoginDialogComponent,
    LoginDialogOptions, LoginDialogPlatform,
};
use maestro_app::{
    ApiKeyCredential, AuthCredential, OAuthSelectPrompt, configuration::config::get_docs_path,
    is_whitespace_scalar,
};
use maestro_theme::ThemeColor;
use std::{cell::Cell, error::Error, path::Path, rc::Rc};
use tokio::sync::oneshot;

/// What the dialog's owner learns when the dialog itself closes.
type Completion = Rc<dyn Fn(bool, Option<&str>)>;

impl InteractiveMode {
    /// A sign-in dialog for `provider_name`, titled `title` when given, that asks for frames
    /// without keeping the terminal alive and reports its own closing to `on_complete`.
    ///
    /// # Errors
    /// Returns the published theme's failure.
    fn authentication_dialog(
        &self,
        provider_id: &str,
        provider_name: &str,
        title: Option<String>,
        on_complete: Completion,
    ) -> Result<Rc<LoginDialogComponent>, Box<dyn Error>> {
        let ui = Rc::downgrade(&self.ui);
        Ok(Rc::new(LoginDialogComponent::new(
            Rc::new(move || {
                if let Some(ui) = ui.upgrade() {
                    ui.request_render(false);
                }
            }),
            provider_id,
            on_complete,
            LoginDialogOptions {
                theme: self.themes.theme(),
                provider_name_override: Some(provider_name.to_owned()),
                title_override: title,
                platform: LoginDialogPlatform::default(),
                provider_name_lookup: None,
                launch_browser: None,
            },
        )?))
    }

    /// A sign-in dialog whose own closing changes nothing; its operation finishes it.
    ///
    /// # Errors
    /// Returns the published theme's failure.
    pub(super) fn sign_in_dialog(
        &self,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<Rc<LoginDialogComponent>, Box<dyn Error>> {
        self.authentication_dialog(provider_id, provider_name, None, Rc::new(|_, _| {}))
    }

    /// Shows how the cloud provider reads its credentials; closing puts the editor back.
    ///
    /// # Errors
    /// Returns the published theme's failure or the documentation path's.
    pub(in crate::interactive_mode) fn show_bedrock_setup_dialog(
        self: &Rc<Self>,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mode = Rc::downgrade(self);
        let dialog = self.authentication_dialog(
            provider_id,
            provider_name,
            Some("Amazon Bedrock setup".to_owned()),
            Rc::new(move |_, _| {
                if let Some(mode) = mode.upgrade() {
                    mode.restore_editor();
                    mode.ui.request_render(false);
                }
            }),
        )?;
        let theme = self.themes.theme().get()?;
        let guide = Path::new(&get_docs_path()?).join("providers.md");
        dialog.show_info(&[
            theme.fg(
                &ThemeColor::Text,
                "Amazon Bedrock uses AWS credentials instead of a single API key.",
            )?,
            theme.fg(
                &ThemeColor::Text,
                "Configure an AWS profile, IAM keys, bearer token, or role-based credentials.",
            )?,
            theme.fg(&ThemeColor::Muted, "See:")?,
            theme.fg(&ThemeColor::Accent, &format!("  {}", guide.display()))?,
        ])?;
        self.show_panel(dialog.clone(), dialog);
        Ok(())
    }

    /// Asks for an API key and saves it for `provider_id`, then puts the editor back and
    /// completes the sign-in; a failure other than cancelling is shown.
    ///
    /// # Errors
    /// Returns the published theme's failure while showing the dialog or a failure.
    pub(in crate::interactive_mode) async fn show_api_key_login_dialog(
        self: &Rc<Self>,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<(), Box<dyn Error>> {
        let previous_model = self.runtime_host.session().model();
        let dialog = self.sign_in_dialog(provider_id, provider_name)?;
        self.show_panel(dialog.clone(), dialog.clone());
        let saved = self.save_api_key(&dialog, provider_id).await;
        self.restore_editor();
        self.ui.request_render(false);
        let completed = match saved {
            Ok(()) => {
                self.complete_provider_authentication(
                    provider_id,
                    provider_name,
                    AuthType::ApiKey,
                    previous_model,
                )
                .await
            }
            Err(error) => Err(error),
        };
        match completed {
            Ok(()) => Ok(()),
            Err(error) => self
                .show_login_failure(
                    &format!("Failed to save API key for {provider_name}"),
                    &*error,
                )
                .map_err(Into::into),
        }
    }

    /// Save the trimmed key and required account values only after every prompt succeeds.
    ///
    /// # Errors
    /// Returns a prompt failure, cancellation, or an empty required value.
    async fn save_api_key(
        &self,
        dialog: &LoginDialogComponent,
        provider_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        let key = prompt_value(dialog, "Enter API key:", "API key").await?;
        let mut env = indexmap::IndexMap::new();
        if matches!(
            provider_id,
            "cloudflare-workers-ai" | "cloudflare-ai-gateway"
        ) {
            let account = prompt_value(
                dialog,
                "Enter Cloudflare account ID:",
                "Cloudflare account ID",
            )
            .await?;
            env.insert("CLOUDFLARE_ACCOUNT_ID".into(), account);
            if provider_id == "cloudflare-ai-gateway" {
                let gateway = prompt_value(
                    dialog,
                    "Enter Cloudflare AI Gateway ID:",
                    "Cloudflare AI Gateway ID",
                )
                .await?;
                env.insert("CLOUDFLARE_GATEWAY_ID".into(), gateway);
            }
        }
        let session = self.runtime_host.session();
        session.model_registry().auth_storage().set(
            provider_id,
            AuthCredential::ApiKey(ApiKeyCredential {
                env: (!env.is_empty()).then_some(env),
                key,
            }),
        );
        Ok(())
    }

    /// Shows a subscription's choice in place of `dialog`; choosing or cancelling puts
    /// `dialog` back first. The returned answer is the first option whose label was
    /// chosen, or none when cancelled.
    ///
    /// # Errors
    /// Returns the published theme's or the key hint's failure.
    pub(in crate::interactive_mode) fn show_oauth_login_select(
        self: &Rc<Self>,
        dialog: Rc<LoginDialogComponent>,
        prompt: OAuthSelectPrompt,
    ) -> Result<impl Future<Output = Option<String>> + use<>, Box<dyn Error>> {
        let (answer, answered) = oneshot::channel::<Option<String>>();
        let answer = Rc::new(Cell::new(Some(answer)));
        let mode = Rc::downgrade(self);
        let settle = Rc::new(move |id: Option<String>| {
            if let Some(mode) = mode.upgrade() {
                mode.show_panel(dialog.clone(), dialog.clone());
            }
            if let Some(answer) = answer.take() {
                // The sign-in that stopped waiting no longer wants the choice.
                drop(answer.send(id));
            }
        });
        let OAuthSelectPrompt { message, options } = prompt;
        let labels = options.iter().map(|option| option.label.clone()).collect();
        let chosen = Rc::clone(&settle);
        let selector = Rc::new(ExtensionSelectorComponent::new(
            message,
            labels,
            Rc::new(move |label| {
                let id = options.iter().find(|option| option.label == label);
                chosen(id.map(|option| option.id.clone()));
            }),
            Rc::new(move || settle(None)),
            ExtensionSelectorOptions {
                tui: None,
                timeout: None,
                runtime: Rc::clone(&self.tool_environment.runtime),
                theme: self.themes.theme(),
            },
        )?);
        self.show_panel(selector.clone(), selector);
        Ok(async move { answered.await.ok().flatten() })
    }
}

/// Read a required prompt value with the same whitespace boundaries as stored keys.
async fn prompt_value(
    dialog: &LoginDialogComponent,
    prompt: &str,
    name: &str,
) -> Result<String, Box<dyn Error>> {
    let entered = dialog.show_prompt(prompt, None)?.await?;
    let value = entered.trim_matches(is_whitespace_scalar);
    if value.is_empty() {
        return Err(format!("{name} cannot be empty.").into());
    }
    Ok(value.to_owned())
}
