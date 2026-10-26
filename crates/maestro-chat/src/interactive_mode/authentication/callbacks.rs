//! Subscription sign-in: the provider's interaction callbacks and the dialog that shows them.

use super::InteractiveMode;
use crate::local_dispatch::{DispatchToken, local_dispatch};
use crate::{AuthType, LoginDialogComponent, LoginDialogError, LoginInputFuture};
use maestro_app::presentation_data::{OAuthAuthInfo, OAuthLoginCallbacks, OAuthPrompt};
use maestro_app::{
    BoxFuture, Cancellation, DiagnosticErrorInfo, Model, OAuthCallbacks, OAuthError,
    OAuthSelectPrompt,
};
use maestro_theme::ThemeError;
use std::{
    error::Error,
    fmt::Display,
    rc::{Rc, Weak},
    sync::{Arc, RwLock},
};
use tokio::sync::{oneshot, watch};

/// Where a provider's question is answered.
type Reply<T> = oneshot::Sender<Result<T, OAuthError>>;
/// The pasted redirect, once entered, or its cancellation.
type ManualResult = Option<Result<String, DiagnosticErrorInfo>>;

/// The provider callback that the terminal shows.
enum LoginEvent {
    /// The authorization link and its instructions.
    Auth(OAuthAuthInfo),
    /// A question answered with typed text.
    Prompt(OAuthPrompt, Reply<String>),
    /// A progress line.
    Progress(String),
    /// A choice answered with the chosen option's identifier.
    Select(OAuthSelectPrompt, Reply<Option<String>>),
}

/// The sendable callbacks a subscription provider calls; each one is shown on the terminal
/// before it returns.
struct LoginCallbacks {
    /// Applies each callback on the terminal's thread.
    dispatch: DispatchToken<LoginEvent>,
    /// The dialog's cancellation.
    signal: Cancellation,
    /// The pasted redirect every request observes.
    manual: watch::Receiver<ManualResult>,
}

/// The cancellation every interrupted question reports.
fn login_cancelled() -> OAuthError {
    login_presentation_error(LoginDialogError::Cancelled).into()
}

/// The answer `reply` brings, or cancellation when its dialog went away.
fn answer<T: Send + 'static>(
    shown: Result<(), DiagnosticErrorInfo>,
    reply: oneshot::Receiver<Result<T, OAuthError>>,
) -> BoxFuture<Result<T, OAuthError>> {
    Box::pin(async move {
        shown?;
        reply.await.unwrap_or_else(|_| Err(login_cancelled()))
    })
}

impl OAuthLoginCallbacks for LoginCallbacks {
    fn callback_presence(&self) -> (bool, bool, bool) {
        (true, true, true)
    }
    fn on_auth(&self, info: OAuthAuthInfo) -> Result<(), OAuthError> {
        Ok(self.dispatch.call(LoginEvent::Auth(info))?)
    }

    fn on_prompt(&self, prompt: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        let (reply, replied) = oneshot::channel();
        answer(
            self.dispatch.call(LoginEvent::Prompt(prompt, reply)),
            replied,
        )
    }

    fn on_progress(&self, message: &str) -> Result<(), OAuthError> {
        Ok(self
            .dispatch
            .call(LoginEvent::Progress(message.to_owned()))?)
    }

    fn on_select(
        &self,
        prompt: OAuthSelectPrompt,
    ) -> Option<BoxFuture<Result<Option<String>, OAuthError>>> {
        let (reply, replied) = oneshot::channel();
        Some(answer(
            self.dispatch.call(LoginEvent::Select(prompt, reply)),
            replied,
        ))
    }

    fn on_manual_code_input(&self) -> Option<BoxFuture<Result<String, OAuthError>>> {
        let mut manual = self.manual.clone();
        Some(Box::pin(async move {
            let entered = manual.wait_for(Option::is_some).await;
            match entered.as_deref() {
                Ok(Some(entered)) => entered.clone().map_err(Into::into),
                _ => Err(login_cancelled()),
            }
        }))
    }

    fn signal(&self) -> Option<&Cancellation> {
        Some(&self.signal)
    }
}

/// A presentation failure as the diagnostic the provider receives.
fn login_presentation_error(error: impl Display) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error(error.to_string())
}

/// Shows a subscription's callbacks in its retained dialog on the terminal's thread.
struct LoginPresentation {
    /// The mode, which the presentation does not keep alive.
    mode: Weak<InteractiveMode>,
    /// The dialog every callback updates.
    dialog: Rc<LoginDialogComponent>,
    /// Whether the provider also accepts a pasted redirect.
    uses_callback_server: bool,
    /// The provider signing in.
    provider_id: String,
    /// Publishes the pasted redirect once.
    manual: Rc<watch::Sender<ManualResult>>,
}

impl LoginPresentation {
    /// Shows one callback.
    fn present(&self, event: LoginEvent) -> Result<(), DiagnosticErrorInfo> {
        match event {
            LoginEvent::Auth(info) => self.present_auth(&info),
            LoginEvent::Prompt(prompt, reply) => self.present_prompt(&prompt, reply),
            LoginEvent::Progress(message) => self
                .dialog
                .show_progress(&message)
                .map_err(login_presentation_error),
            LoginEvent::Select(prompt, reply) => self.present_select(prompt, reply),
        }
    }

    /// Shows the authorization link, then either the pasted-redirect entry or the waiting
    /// line of a provider that polls.
    fn present_auth(&self, info: &OAuthAuthInfo) -> Result<(), DiagnosticErrorInfo> {
        self.dialog
            .show_auth(&info.url, info.instructions.as_deref())
            .map_err(login_presentation_error)?;
        if self.uses_callback_server {
            let entered = self
                .dialog
                .show_manual_input("Paste redirect URL below, or complete login in browser:")
                .map_err(login_presentation_error)?;
            let (manual, signal) = (Rc::clone(&self.manual), self.dialog.signal().clone());
            self.spawn(publish_manual_input(entered, signal, manual));
        } else if self.provider_id == "github-copilot" {
            self.dialog
                .show_waiting("Waiting for browser authentication...")
                .map_err(login_presentation_error)?;
        }
        Ok(())
    }

