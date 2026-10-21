//! Selected SOCKS proxy URL and destination lookup policy.

use std::io;

/// Protocol selected by the proxy URL.
#[derive(Clone, Copy)]
enum SocksVersion {
    /// SOCKS4 with a NUL-terminated identity.
    V4,
    /// SOCKS5 with negotiated authentication.
    V5,
}

/// Parsed connection policy for one selected SOCKS URL.
#[derive(Clone)]
pub(super) struct SocksProxyAgent {
    /// Selected wire protocol.
    version: SocksVersion,
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
        let (lookup, version) = match url.scheme() {
            "socks4" => (true, SocksVersion::V4),
            "socks4a" => (false, SocksVersion::V4),
            "socks5" => (true, SocksVersion::V5),
            "socks" | "socks5h" => (false, SocksVersion::V5),
            _ => return Err(io::Error::other("Unsupported SOCKS protocol")),
        };
        let host = url.host_str().unwrap_or_default().to_owned();
        Ok(Self {
            version,
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
        let host = socket_host(host);
        let port = uri
            .port_u16()
            .unwrap_or(if uri.scheme_str() == Some("https") {
                443
            } else {
                80
            });
        let target = if matches!(self.version, SocksVersion::V4)
            && host.parse::<std::net::Ipv6Addr>().is_ok()
        {
            tokio_socks::TargetAddr::Domain(host.into(), port)
        } else if let Ok(ip) = host.parse::<std::net::IpAddr>() {
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
        let host = socket_host(&self.host);
        let socket = tokio::net::TcpStream::connect((host, self.port)).await?;
        if matches!(self.version, SocksVersion::V4) {
            return self.tunnel4(socket, target).await;
        }
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

    /// Delegate representable SOCKS4 frames and correct the remaining inputs once.
    async fn tunnel4(
        &self,
        socket: tokio::net::TcpStream,
        target: tokio_socks::TargetAddr<'_>,
    ) -> io::Result<tokio::net::TcpStream> {
        let native = self.user.len() <= 255
            && match &target {
                tokio_socks::TargetAddr::Ip(std::net::SocketAddr::V4(_)) => true,
                tokio_socks::TargetAddr::Ip(std::net::SocketAddr::V6(_)) => false,
                tokio_socks::TargetAddr::Domain(host, _) => self.user.len() + host.len() <= 503,
            };
        if !native {
            return super::socks_client::connect4(socket, target, &self.user).await;
        }
        let stream = if self.user.is_empty() {
            tokio_socks::tcp::Socks4Stream::connect_with_socket(socket, target).await
        } else {
            tokio_socks::tcp::Socks4Stream::connect_with_userid_and_socket(
                socket, target, &self.user,
            )
            .await
        };
        stream
            .map(tokio_socks::tcp::Socks4Stream::into_inner)
            .map_err(io::Error::other)
    }
}

/// Remove URL brackets only for socket addressing.
fn socket_host(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
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
