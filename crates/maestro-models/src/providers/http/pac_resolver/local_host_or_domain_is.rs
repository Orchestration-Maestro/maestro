//! Complete-label prefix comparison for local hostnames.

use boa_engine::{Context, JsNativeError, JsResult, JsValue};

/// Compare host labels without treating a partial label as a prefix.
pub(super) fn local_host_or_domain_is(
    _: &JsValue,
    args: &[JsValue],
    _: &mut Context,
) -> JsResult<JsValue> {
    let host = args
        .first()
        .and_then(JsValue::as_string)
        .ok_or_else(|| JsNativeError::typ().with_message("Host must be a string"))?
        .to_std_string_escaped();
    let domain = args
        .get(1)
        .and_then(JsValue::as_string)
        .ok_or_else(|| JsNativeError::typ().with_message("Domain must be a string"))?
        .to_std_string_escaped();
    let mut labels = domain.split('.');
    Ok(host
        .split('.')
        .all(|label| labels.next() == Some(label))
        .into())
}
