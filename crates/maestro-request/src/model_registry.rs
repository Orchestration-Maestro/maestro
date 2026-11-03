//! Resolved model request authentication records.
use indexmap::IndexMap;

/// Authentication resolution, distinct from a failed host call.
pub enum ResolvedRequestAuth {
    /// Resolved credentials.
    Ok {
        /// Optional key, retaining present empty strings.
        api_key: Option<String>,
        /// Endpoint materialized from provider account configuration.
        base_url: Option<String>,
        /// Optional headers in supplied order, including present empty maps.
        headers: Option<IndexMap<String, String>>,
    },
    /// Structured authentication denial.
    Error {
        /// The supplied denial message.
        error: String,
    },
}
