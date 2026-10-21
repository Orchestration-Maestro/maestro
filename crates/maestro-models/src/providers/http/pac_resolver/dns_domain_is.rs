//! Case-sensitive domain suffix comparison.

use boa_engine::{Context, JsResult, JsValue};

/// Compare the complete host for an empty suffix, otherwise its trailing domain.
pub(super) fn dns_domain_is(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let host = super::util::string(args, 0, context)?;
    let domain = super::util::string(args, 1, context)?;
    Ok((if domain.is_empty() {
        host.is_empty()
    } else {
        host.ends_with(&domain)
    })
    .into())
}
