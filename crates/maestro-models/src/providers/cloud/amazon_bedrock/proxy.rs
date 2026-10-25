//! The process environment as one request saw it, and the proxy it names for a destination.

use std::collections::HashMap;

use url::Url;

use crate::arguments::json_parse::whitespace;

/// A snapshot of the process environment taken once per request.
pub(super) struct Environment(pub(super) HashMap<String, String>);

impl Environment {
    /// Copy the current environment; names or values that are not Unicode are left out.
    pub(super) fn capture() -> Self {
        Self(
            std::env::vars_os()
                .filter_map(|(name, value)| {
                    Some((name.into_string().ok()?, value.into_string().ok()?))
                })
                .collect(),
        )
    }

    /// The value of exactly `name` when it is not empty.
    pub(super) fn get(&self, name: &str) -> Option<&str> {
        self.0
            .get(name)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    /// The nonempty value of the lowercase spelling of `name`, otherwise of its uppercase one.
    fn variable(&self, name: &str) -> Option<&str> {
        self.get(&name.to_lowercase())
            .or_else(|| self.get(&name.to_uppercase()))
    }

    /// Whether any of the six variables that turn proxying on is set.
    pub(super) fn names_proxy(&self) -> bool {
        ["HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"]
            .into_iter()
            .any(|name| self.variable(name).is_some())
    }

    /// The proxy for `destination`, or none when the destination is excluded or no variable
    /// names one. A proxy written without a scheme takes the destination's.
    ///
    /// # Errors
    /// Fails when the selected proxy is not a URL or uses an unsupported scheme.
    pub(super) fn proxy_for(&self, destination: &Url) -> Result<Option<Url>, String> {
        let Some(host) = destination.host_str() else {
            return Ok(None);
        };
        let port = destination.port_or_known_default().map_or(0, u64::from);
        if self.bypasses(host, port) {
            return Ok(None);
        }
        let scheme = destination.scheme();
        let proxy = [
            format!("npm_config_{scheme}_proxy"),
            format!("{scheme}_proxy"),
            "npm_config_proxy".to_owned(),
            "all_proxy".to_owned(),
        ]
        .iter()
        .find_map(|name| self.variable(name));
        let Some(proxy) = proxy else {
            return Ok(None);
        };
        let parsed = if proxy.contains("://") || proxy.starts_with("pac+data:") {
            Url::parse(proxy)
        } else {
            Url::parse(&format!("{scheme}://{proxy}"))
        }
        .map_err(|error| format!("Invalid proxy URL {proxy}: {error}"))?;
        if let Some(source) = proxy.strip_prefix("pac+") {
            Url::parse(source).map_err(|error| format!("Invalid proxy URL {proxy}: {error}"))?;
        }
        match parsed.scheme() {
            "http" | "https" | "socks" | "socks4" | "socks4a" | "socks5" | "socks5h"
            | "pac+data" | "pac+file" | "pac+ftp" | "pac+http" | "pac+https" => Ok(Some(parsed)),
            _ => Err(format!("Unsupported protocol for proxy URL: {proxy}")),
        }
    }

    /// Whether the exclusion list leaves out a destination: `*` leaves out all, `host` only
    /// that host, `.suffix` or `*suffix` hosts ending so, and a `:port` limits an entry.
    fn bypasses(&self, host: &str, port: u64) -> bool {
        let Some(list) = self
            .variable("npm_config_no_proxy")
            .or_else(|| self.variable("no_proxy"))
        else {
            return false;
        };
        let list = list.to_lowercase();
        list == "*"
            || list
                .split(|c| c == ',' || whitespace(c))
                .filter(|entry| !entry.is_empty())
                .any(|entry| excludes(entry, host, port))
    }
}

/// Whether one exclusion entry leaves out `host` on `port`.
fn excludes(entry: &str, host: &str, port: u64) -> bool {
    let (name, limit) = match entry.rsplit_once(':') {
        Some((name, digits))
            if !name.is_empty()
                && !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit()) =>
        {
            (name, digits.parse().unwrap_or(u64::MAX))
        }
        _ => (entry, 0),
    };
    if limit != 0 && limit != port {
        return false;
    }
    match name.strip_prefix('*') {
        Some(suffix) => host.ends_with(suffix),
        None if name.starts_with('.') => host.ends_with(name),
        None => host == name,
    }
}