    /// Shows the question and answers `reply` when it is submitted or cancelled.
    fn present_prompt(
        &self,
        prompt: &OAuthPrompt,
        reply: Reply<String>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let entered = self
            .dialog
            .show_prompt(&prompt.message, prompt.placeholder.as_deref())
            .map_err(login_presentation_error)?;
        self.spawn(async move {
            let answer = entered
                .await
                .map_err(|error| login_presentation_error(error).into());
            // The provider that stopped waiting no longer wants the answer.
            drop(reply.send(answer));
        });
        Ok(())
    }

    /// Shows the choice in place of the dialog and answers `reply` with the chosen option.
    fn present_select(
        &self,
        prompt: OAuthSelectPrompt,
        reply: Reply<Option<String>>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let Some(mode) = self.mode.upgrade() else {
            return Err(login_presentation_error(LoginDialogError::Cancelled));
        };
        let chosen = mode
            .show_oauth_login_select(Rc::clone(&self.dialog), prompt)
            .map_err(login_presentation_error)?;
        self.spawn(async move {
            drop(reply.send(Ok(chosen.await)));
        });
        Ok(())
    }

    /// Runs `work` on the terminal runtime while the mode is alive.
    fn spawn(&self, work: impl Future<Output = ()> + 'static) {
        if let Some(mode) = self.mode.upgrade() {
            mode.tool_environment
                .runtime
                .spawn_local(Box::pin(async move {
                    work.await;
                    Ok(())
                }));
        }
    }
}

/// Publishes the pasted redirect once `entered` brings nonempty text, or cancellation once
/// `signal` was aborted; empty text, or entry replaced by a later question, leaves it
/// unanswered.
async fn publish_manual_input(
    entered: LoginInputFuture,
    signal: Cancellation,
    manual: Rc<watch::Sender<ManualResult>>,
) {
    let answer = match entered.await {
        Ok(value) if !value.is_empty() => Ok(value),
        Err(error) if signal.is_aborted() => Err(login_presentation_error(error)),
        _ => return,
    };
    manual.send_if_modified(|current| {
        let unanswered = current.is_none();
        if unanswered {
            *current = Some(answer);
        }
        unanswered
    });
}

impl InteractiveMode {
    /// Signs in to the subscription `provider_id` in a dialog that shows each of its
    /// callbacks, then puts the editor back and completes the sign-in or shows its failure.
    ///
    /// # Errors
    /// Returns the published theme's failure while showing the dialog or a failure.
    pub(in crate::interactive_mode) async fn show_login_dialog(
        self: &Rc<Self>,
        provider_id: &str,
        provider_name: &str,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        let storage = Arc::clone(session.model_registry().auth_storage());
        let uses_callback_server = storage
            .get_oauth_providers()
            .iter()
            .find(|provider| provider.id() == provider_id)
            .and_then(|provider| provider.uses_callback_server())
            .unwrap_or(false);
        let previous_model = session.model();
        let dialog = self.sign_in_dialog(provider_id, provider_name)?;
        self.show_panel(dialog.clone(), dialog.clone());
        let (manual, entered) = watch::channel(None);
        let presentation = LoginPresentation {
            mode: Rc::downgrade(self),
            dialog: Rc::clone(&dialog),
            uses_callback_server,
            provider_id: provider_id.to_owned(),
            manual: Rc::new(manual),
        };
        let dispatch = local_dispatch(&self.tool_environment.runtime, move |event| {
            presentation.present(event)
        });
        let callbacks: OAuthCallbacks = Arc::new(LoginCallbacks {
            dispatch: dispatch.requester(),
            signal: dialog.signal().clone(),
            manual: entered,
        });
        let outcome = storage.login(provider_id, callbacks).await;
        let presented = dispatch.settle().await;
        let outcome = outcome.and(presented.map_err(Into::into));
        self.finish_provider_login(provider_id, provider_name, previous_model, outcome)
            .await
    }

    /// Puts the editor back, then completes a successful sign-in or shows its failure.
    ///
    /// # Errors
    /// Returns the published theme's failure while showing a failure.
    async fn finish_provider_login(
        self: &Rc<Self>,
        provider_id: &str,
        provider_name: &str,
        previous_model: Arc<RwLock<Model>>,
        outcome: Result<(), OAuthError>,
    ) -> Result<(), Box<dyn Error>> {
        self.restore_editor();
        self.ui.request_render(false);
        let completed = match outcome {
            Ok(()) => {
                self.complete_provider_authentication(
                    provider_id,
                    provider_name,
                    AuthType::OAuth,
                    previous_model,
                )
                .await
            }
            Err(error) => Err(error.into()),
        };
        match completed {
            Ok(()) => Ok(()),
            Err(error) => self
                .show_login_failure(&format!("Failed to login to {provider_name}"), &*error)
                .map_err(Into::into),
        }
    }

    /// Shows `error` after `action` unless the sign-in was cancelled, which stays silent.
    ///
    /// # Errors
    /// Returns the published theme's failure.
    pub(super) fn show_login_failure(
        &self,
        action: &str,
        error: &dyn Error,
    ) -> Result<(), ThemeError> {
        let message = error.to_string();
        if message == LoginDialogError::Cancelled.to_string() {
            return Ok(());
        }
        self.show_error(&format!("{action}: {message}"))
    }
}
