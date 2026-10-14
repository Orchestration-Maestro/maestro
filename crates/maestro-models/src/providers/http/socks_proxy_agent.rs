//! Selected SOCKS proxy URL and destination lookup policy.

use std::io;

/// Parsed connection policy for one selected SOCKS URL.
#[derive(Clone)]
pub(super) struct SocksProxyAgent {
    /// Resolve destination names locally rather than at the proxy.
    pub(super) lookup: bool,
    /// Proxy URL host; IPv6 brackets are removed only at socket connection.
    pub(super) host: String,
    /// Effective proxy port.
    pub(super) port: u16,
    /// Once-decoded username.
    pub(super) user: String,
    /// Once-decoded password.
    pub(super) password: String,
}

impl SocksProxyAgent {
    /// Parse only the selected proxy, retaining empty credential fields.
    pub(super) fn new(url: &url::Url) -> io::Result<Self> {
        let lookup = match url.scheme() {
            "socks5" => true,
            "socks" | "socks5h" => false,
            _ => return Err(io::Error::other("Unsupported SOCKS protocol")),
        };
        let host = url.host_str().unwrap_or_default().to_owned();
        Ok(Self {
            lookup,
            host,
            port: url.port().filter(|port| *port != 0).unwrap_or(1080),
            user: decode(url.username())?,
            password: decode(url.password().unwrap_or_default())?,
        })
    }

    /// Resolve the destination before timing proxy connection and negotiation.
    pub(super) async fn connect(&self, uri: &hyper::Uri) -> io::Result<tokio::net::TcpStream> {
        let host = uri
            .host()
            .ok_or_else(|| io::Error::other("Missing destination host"))?;
        let host = host
            .strip_prefix('[')
            .and_then(|host| host.strip_suffix(']'))
            .unwrap_or(host);
        let port = uri
            .port_u16()
            .unwrap_or(if uri.scheme_str() == Some("https") {
                443
            } else {
                80
            });
        let target = if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            tokio_socks::TargetAddr::Ip((ip, port).into())
        } else if self.lookup {
            let address = tokio::net::lookup_host((host, port))
                .await?
                .next()
                .ok_or_else(|| io::Error::other("No destination address"))?;
            tokio_socks::TargetAddr::Ip(address)
        } else {
            tokio_socks::TargetAddr::Domain(host.into(), port)
        };
        tokio::time::timeout(std::time::Duration::from_secs(30), self.tunnel(target))
            .await
            .map_err(|_| io::Error::other("SOCKS handshake deadline elapsed"))?
    }

    /// Connect and negotiate inside the SOCKS handshake deadline.
    async fn tunnel(
        &self,
        target: tokio_socks::TargetAddr<'_>,
    ) -> io::Result<tokio::net::TcpStream> {
        let host = self
            .host
            .strip_prefix('[')
            .and_then(|host| host.strip_suffix(']'))
            .unwrap_or(&self.host);
        let socket = tokio::net::TcpStream::connect((host, self.port)).await?;
        let stream = if self.user.is_empty() && self.password.is_empty() {
            tokio_socks::tcp::Socks5Stream::connect_with_socket(socket, target).await
        } else if (1..=255).contains(&self.user.len()) && (1..=255).contains(&self.password.len()) {
            tokio_socks::tcp::Socks5Stream::connect_with_password_and_socket(
                socket,
                target,
                &self.user,
                &self.password,
            )
            .await
        } else {
            return super::socks_client::connect(socket, target, &self.user, &self.password).await;
        };
        stream
            .map(tokio_socks::tcp::Socks5Stream::into_inner)
            .map_err(io::Error::other)
    }
}

/// Decode one URI component strictly, without form-style plus replacement.
fn decode(value: &str) -> io::Result<String> {
    let bytes = value.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'%'
            && !bytes
                .get(index + 1..index + 3)
                .is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit))
        {
            return Err(io::Error::other("Invalid proxy credential escape"));
        }
    }
    percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .map(std::borrow::Cow::into_owned)
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests;
