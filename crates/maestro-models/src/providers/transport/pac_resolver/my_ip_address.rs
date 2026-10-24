//! Reachability-based local address observation with native interface fallback.

use std::net::{Ipv4Addr, SocketAddr};

use boa_engine::{Context, JsNativeError, JsResult, JsValue, js_string};

/// Probe the standard endpoint on the supplied runtime while the interpreter waits.
pub(super) fn my_ip_address(
    _: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let host = context
        .get_data::<super::util::Host>()
        .ok_or_else(|| JsNativeError::typ().with_message("Missing native runtime"))?;
    let ip = host
        .0
        .block_on(address(SocketAddr::from(([8, 8, 8, 8], 53))))
        .map_err(|error| JsNativeError::typ().with_message(error.to_string()))?;
    Ok(js_string!(ip.to_string()).into())
}

/// Close the probe socket before returning its local IPv4 or reading interfaces.
pub(super) async fn address(endpoint: SocketAddr) -> std::io::Result<Ipv4Addr> {
    match tokio::net::TcpStream::connect(endpoint).await {
        Ok(socket) => match socket.local_addr()?.ip() {
            std::net::IpAddr::V4(ip) => Ok(ip),
            std::net::IpAddr::V6(_) => Err(std::io::Error::other("Expected an IPv4 address")),
        },
        Err(_) => super::ip::address(),
    }
}
