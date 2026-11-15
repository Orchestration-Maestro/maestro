//! Native provider metadata and operations retain the same host resource.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{Imports, session::decode};
use crate::{ExtensionFuture, ExtensionResult, Model, OAuthCredentials, OAuthProviderInterface};
use std::rc::Rc;

/// Original imports beside their independently retained native provider.
struct Provider<I: Imports> {
    /// Calls into the creating host.
    imports: I,
    /// Retains the provider rather than looking it up again on each call.
    resource: Rc<I::OAuthProvider>,
    /// Literal provider identifier.
    id: String,
    /// Login display label.
    name: String,
    /// Optional callback-server declaration.
    uses_callback_server: Option<bool>,
}
/// Decodes metadata while retaining the provider's callable resource.
pub(super) fn provider<I: Imports>(
    imports: I,
    resource: I::OAuthProvider,
) -> ExtensionResult<Rc<dyn OAuthProviderInterface>> {
    let (id, name, uses_callback_server) = decode(&imports.oauth_info(&resource)?)?;
    Ok(Rc::new(Provider {
        imports,
        resource: Rc::new(resource),
        id,
        name,
        uses_callback_server,
    }))
}
impl<I: Imports> OAuthProviderInterface for Provider<I> {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn uses_callback_server(&self) -> Option<bool> {
        self.uses_callback_server
    }
    fn login(
        &self,
        callbacks: Rc<crate::OAuthLoginCallbacks>,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        let (imports, resource) = (self.imports.clone(), self.resource.clone());
        Box::pin(async move {
            let login = Login::new(&imports, callbacks)?;
            let result = imports
                .oauth_login(
                    &resource,
                    &login.handler.handle,
                    (login.options, login.signal.signal()),
                )
                .await;
            login.handler.settle(true);
            decode(&result?)
        })
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        let (imports, resource) = (self.imports.clone(), self.resource.clone());
        Box::pin(async move {
            let data = crate::types::finite::encode_value(&credentials)?;
            decode(&imports.oauth_refresh(&resource, data).await?)
        })
    }
    fn modify_models(
        &self,
        models: &[Model],
        credentials: &OAuthCredentials,
    ) -> ExtensionResult<Option<Vec<Model>>> {
        let data = crate::types::finite::encode_value(&(models, credentials))?;
        decode(&self.imports.oauth_models(&self.resource, &data)?)
    }
    fn get_api_key(&self, credentials: &OAuthCredentials) -> ExtensionResult<String> {
        self.imports.oauth_key(
            &self.resource,
            &crate::types::finite::encode_value(credentials)?,
        )
    }
}

/// Stages one interaction using the same identity and signal lifetime for both login entry points.
pub(super) struct Login<I: Imports> {
    /// Native final release removes the interaction's guest captures.
    pub(super) handler: super::callbacks::Identity<I>,
    /// The supplied signal follows live caller cancellation.
    pub(super) signal: Rc<super::signals::Mirror<I>>,
    /// Presence of progress, manual-code and selection callbacks.
    pub(super) options: (bool, bool, bool),
}
impl<I: Imports> Login<I> {
    /// Retains the callbacks and signal mirror under the native final-release identity.
    pub(super) fn new(
        imports: &I,
        callbacks: Rc<crate::OAuthLoginCallbacks>,
    ) -> ExtensionResult<Self> {
        let signal = Rc::new(super::signals::Mirror::new(
            imports,
            callbacks.signal.as_ref(),
        )?);
        let options = callbacks.callback_presence();
        let handler = super::callbacks::Identity::new(imports);
        handler.stage(Rc::new(Interaction {
            callbacks,
            _signal: Box::new(signal.clone()),
        }));
        Ok(Self {
            handler,
            signal,
            options,
        })
    }
}

/// Retains the caller's signal binding for as long as native callbacks retain this identity.
pub(super) struct Interaction {
    /// Original author-supplied operations.
    callbacks: Rc<crate::OAuthLoginCallbacks>,
    /// Native signal mirror with the original subscription removal.
    _signal: Box<dyn std::any::Any>,
}
/// Invokes the original authored callback without holding its table borrow.
pub(crate) async fn invoke(
    handler: u32,
    phase: crate::bindings::maestro::extension::host::OauthCallback,
    data: &str,
    interaction: Option<Rc<crate::OAuthLoginCallbacks>>,
) -> ExtensionResult<Option<String>> {
    use crate::bindings::maestro::extension::host::OauthCallback;
    if matches!(
        phase,
        OauthCallback::Key | OauthCallback::Models | OauthCallback::Refresh | OauthCallback::Login
    ) {
        return invoke_provider(handler, phase, data, interaction)
            .await
            .map(Some);
    }
    let interaction = super::callbacks::find::<Rc<Interaction>>(handler)?;
    let callbacks = &interaction.callbacks;
    match phase {
        OauthCallback::Auth => {
            (callbacks.on_auth)(decode(data)?)?;
            Ok(None)
        }
        OauthCallback::Progress => {
            if let Some(callback) = &callbacks.on_progress {
                callback(data)?;
            }
            Ok(None)
        }
        OauthCallback::Prompt => (callbacks.on_prompt)(decode(data)?).await.map(Some),
        OauthCallback::Manual => callbacks
            .on_manual_code_input
            .as_ref()
            .ok_or("missing manual code callback")?()
        .await
        .map(Some),
        OauthCallback::Key
        | OauthCallback::Models
        | OauthCallback::Refresh
        | OauthCallback::Login => Err("invalid OAuth interaction phase".into()),
        OauthCallback::Select => {
            callbacks
                .on_select
                .as_ref()
                .ok_or("missing selection callback")?(decode(data)?)
            .await
        }
    }
}

/// Invokes the original author implementation without holding its callback-table borrow.
async fn invoke_provider(
    handler: u32,
    phase: crate::bindings::maestro::extension::host::OauthCallback,
    data: &str,
    interaction: Option<Rc<crate::OAuthLoginCallbacks>>,
) -> ExtensionResult<String> {
    use crate::bindings::maestro::extension::host::OauthCallback;
    use crate::types::finite::encode_value;
    let provider = super::callbacks::find::<Rc<dyn crate::OAuthProviderInterface>>(handler)?;
    match phase {
        OauthCallback::Key => provider.get_api_key(&decode(data)?),
        OauthCallback::Models => {
            let (models, credentials): (Vec<Model>, _) = decode(data)?;
            encode_value(&provider.modify_models(&models, &credentials)?)
        }
        OauthCallback::Refresh => encode_value(&provider.refresh_token(decode(data)?).await?),
        OauthCallback::Login => encode_value(
            &provider
                .login(interaction.ok_or("missing OAuth interaction")?)
                .await?,
        ),
        _ => Err("invalid OAuth provider operation".into()),
    }
}
