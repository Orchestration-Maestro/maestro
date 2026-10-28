//! Domain rules shared by both sandbox proxies.

use serde::{Deserialize, Serialize};

/// Domain admission rules; denied entries take precedence over allowed entries.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetworkConfig {
    /// Exact host names or wildcard subdomain patterns admitted by the proxy.
    pub(crate) allowed_domains: Vec<String>,
    /// Exact host names or wildcard subdomain patterns refused before admission.
    pub(crate) denied_domains: Vec<String>,
    /// Specific Unix socket paths admitted by the macOS policy.
    #[serde(default)]
    pub(crate) allow_unix_sockets: Vec<String>,
    /// Whether every Unix socket is explicitly admitted.
    #[serde(default)]
    pub(crate) allow_all_unix_sockets: bool,
    /// Whether macOS loopback listeners are explicitly admitted.
    #[serde(default)]
    pub(crate) allow_local_binding: bool,
    /// Explicit user-owned HTTP proxy endpoint instead of the internal server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) http_proxy_port: Option<u16>,
    /// Explicit user-owned SOCKS endpoint instead of the internal server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) socks_proxy_port: Option<u16>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            allowed_domains: [
                "npmjs.org",
                "*.npmjs.org",
                "registry.npmjs.org",
                "registry.yarnpkg.com",
                "pypi.org",
                "*.pypi.org",
                "github.com",
                "*.github.com",
                "api.github.com",
                "raw.githubusercontent.com",
            ]
            .map(str::to_owned)
            .into(),
            denied_domains: Vec::new(),
            allow_unix_sockets: Vec::new(),
            allow_all_unix_sockets: false,
            allow_local_binding: false,
            http_proxy_port: None,
            socks_proxy_port: None,
        }
    }
}

impl NetworkConfig {
    /// Reports whether the supplied host is admitted by these domain rules.
    pub(crate) fn allows(&self, hostname: &str) -> bool {
        let hostname = hostname.to_lowercase();
        !self
            .denied_domains
            .iter()
            .any(|rule| matches(rule, &hostname))
            && self
                .allowed_domains
                .iter()
                .any(|rule| matches(rule, &hostname))
    }
}

/// Matches an exact host or a wildcard that admits subdomains, never its base domain.
fn matches(rule: &str, hostname: &str) -> bool {
    let rule = rule.to_lowercase();
    match rule.strip_prefix("*.") {
        Some(suffix) => hostname.ends_with(&format!(".{suffix}")),
        None => rule == hostname,
    }
}
