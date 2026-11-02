//! Ordered native interface fallback for the local IPv4 address.

use std::net::{IpAddr, Ipv4Addr};

/// Keep interface first-appearance order, then its first eligible IPv4, including link-local.
pub(super) fn select<'a>(addresses: impl IntoIterator<Item = (&'a str, IpAddr)>) -> Ipv4Addr {
    let mut interfaces = indexmap::IndexMap::<_, Vec<_>>::new();
    for (name, address) in addresses {
        interfaces.entry(name).or_default().push(address);
    }
    interfaces
        .into_values()
        .flatten()
        .find_map(|address| match address {
            IpAddr::V4(ip) if !ip.is_loopback() => Some(ip),
            _ => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

/// Read interface observations only after the UDP route lookup fails.
pub(super) fn address() -> std::io::Result<Ipv4Addr> {
    Ok(select(if_addrs::get_if_addrs()?.iter().map(|interface| {
        (interface.name.as_str(), interface.ip())
    })))
}
