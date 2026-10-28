//! Domain rules shared by both sandbox proxies.

use serde::{Deserialize, Serialize};

/// Domain admission rules; denied entries take precedence over allowed entries.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetworkConfig {
    /// Exact host names or wildcard subdomain patterns admitted by the proxy.
    pub(crate) allowed_domains: Vec<String>,
    /// Exact host names or wildcard subdomain patterns refused before admission.
    pub(crate) denied_domains: Vec<String>,
}

impl NetworkConfig {
    /// Reports whether the supplied host is admitted by these domain rules.
    pub(crate) fn allows(&self, hostname: &str) -> bool {
        self.allowed_domains.iter().any(|domain| domain == hostname)
    }
}
