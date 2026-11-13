//! Replacement operations retain the actual creating session, not the current API.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    api_reads::delivery,
    command_reads::settled,
    context_reads::Owner,
    local_resources::{Lease, held::ReplacedContext},
};
use std::sync::Arc;

/// An independently retained original-session capability in the Store.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Retains a distinct guarded command view without retargeting the replacement.
    pub(crate) fn command(&self) -> Result<crate::context_reads::Endpoint, String> {
        self.0.run::<ReplacedContext, _>(|context, registry| {
            registry.context(Owner::Command((**context).clone()))
        })
    }

    /// Delivers a custom message to the creating session through the sole receiver.
    pub(crate) async fn send_message(
        &self,
        message: String,
        options: Option<String>,
    ) -> Result<(), String> {
        self.0
            .admit::<ReplacedContext, _, _>(move |context, _| {
                let (message, options) = delivery(&message, options.as_deref())?;
                Ok(settled(context.send_message(message, options)))
            })
            .await
    }

    /// Delivers text or ordered blocks to the creating session through the sole receiver.
    pub(crate) async fn send_user_message(
        &self,
        value: String,
        options: Option<String>,
    ) -> Result<(), String> {
        self.0
            .admit::<ReplacedContext, _, _>(move |context, _| {
                let (value, options) = delivery(&value, options.as_deref())?;
                Ok(settled(context.send_user_message(value, options)))
            })
            .await
    }
}
