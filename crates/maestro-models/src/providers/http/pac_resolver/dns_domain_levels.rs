//! Literal domain separator counting.

use boa_engine::{Context, JsResult, JsValue};

/// Count literal dots after interpreter string conversion.
pub(super) fn dns_domain_levels(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let host = super::util::string(args, 0, context)?;
    let count = host.bytes().filter(|byte| *byte == b'.').count();
    Ok(JsValue::from(count))
}
