//! Credential-seam failure categories retained independently of invocation.
/// Recoverable categories whose display text is fixed and never includes request or adapter secrets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// No registration has the requested provider identifier.
    UnknownProvider,
    /// The provider has no registration for the requested model identity.
    UnknownModel,
    /// The operation or protocol is unsupported.
    UnsupportedOperation,
    /// An exact model identity is already registered.
    DuplicateModel,
    /// No scripted response remains.
    ScriptExhausted,
    /// The update source ended without a terminal update.
    IncompleteStream,
    /// The adapter could not set up or continue the request.
    AdapterFailed,
    /// Invalid block updates, completed tool JSON or accounting arithmetic.
    /// Unrepresentable token totals, invalid rates and non-finite estimates retain prior valid state.
    MalformedStream,
    /// Transport lost during a request.
    Transport,
    /// Local request work was cancelled.
    Cancelled,
    /// No usable request authentication was supplied.
    MissingAuthentication,
    /// Request credential resolution or exchange failed.
    AuthenticationFailed,
    /// Malformed or ambiguous literal header input.
    InvalidRequestHeaders,
    /// Supplied local metadata failed validation.
    InvalidCatalog,
    /// A trusted local catalog getter failed.
    CatalogFailed,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownProvider => "unknown provider: register a provider before invoking it",
            Self::UnknownModel => "unknown model: register the requested model identity",
            Self::UnsupportedOperation => "unsupported operation or protocol",
            Self::DuplicateModel => "model identity is already registered",
            Self::ScriptExhausted => "scripted provider has no response remaining",
            Self::IncompleteStream => "provider stream ended without a terminal update",
            Self::AdapterFailed => "provider adapter failed",
            Self::MalformedStream => "malformed provider stream",
            Self::Transport => "provider transport failed",
            Self::Cancelled => "request cancelled",
            Self::MissingAuthentication => "missing request authentication",
            Self::AuthenticationFailed => "request authentication failed",
            Self::InvalidCatalog => "invalid local model catalog",
            Self::CatalogFailed => "local model catalog getter failed",
            Self::InvalidRequestHeaders => "invalid request headers",
        })
    }
}

impl std::error::Error for Failure {}
