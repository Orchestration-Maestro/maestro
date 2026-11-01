//! The borrowed native update stays in this Send future until its author backend settles.
use super::{Backend, BackendCall, Call, CredentialCall};
use crate::local_resources::{
    Registry,
    credential_scope::{AsyncRequest, Scope},
};
use maestro_extensions::{AsyncLockUpdate, AuthStorageError, AuthStorageFuture, LockUpdate};
use std::{
    future::Future,
    task::{Context, Poll},
};

/// One pending borrowed update beside its ordinary lifecycle reply.
struct Pending<'a> {
    /// Single native update, never moved into static storage.
    update: Option<AsyncLockUpdate<'a>>,
    /// Native update future, bounded by its original borrow.
    running: Option<AuthStorageFuture<'a, LockUpdate>>,
    /// Reply to the author's current update call.
    replacement: Option<tokio::sync::oneshot::Sender<Result<Option<String>, String>>>,
    /// Invocation scope holds owned requests in the existing operand service.
    scope: Option<Scope>,
    /// The backend's original lifecycle reply.
    answer: Option<tokio::sync::oneshot::Receiver<Result<(), String>>>,
}
/// Returns a Send future without moving the local registry across a suspension.
pub(super) fn run<'a>(
    backend: &'a Backend,
    update: AsyncLockUpdate<'a>,
) -> AuthStorageFuture<'a, Result<(), AuthStorageError>> {
    let mut pending = Pending {
        update: Some(update),
        running: None,
        replacement: None,
        scope: None,
        answer: None,
    };
    Box::pin(std::future::poll_fn(
        move |context| match Registry::on_pump(backend.owner.requests(), |registry| {
            pending.poll(backend, registry, context)
        }) {
            Ok(Ok(Poll::Pending)) => Poll::Pending,
            Ok(Ok(Poll::Ready(value))) => Poll::Ready(value.map_err(Into::into)),
            Ok(Err(error)) | Err(error) => Poll::Ready(Err(error.into())),
        },
    ))
}
impl Pending<'_> {
    /// Starts once, serves native updates and observes backend settlement.
    fn poll(
        &mut self,
        backend: &Backend,
        registry: &Registry,
        context: &mut Context<'_>,
    ) -> Result<Poll<Result<(), String>>, String> {
        if self.scope.is_none() {
            self.start(backend, registry, context)?;
        }
        if let Some(scope) = &self.scope
            && let Some(request) = scope.request(registry, context)
        {
            self.begin(request);
        }
        self.poll_update(context);
        let answer = self
            .answer
            .as_mut()
            .ok_or("extension credential backend reply ended")?;
        Ok(std::pin::Pin::new(answer).poll(context).map(|value| {
            value
                .map_err(|_| "extension credential backend reply ended".into())
                .and_then(std::convert::identity)
        }))
    }
    /// Admits the author's method, keeping its scoped operand through its tail.
    fn start(
        &mut self,
        backend: &Backend,
        registry: &Registry,
        context: &Context<'_>,
    ) -> Result<(), String> {
        let scope = Scope::new(registry, context)?;
        let (reply, answer) = backend.handler.endpoint().replies.reply()?;
        registry.admit(
            backend.handler.endpoint(),
            Call::Credential(CredentialCall::Backend(BackendCall {
                handler: backend.handler.clone(),
                update: scope.endpoint.clone(),
                asynchronous: true,
                reply,
            })),
        )?;
        self.answer = Some(answer);
        self.scope = Some(scope);
        Ok(())
    }
    /// Consumes only the single `FnOnce` update admitted by this operand.
    fn begin(&mut self, request: AsyncRequest) {
        if let Some(update) = self.update.take() {
            self.running = Some(update(request.current));
            self.replacement = Some(request.reply);
        } else {
            let _ = request
                .reply
                .send(Err("extension lock update ended".into()));
        }
    }
    /// A pending native update yields to the local executor, without blocking it.
    fn poll_update(&mut self, context: &mut Context<'_>) {
        if let Some(work) = &mut self.running
            && let Poll::Ready(value) = work.as_mut().poll(context)
        {
            self.running = None;
            if let Some(reply) = self.replacement.take() {
                let _ = reply.send(value.map_err(|error| error.to_string()));
            }
        }
    }
}
