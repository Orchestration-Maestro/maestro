//! Runtime-independent adapter-owned update sources.

use crate::{Context, Failure, Model, Usage};
use std::future::Future;
use std::pin::Pin;

/// Adapter-owned text updates; the model module constructs caller events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderUpdate {
    /// A chunk to append to the text block.
    TextDelta {
        /// Explicit text chunk.
        delta: String,
    },
    /// Successful termination with reported usage.
    Done {
        /// Explicit final counters, without text-based estimation.
        usage: Usage,
    },
    /// Failed termination with a safe typed category.
    Error {
        /// Recoverable failure category.
        failure: Failure,
    },
}

/// A pull-based asynchronous source belonging to one dispatched request.
pub trait ProviderStream: Send {
    /// Obtain one update without requiring a runtime. No lock may span polling or awaiting.
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>>;
}

/// A replaceable adapter that implements declared operations.
pub trait Provider: Send + Sync {
    /// Report implemented operation capability independently of advertised registration.
    fn supports(&self, operation: &str) -> bool;
    /// Start exactly one invocation with owned inputs, or return a safe setup failure.
    /// The model interface converts setup failures into terminal error events.
    fn stream(&self, model: Model, context: Context) -> Result<Box<dyn ProviderStream>, Failure>;
}
