//! Case-sensitive candidate words and interpreter-compatible whitespace.

use super::Failure;
use crate::providers::http::socks_proxy_agent::SocksProxyAgent;

/// A recognized connection policy, constructed only when its turn is reached.
pub(super) enum Candidate {
    /// Destination socket without a proxy.
    Direct,
    /// HTTP forwarding or a CONNECT tunnel.
    Http(url::Url),
    /// Existing SOCKS negotiation with remote destination DNS.
    Socks(SocksProxyAgent),
    /// Unknown directives advance like failed connection attempts.
    Unknown,
}

/// Split once, retaining order and duplicate directives.
pub(super) fn parse(result: &str) -> Vec<String> {
    result
        .trim_matches(maestro_text::is_whitespace_scalar)
        .split(';')
        .map(|part| part.trim_matches(maestro_text::is_whitespace_scalar))
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Recognized URL constructor errors are terminal, not failed socket attempts.
pub(super) fn candidate(text: &str) -> Result<Candidate, Failure> {
    let mut parts = text
        .split(maestro_text::is_whitespace_scalar)
        .filter(|part| !part.is_empty());
    let kind = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("undefined");
    let scheme = match kind {
        "DIRECT" => return Ok(Candidate::Direct),
        "PROXY" | "HTTP" => "http",
        "HTTPS" => "https",
        "SOCKS" | "SOCKS5" => "socks",
        "SOCKS4" => "socks4a",
        _ => return Ok(Candidate::Unknown),
    };
    let url = url::Url::parse(&format!("{scheme}://{target}"))?;
    Ok(if scheme.starts_with("socks") {
        Candidate::Socks(SocksProxyAgent::new(&url)?)
    } else {
        Candidate::Http(url)
    })
}
