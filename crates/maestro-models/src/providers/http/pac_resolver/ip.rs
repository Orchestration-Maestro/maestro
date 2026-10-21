//! Ordered native interface fallback for the local IPv4 address.

use std::net::{IpAddr, Ipv4Addr};

/// Keep the first eligible IPv4, including link-local addresses.
pub(super) fn select(addresses: impl IntoIterator<Item = IpAddr>) -> Ipv4Addr {
    addresses
        .into_iter()
        .find_map(|address| match address {
            IpAddr::V4(ip) if !ip.is_loopback() => Some(ip),
            _ => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

/// Read interface observations only after the reachability probe fails.
pub(super) fn address() -> std::io::Result<Ipv4Addr> {
    Ok(select(
        if_addrs::get_if_addrs()?
            .iter()
            .map(if_addrs::Interface::ip),
    ))
}
