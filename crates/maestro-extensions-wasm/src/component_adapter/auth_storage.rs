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
    resource: I::Credentials,
}
/// Wraps an independently retained host resource in the public credential facade.
pub(super) fn storage<I: Imports>(imports: I, resource: I::Credentials) -> AuthStorage {
    AuthStorage::new(Rc::new(Owner { imports, resource }))
}
impl<I: Imports> AuthStoragePort for Owner<I> {
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
