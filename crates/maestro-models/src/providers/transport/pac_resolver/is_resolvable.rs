//! Native IPv4 lookup success exposed without formatting an address.

use boa_engine::{Context, JsResult, JsValue};

/// Report whether the shared DNS operation finds an IPv4 address.
pub(super) fn is_resolvable(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let host = super::util::string(args, 0, context)?;
    Ok(super::util::dns_lookup(&host, context).is_some().into())
}
