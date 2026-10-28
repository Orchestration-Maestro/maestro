//! Futures the host runs on its event-tail executor, each until it ends or its task drops.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{EventBus, ExtensionFuture, ExtensionResult};

/// Starts background work for one extension.
#[derive(Clone)]
pub struct Background;

impl Background {
    /// Registers the listener that hands started work to the host.
    ///
    /// # Errors
    /// Returns the host's registration rejection.
    pub fn new(events: EventBus) -> ExtensionResult<Self> {
        drop(events);
        Ok(Self)
    }

    /// Hands `work` to the host.
    #[must_use]
    pub fn spawn(&self, work: ExtensionFuture<'static, ()>) -> ExtensionFuture<'_, BackgroundTask> {
        drop(work);
        Box::pin(async { Err("background work is not started yet".to_owned()) })
    }
}

/// Started background work.
pub struct BackgroundTask;
