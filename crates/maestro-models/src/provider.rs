//! Indexed adapter updates and supplied request cancellation.

use crate::{
    AssistantMessage, Cancellation, Context, Failure, Model, RequestAuth, StopReason,
    StreamOptions, ThinkingLevel, Usage,
};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

/// Authorized inputs for one adapter, without a resolver surface.
#[derive(Clone)]
pub struct ProviderOptions {
    /// Supported tool selection.
    pub tool_choice: Option<crate::ToolChoice>,
    /// Per-attempt header timeout; absent resolves to 600000 milliseconds.
    pub timeout_ms: Option<u64>,
    /// Additional setup retries; absent resolves to two.
    pub max_retries: Option<u32>,
    /// Shared local cancellation signal.
    pub cancellation: Cancellation,
    /// Original application-supplied reasoning choice.
    pub requested_thinking: ThinkingLevel,
    /// Supported reasoning choice from registered metadata.
    pub thinking: ThinkingLevel,
    /// Resolved effort string, mutually exclusive with a token budget.
    pub effort: Option<String>,
    /// Resolved shared-ceiling reasoning budget.
    pub thinking_budget: Option<u64>,
    /// Supported supplied temperature, without a default or reasoning overlay.
    pub temperature: Option<f64>,
    /// Resolved output allowance, including shared-ceiling adjustment if enabled.
    pub output_limit: Option<u64>,
    /// Supported supplied transport preference.
    pub transport: Option<String>,
    /// Supported supplied cache preference.
    pub cache_preference: Option<String>,
    /// Supported supplied session affinity.
    pub session_affinity: Option<String>,
    /// Resolved request authentication.
    pub auth: RequestAuth,
    /// Effective literal headers; sensitive and not safe to log.
    pub headers: BTreeMap<String, String>,
}

/// Inert provider metadata, never an environment reader.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ProviderDescription {
    /// Arbitrary declared ambient credential names, without discovery policy.
    pub ambient_credential_names: Vec<String>,
    /// Literal default headers; sensitive and not safe to log.
    pub headers: BTreeMap<String, String>,
}

/// Indexed source updates normalized into owned caller events.
#[derive(Clone, Debug, PartialEq)]
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
    /// Close a text block, attaching supplied opaque replay metadata.
    TextEnd {
        /// Stable content-block index.
        content_index: usize,
        /// Opaque metadata attached before the closing snapshot.
        replay_metadata: Option<String>,
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
    /// Update metadata on an open tool block without emitting a model event.
    ToolCallMetadata {
        /// Stable content-block index.
        content_index: usize,
        /// Replacement identifier, when supplied.
        id: Option<String>,
        /// Replacement name, when supplied.
        name: Option<String>,
        /// Opaque replay data, when supplied.
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
    /// Replace one attempt's flat report with authoritative non-overlapping categories.
    /// Input excludes cache reads/writes; cache reads exclude current writes; output includes reasoning.
    /// Adapters own raw overlap subtraction and assemble complete snapshots, not deltas.
    /// The normalizer ignores supplied totals, reporting flags and costs, derives them from
    /// captured catalog rates, and rejects invalid arithmetic without replacing prior state.
    Usage {
        /// Explicit reported category snapshot, replacing the previous counters.
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
    /// Return owned metadata without I/O.
    fn description(&self) -> ProviderDescription {
        ProviderDescription::default()
    }
    /// Obtain an optional exchange primitive without executing it.
    /// Only the credential owner invokes this operation; models never do.
    fn token_exchange(&self) -> Option<Arc<dyn crate::TokenExchange>> {
        None
    }
    /// Apply this adapter's deterministic, effect-free foreign call-ID rule.
    /// The identity default preserves IDs; overrides must keep batch IDs distinct.
    fn normalize_tool_call_id(
        &self,
        id: &str,
        _model: &Model,
        _source: &AssistantMessage,
    ) -> String {
        id.to_owned()
    }
    /// Start exactly one invocation with an owned, already projected context, or return a safe setup failure.
    /// The model interface converts setup failures into terminal error events.
    /// Synchronous setup must not block; asynchronous work belongs in the source.
    fn stream(
        &self,
        model: Model,
        context: Context,
        options: ProviderOptions,
    ) -> Result<Box<dyn ProviderStream>, Failure>;
}

pub(crate) struct RedactedHeaders<'a>(pub(crate) &'a BTreeMap<String, String>);
impl std::fmt::Debug for RedactedHeaders<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.0.keys().map(|key| (key, "[REDACTED]")))
            .finish()
    }
}
impl std::fmt::Debug for StreamOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamOptions")
            .field("auth", &self.auth)
            .field(
                "auth_resolver",
                &self.auth_resolver.as_ref().map(|_| "[SUPPLIED]"),
            )
            .field("headers", &RedactedHeaders(&self.headers))
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for ProviderOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderOptions")
            .field("auth", &self.auth)
            .field("headers", &RedactedHeaders(&self.headers))
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for ProviderDescription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderDescription")
            .field("ambient_credential_names", &self.ambient_credential_names)
            .field("headers", &RedactedHeaders(&self.headers))
            .finish()
    }
}
