//! IPv4 resolution exposed to authored scripts.

use boa_engine::{Context, JsResult, JsValue, js_string};

/// Return the native lookup's IPv4 text or null on failure, never an IPv6 replacement.
pub(super) fn dns_resolve(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let host = super::util::string(args, 0, context)?;
    Ok(super::util::dns_lookup(&host, context)
        .map_or(JsValue::null(), |ip| js_string!(ip.to_string()).into()))
}
