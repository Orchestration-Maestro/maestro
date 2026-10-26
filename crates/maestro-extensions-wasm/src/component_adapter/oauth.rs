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
