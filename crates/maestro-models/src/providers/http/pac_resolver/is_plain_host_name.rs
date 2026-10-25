//! Hostnames without a domain separator.

use boa_engine::{Context, JsResult, JsValue};

/// Test for literal dots after interpreter string conversion.
pub(super) fn is_plain_host_name(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    Ok((!super::util::string(args, 0, context)?.contains('.')).into())
}
