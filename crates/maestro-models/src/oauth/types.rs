//! Shared OAuth credential and prompt records.
use std::borrow::Cow;

pub use maestro_request::oauth::OAuthCredentials;

pub use maestro_request::oauth::{
    OAuthAuthInfo, OAuthPrompt, OAuthSelectOption, OAuthSelectPrompt,
};

/// Open provider identifier.
pub type OAuthProviderId = String;
/// Provider identifier used by authorization records.
pub type OAuthProvider = OAuthProviderId;

/// OAuth failure retaining supplied diagnostics and its wrapped cause.
#[derive(Debug)]
pub struct OAuthError {
    /// Supplied error fields; an absent name denotes already-formatted text.
    pub diagnostic: Box<crate::DiagnosticErrorInfo>,
    /// Native or supplied operating-system error number.
    pub errno: Option<crate::DiagnosticCode>,
    /// Wrapped failure, when present.
    pub cause: Option<Box<Self>>,
}

impl From<crate::DiagnosticErrorInfo> for OAuthError {
    fn from(diagnostic: crate::DiagnosticErrorInfo) -> Self {
        Self {
            diagnostic: Box::new(diagnostic),
            errno: None,
            cause: None,
        }
    }
}

impl From<crate::FetchError> for OAuthError {
    fn from(error: crate::FetchError) -> Self {
        match error {
            crate::FetchError::Connection(diagnostic) => diagnostic.into(),
            other => Self::message(other.to_string()),
        }
    }
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic.message)
    }
}

impl std::error::Error for OAuthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &dyn std::error::Error)
    }
}

impl OAuthError {
    /// Construct an authored message without inventing diagnostic details.
    pub(crate) fn message(message: impl Into<String>) -> Self {
        crate::DiagnosticErrorInfo::new(None, message).into()
    }

    /// Format supplied fields, with nested cause before supplied stack text.
    pub(crate) fn details(&self) -> String {
        let mut parts = vec![self.diagnostic.name.as_ref().map_or_else(
            || self.diagnostic.message.clone(),
            |name| format!("{name}: {}", self.diagnostic.message),
        )];
        if let Some(code) = self.diagnostic.code.as_ref().filter(|code| match code {
            crate::DiagnosticCode::Text(text) => !text.is_empty(),
            crate::DiagnosticCode::Number(number) => *number != 0.0 && !number.is_nan(),
        }) {
            parts.push(format!("code={}", diagnostic_code(code)));
        }
        if let Some(errno) = &self.errno {
            parts.push(format!("errno={}", diagnostic_code(errno)));
        }
        if let Some(cause) = &self.cause {
            parts.push(format!("cause={}", cause.details()));
        }
        if let Some(stack) = self
            .diagnostic
            .stack
            .as_ref()
            .filter(|stack| !stack.is_empty())
        {
            parts.push(format!("stack={stack}"));
        }
        parts.join("; ")
    }
}

/// Spell supplied numeric codes with double-precision display semantics.
fn diagnostic_code(code: &crate::DiagnosticCode) -> String {
    match code {
        crate::DiagnosticCode::Text(text) => text.clone(),
        crate::DiagnosticCode::Number(number) => ryu_js::Buffer::new().format(*number).to_owned(),
    }
}

/// Retained interaction callbacks on native targets.
#[cfg(not(target_arch = "wasm32"))]
pub type OAuthCallbacks = std::sync::Arc<dyn OAuthLoginCallbacks + Send + Sync>;
/// Retained interaction callbacks on browser targets.
#[cfg(target_arch = "wasm32")]
pub type OAuthCallbacks = std::sync::Arc<dyn OAuthLoginCallbacks>;

/// Interaction supplied by the caller of an authorization flow.
pub trait OAuthLoginCallbacks {
    /// Publish authorization URL and instructions.
    ///
    /// # Errors
    /// Returns the caller's interaction failure.
    fn on_auth(&self, info: OAuthAuthInfo) -> Result<(), OAuthError>;
    /// Request pasted authorization text.
    fn on_prompt(&self, prompt: OAuthPrompt) -> crate::BoxFuture<Result<String, OAuthError>>;
    /// Reports whether progress, manual-code and selection callbacks are supplied, in that order.
    ///
    /// Override this alongside optional callback methods; the default declares all three absent.
    /// Reading presence does not start any callback or future.
    fn callback_presence(&self) -> (bool, bool, bool) {
        (false, false, false)
    }
    /// Publish progress; the default does nothing.
    ///
    /// # Errors
    /// Implementations may return interaction failures.
    fn on_progress(&self, _message: &str) -> Result<(), OAuthError> {
        Ok(())
    }
    /// Start optional concurrent pasted input; absent by default.
    fn on_manual_code_input(&self) -> Option<crate::BoxFuture<Result<String, OAuthError>>> {
        None
    }
    /// Request an optional choice; absent by default.
    fn on_select(
        &self,
        _prompt: OAuthSelectPrompt,
    ) -> Option<crate::BoxFuture<Result<Option<String>, OAuthError>>> {
        None
    }
    /// Supplied cancellation signal; absent by default.
    fn signal(&self) -> Option<&crate::Cancellation> {
        None
    }
}

/// Subscription provider operations independent of interaction and transport adapters.
pub trait OAuthProviderInterface {
    /// Provider identifier.
    fn id(&self) -> &str;
    /// Provider display name.
    fn name(&self) -> &str;
    /// Whether this provider uses a callback listener; absent by default.
    fn uses_callback_server(&self) -> Option<bool> {
        None
    }
    /// Authorize through retained interaction and the selected transport.
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        fetch: Option<crate::Fetch>,
    ) -> crate::BoxFuture<Result<OAuthCredentials, OAuthError>>;
    /// Rotate supplied credentials through the selected transport.
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        fetch: Option<crate::Fetch>,
    ) -> crate::BoxFuture<Result<OAuthCredentials, OAuthError>>;
    /// Borrow a credential field or return an independently computed request key.
    ///
    /// # Errors
    /// Implementations may reject credentials.
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<Cow<'a, str>, OAuthError>;
    /// Optionally transform model descriptors; absent by default.
    ///
    /// Borrow an input element to retain its identity, or return an owned descriptor
    /// to publish a new object.
    ///
    /// # Errors
    /// Implementations may return transformation failures.
    fn modify_models<'m>(
        &self,
        _models: &'m [crate::Model],
        _credentials: &OAuthCredentials,
    ) -> Option<Result<Vec<Cow<'m, crate::Model>>, OAuthError>> {
        None
    }
}
