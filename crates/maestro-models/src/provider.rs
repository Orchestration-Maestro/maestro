//! Indexed adapter updates and supplied request cancellation.

use crate::{Cancellation, Context, Failure, Model, StopReason, Usage};
use std::{future::Future, pin::Pin};

/// Request-local streaming options.
#[derive(Clone, Default)]
pub struct StreamOptions {
    /// Supplied signal shared by clones of this request.
    pub cancellation: Cancellation,
}

/// Indexed source updates normalized into owned caller events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderUpdate {
    /// Open an empty text block.
    TextStart {
        /// Stable content-block index.
        content_index: usize,
    },
    /// Append a supplied fragment to an open text block.
    TextDelta {
        /// Stable content-block index.
        content_index: usize,
        /// Exact supplied fragment, including an empty fragment.
        delta: String,
    },
    /// Close a text block.
    TextEnd {
        /// Stable content-block index.
        content_index: usize,
    },
    /// Open empty readable thinking or introduce opaque redacted thinking.
    ThinkingStart {
        /// Stable content-block index.
        content_index: usize,
        /// Optional opaque replay signature.
        signature: Option<String>,
    },
    /// Append readable reasoning; never carries opaque redacted data.
    ThinkingDelta {
        /// Stable content-block index.
        content_index: usize,
        /// Exact supplied fragment, including an empty fragment.
        delta: String,
    },
    /// Close thinking; readable content is empty for redacted thinking.
    ThinkingEnd {
        /// Stable content-block index.
        content_index: usize,
    },
    /// Introduce an already-complete opaque block, with no readable delta.
    RedactedThinking {
        /// Stable content-block index.
        content_index: usize,
        /// Unmodified opaque redacted data.
        data: String,
    },
    /// Open a tool call with unavailable arguments.
    ToolCallStart {
        /// Stable index of this content block.
        content_index: usize,
        /// Explicit tool-call identifier.
        id: String,
        /// Tool name.
        name: String,
        /// Optional opaque replay metadata.
        replay_metadata: Option<String>,
    },
    /// Append a private JSON fragment without making arguments executable.
    ToolCallDelta {
        /// Stable content-block index.
        content_index: usize,
        /// Exact supplied fragment, including an empty fragment.
        delta: String,
    },
    /// Close a tool call only after strict JSON-object parsing.
    ToolCallEnd {
        /// Stable content-block index.
        content_index: usize,
    },
    /// Replace explicitly reported usage without estimation.
    Usage {
        /// Explicit reported usage snapshot, replacing the previous counters.
        usage: Usage,
    },
    /// Update actual response identity independently of requested identity.
    ResponseIdentity {
        /// Optional actual model; never rewrites requested identity.
        response_model: Option<String>,
        /// Optional actual response identifier.
        response_id: Option<String>,
    },
    /// Successful terminal outcome: Stop, Length or ToolUse only.
    Done {
        /// Terminal outcome.
        reason: StopReason,
    },
    /// Failed terminal outcome; Cancelled maps to Aborted.
    Error {
        /// Secret-safe typed failure category.
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
    /// Synchronous setup must not block; asynchronous work belongs in the source.
    fn stream(
        &self,
        model: Model,
        context: Context,
        options: StreamOptions,
    ) -> Result<Box<dyn ProviderStream>, Failure>;
}
