//! Route-selected local IPv4 observation with native interface fallback.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use boa_engine::{Context, JsNativeError, JsResult, JsValue, js_string};

/// Read the local IPv4 without sending a packet to the standard endpoint.
pub(super) fn my_ip_address(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let ip = address(SocketAddr::from(([8, 8, 8, 8], 53)))
        .map_err(|error| JsNativeError::typ().with_message(error.to_string()))?;
    Ok(js_string!(ip.to_string()).into())
}

/// Close the route-lookup socket before returning its IPv4 or reading interfaces.
pub(super) fn address(endpoint: SocketAddr) -> std::io::Result<Ipv4Addr> {
    let local = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).and_then(|socket| {
        socket.connect(endpoint)?;
        socket.local_addr()
    });
    match local {
        Ok(SocketAddr::V4(address)) => Ok(*address.ip()),
        Ok(SocketAddr::V6(_)) | Err(_) => super::ip::address(),
    }
}
