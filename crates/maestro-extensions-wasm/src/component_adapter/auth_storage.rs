//! Credential operations on an independently owned host resource.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{Imports, session::decode};
use crate::types::finite::encode_value;
use crate::{
    AuthCredential, AuthStatus, AuthStorage, AuthStorageData, AuthStoragePort, DiagnosticErrorInfo,
    ExtensionFuture, ExtensionResult, GetApiKeyOptions,
};
use std::rc::Rc;
/// The original host imports beside their retained credential resource.
struct Owner<I: Imports> {
    /// Calls into the creating host.
    imports: I,
    /// Independently owned native credentials.
    resource: Rc<I::Credentials>,
}
/// Wraps an independently retained host resource in the public credential facade.
pub(super) fn storage<I: Imports>(imports: I, resource: I::Credentials) -> AuthStorage {
    let resource = Rc::new(resource);
    let mut storage = AuthStorage::new(Rc::new(Owner {
        imports,
        resource: resource.clone(),
    }));
    storage.resource = Some(resource);
    storage
}
impl<I: Imports> AuthStoragePort for Owner<I> {
    fn login(
        &self,
        provider: &str,
        callbacks: Rc<crate::OAuthLoginCallbacks>,
    ) -> ExtensionFuture<'static, ()> {
        let (imports, resource, provider) = (
            self.imports.clone(),
            self.resource.clone(),
            provider.to_owned(),
        );
        Box::pin(async move {
            let login = super::oauth::Login::new(&imports, callbacks)?;
            let result = imports
                .credentials_login(
                    &resource,
                    provider,
                    &login.handler.handle,
                    (login.options, login.signal.signal()),
                )
                .await;
            login.handler.settle(true);
            result
        })
    }
    fn set_fallback_resolver(&self, resolver: crate::FallbackResolver) -> ExtensionResult<()> {
        let identity = super::callbacks::Identity::new(&self.imports);
        identity.stage(resolver);
        let result = self
            .imports
            .credentials_set_fallback(&self.resource, &identity.handle);
        identity.settle(result.is_ok());
        result
    }
    fn set_runtime_api_key(&self, provider: &str, api_key: &str) -> ExtensionResult<()> {
        self.imports
            .credentials_set_runtime_api_key(&self.resource, provider, api_key)
    }
    fn remove_runtime_api_key(&self, provider: &str) -> ExtensionResult<()> {
        self.imports
            .credentials_remove_runtime_api_key(&self.resource, provider)
    }
    fn reload(&self) -> ExtensionResult<()> {
        self.imports.credentials_reload(&self.resource)
    }
    fn get(&self, provider: &str) -> ExtensionResult<Option<AuthCredential>> {
        self.imports
            .credentials_get(&self.resource, provider)?
            .as_deref()
            .map(decode)
            .transpose()
    }
    fn set(&self, provider: &str, credential: AuthCredential) -> ExtensionResult<()> {
        self.imports
            .credentials_set(&self.resource, provider, &encode_value(&credential)?)
    }
    fn remove(&self, provider: &str) -> ExtensionResult<()> {
        self.imports.credentials_remove(&self.resource, provider)
    }
    fn get_oauth_providers(&self) -> ExtensionResult<Vec<Rc<dyn crate::OAuthProviderInterface>>> {
        self.imports
            .credentials_oauth_providers(&self.resource)?
            .into_iter()
            .map(|resource| super::oauth::provider(self.imports.clone(), resource))
            .collect()
    }
    fn list(&self) -> ExtensionResult<Vec<String>> {
        self.imports.credentials_list(&self.resource)
    }
    fn has(&self, provider: &str) -> ExtensionResult<bool> {
        self.imports.credentials_has(&self.resource, provider)
    }
    fn has_auth(&self, provider: &str) -> ExtensionResult<bool> {
        self.imports.credentials_has_auth(&self.resource, provider)
    }
    fn get_auth_status(&self, provider: &str) -> ExtensionResult<AuthStatus> {
        decode(
            &self
                .imports
                .credentials_get_auth_status(&self.resource, provider)?,
        )
    }
    fn get_all(&self) -> ExtensionResult<AuthStorageData> {
        decode(&self.imports.credentials_get_all(&self.resource)?)
    }
    fn drain_errors(&self) -> ExtensionResult<Vec<DiagnosticErrorInfo>> {
        decode(&self.imports.credentials_drain_errors(&self.resource)?)
    }
    fn logout(&self, provider: &str) -> ExtensionResult<()> {
        self.imports.credentials_logout(&self.resource, provider)
    }
    fn get_api_key<'a>(
        &'a self,
        provider: &'a str,
        options: Option<GetApiKeyOptions>,
    ) -> ExtensionFuture<'a, Option<String>> {
        self.imports.credentials_get_api_key(
            &self.resource,
            provider.to_owned(),
            options.and_then(|o| o.include_fallback),
        )
    }
}

/// Borrows the original transport resource, rejecting supplied or foreign port types.
pub(super) fn resource<I: Imports>(storage: &AuthStorage) -> ExtensionResult<&I::Credentials> {
    storage
        .resource
        .as_deref()
        .and_then(|value| value.downcast_ref())
        .ok_or_else(|| "extension credential resource belongs to another host".into())
}

/// Invokes the author's backend outside the callback table borrow.
pub(crate) fn invoke_backend<I: Imports>(
    imports: &I,
    target: u32,
    update: &I::LockUpdate,
) -> ExtensionResult<()> {
    let backend = super::callbacks::find::<Rc<dyn crate::AuthStorageBackend>>(target)?;
    backend.with_lock(&mut |current| imports.lock_update(update, current))
}
/// Awaits the author's backend, keeping both its owner and the borrowed update until settlement.
pub(super) async fn invoke_backend_async<I: Imports>(
    imports: &I,
    target: u32,
    update: &I::LockUpdate,
) -> ExtensionResult<()> {
    let backend = super::callbacks::find::<Rc<dyn crate::AuthStorageBackend>>(target)?;
    backend
        .with_lock_async(Box::new(|current| {
            imports.lock_update_async(update, current)
        }))
        .await
}
/// Delivers the exact authored fallback result, including absent and empty values.
pub(crate) fn invoke_fallback(target: u32, provider: &str) -> ExtensionResult<Option<String>> {
    super::callbacks::find::<crate::FallbackResolver>(target)?(provider)
}

#[cfg(test)]
mod tests;
