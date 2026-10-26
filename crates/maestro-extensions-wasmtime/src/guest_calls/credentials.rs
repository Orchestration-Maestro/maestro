//! Authored credential callbacks retain the existing Store lifecycle and scoped operands.
use super::{Call, Handle};
use crate::{
    lifecycle::replies::Reply,
    local_resources::{Lease, Registry, credential_scope::Endpoint},
};
use maestro_extensions::{
    AsyncLockUpdate, AuthStorageBackend, AuthStorageError, AuthStorageFuture, LockUpdate,
};
use std::sync::Arc;
mod pending;

/// Credential callback families share the original invocation and release path.
pub(crate) enum CredentialCall<H = Handle> {
    /// A backend using its native borrowed update.
    Backend(BackendCall<H>),
    /// A resolver returning an optional key.
    Fallback(FallbackCall<H>),
}
/// A backend invocation lends the update only for its actual native call.
pub(crate) struct BackendCall<H = Handle> {
    /// Retained authored backend identity.
    pub(crate) handler: H,
    /// Invocation-local borrowed native update.
    pub(crate) update: Endpoint,
    /// Selects the synchronous or awaited author method.
    pub(crate) asynchronous: bool,
    /// Original native completion.
    pub(crate) reply: Reply<()>,
}
/// A synchronous fallback returns its actual optional value to its native caller.
pub(crate) struct FallbackCall<H = Handle> {
    /// Retained authored resolver identity.
    pub(crate) handler: H,
    /// Literal provider identifier.
    pub(crate) provider: String,
    /// The original typed native result, never unit-only success.
    pub(crate) reply: Reply<Option<String>>,
}
/// A Send native backend whose production callers stay on the creating pump thread.
pub(crate) struct Backend {
    /// Original authored callback and Store admission.
    pub(crate) handler: Handle,
    /// Send lookup of the creating pump, not an Rc moved across threads.
    pub(crate) owner: Arc<Lease>,
}
impl AuthStorageBackend for Backend {
    fn with_lock(
        &self,
        update: &mut dyn FnMut(Option<&str>) -> LockUpdate,
    ) -> Result<(), AuthStorageError> {
        Registry::on_pump(self.owner.requests(), |registry| {
            self.synchronous(registry, update)
        })
        .and_then(std::convert::identity)
        .map_err(Into::into)
    }

    fn with_lock_async<'a>(
        &'a self,
        update: AsyncLockUpdate<'a>,
    ) -> AuthStorageFuture<'a, Result<(), AuthStorageError>> {
        pending::run(self, update)
    }
}
impl Backend {
    /// Enters the author backend through the shared borrowed-operand service.
    fn synchronous(
        &self,
        registry: &Registry,
        update: &mut dyn FnMut(Option<&str>) -> LockUpdate,
    ) -> Result<(), String> {
        self.handler.endpoint().replies.synchronous_operand(
            registry,
            crate::local_resources::synchronous::Operand::Lock(update),
            |reply, node| {
                registry.dispatch(
                    self.handler.endpoint(),
                    Call::Credential(CredentialCall::Backend(BackendCall {
                        handler: self.handler.clone(),
                        update: Endpoint {
                            id: node.id,
                            requests: node.requests,
                        },
                        asynchronous: false,
                        reply,
                    })),
                )
            },
        )
    }
}
impl Handle {
    /// Invokes one resolver with a typed optional result before its native caller continues.
    fn resolve(&self, registry: &Registry, provider: &str) -> Result<Option<String>, String> {
        self.endpoint().synchronous(registry, |reply| {
            Ok(Call::Credential(CredentialCall::Fallback(FallbackCall {
                handler: self.clone(),
                provider: provider.into(),
                reply,
            })))
        })
    }
    /// Retains pump admission, not a capability that could own these credentials.
    pub(crate) fn fallback(
        &self,
        requests: tokio::sync::mpsc::UnboundedSender<crate::local_resources::Message>,
    ) -> impl Fn(&str) -> Result<Option<String>, maestro_extensions::OAuthError> + Send + Sync + 'static
    {
        let handler = self.clone();
        move |provider| {
            Registry::on_pump(&requests, |registry| handler.resolve(registry, provider))
                .and_then(std::convert::identity)
                .map_err(|error| maestro_extensions::DiagnosticErrorInfo::error(error).into())
        }
    }
}
