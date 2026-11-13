//! IPv4 network membership with legacy network and netmask spellings.

use boa_engine::{Context, JsResult, JsValue};

/// Resolve the host once before comparing its masked IPv4 address.
pub(super) fn is_in_net(args: &[JsValue], context: &mut Context) -> JsValue {
    check(args, context).unwrap_or(JsValue::from(false))
}

/// Failed lookups, malformed networks and argument conversions all mean nonmembership.
fn check(args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let host = super::util::string(args, 0, context)?;
    let Some(ip) = super::util::dns_lookup(&host, context) else {
        return Ok(false.into());
    };
    let Some(pattern) = args.get(1).and_then(JsValue::as_string) else {
        return Ok(false.into());
    };
    let pattern = pattern.to_std_string_escaped();
    let mask = args.get(2).filter(|value| value.to_boolean());
    let mask = mask.map(|value| value.to_string(context)).transpose()?;
    let mask = mask.map(|value| value.to_std_string_escaped());
    Ok(membership(u32::from(ip), &pattern, mask.as_deref()).into())
}

/// Accept contiguous dotted masks or decimal-prefix lengths, including dotted zero.
fn membership(ip: u32, pattern: &str, mask: Option<&str>) -> bool {
    let mut parts = pattern.split('/');
    let network = parts.next().unwrap_or_default();
    let mask = mask
        .or_else(|| parts.next())
        .filter(|mask| !mask.is_empty())
        .unwrap_or("32");
    let mask = if mask.contains('.') {
        address(mask).filter(|value| value.leading_ones() + value.trailing_zeros() == 32)
    } else {
        super::util::decimal_prefix(mask)
            .filter(|bits| *bits <= 32)
            .map(|bits| {
                if bits <= 0 {
                    0
                } else {
                    u32::MAX << (32 - bits.cast_unsigned())
                }
            })
    };
    match (address(network), mask) {
        (Some(network), Some(mask)) => ip & mask == network & mask,
        _ => false,
    }
}

/// Parse the accepted one-to-four component decimal, octal and hexadecimal IPv4 forms.
fn address(text: &str) -> Option<u32> {
    if text.is_empty()
        || text.ends_with('.')
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || matches!(byte, b'x' | b'X' | b'.'))
    {
        return None;
    }
    match url::Host::parse(text).ok()? {
        url::Host::Ipv4(ip) => Some(u32::from(ip)),
        _ => None,
    }
}
