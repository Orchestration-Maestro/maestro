//! Registered callbacks keep the actual native caller's capabilities.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use crate::{
    context_reads::{Endpoint, Owner},
    local_resources::Registry,
};
use maestro_extensions::{
    AutocompleteItem, EventError, Local,
    types::{ArgumentCompletions, CommandHandler, ShortcutHandler},
};
use std::rc::Rc;

/// Typed exports selected by the real command and shortcut registration owners.
pub(crate) enum RegisteredCall<H = Handle> {
    /// Exact command arguments and original command context.
    Command(H, String, Endpoint, Reply<()>),
    /// Exact argument prefix and independently optional ordered candidates.
    Completion(H, String, Reply<Option<Vec<AutocompleteItem>>>),
    /// Original ordinary context supplied by the shortcut caller.
    Shortcut(H, Endpoint, Reply<()>),
}
impl Handle {
    /// Adapts the native command without substituting a loading or current context.
    pub(crate) fn command(&self, registry: Registry) -> CommandHandler<Local> {
        let handler = self.clone();
        Rc::new(move |args, context| {
            let context = registry.context(Owner::Command(context));
            Box::pin(invoke_command(
                handler.clone(),
                registry.clone(),
                args,
                context,
            ))
        })
    }
    /// Adapts the shortcut's original ordinary context on the native local owner.
    pub(crate) fn shortcut(&self, registry: Registry) -> ShortcutHandler<Local> {
        let handler = self.clone();
        Rc::new(move |context| {
            let context = registry.context(Owner::Base(context));
            Box::pin(invoke_shortcut(handler.clone(), registry.clone(), context))
        })
    }
    /// Preserves absent and present-empty completion lists as distinct native outcomes.
    pub(crate) fn completions(&self, registry: Registry) -> ArgumentCompletions<Local> {
        let handler = self.clone();
        Rc::new(move |prefix| {
            Box::pin(invoke_completion(handler.clone(), registry.clone(), prefix))
        })
    }
}
/// Awaits the original command after its supplied context has crossed the local boundary.
async fn invoke_command(
    handler: Handle,
    registry: Registry,
    args: String,
    context: Result<Endpoint, String>,
) -> Result<(), EventError> {
    let context = context?;
    handler
        .0
        .endpoint
        .call(&registry, |reply| {
            Call::Registered(RegisteredCall::Command(
                handler.clone(),
                args,
                context,
                reply,
            ))
        })
        .await
        .map_err(Into::into)
}
/// Awaits the original shortcut with no command-context substitution.
async fn invoke_shortcut(
    handler: Handle,
    registry: Registry,
    context: Result<Endpoint, String>,
) -> Result<(), EventError> {
    let context = context?;
    handler
        .0
        .endpoint
        .call(&registry, |reply| {
            Call::Registered(RegisteredCall::Shortcut(handler.clone(), context, reply))
        })
        .await
        .map_err(Into::into)
}
/// Returns the authored optional completion list unchanged.
async fn invoke_completion(
    handler: Handle,
    registry: Registry,
    prefix: String,
) -> Result<Option<Vec<AutocompleteItem>>, EventError> {
    handler
        .0
        .endpoint
        .call(&registry, |reply| {
            Call::Registered(RegisteredCall::Completion(handler.clone(), prefix, reply))
        })
        .await
        .map_err(Into::into)
}
