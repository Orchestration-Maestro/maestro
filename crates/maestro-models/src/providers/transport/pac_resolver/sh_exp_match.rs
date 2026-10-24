//! Shell wildcard expansion through the installed regular-expression engine.

use boa_engine::{Context, JsNativeError, JsResult, JsValue};

/// Keep authored regular-expression forms while expanding the three shell metacharacters.
pub(super) fn sh_exp_match(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let pattern = super::util::string(args, 1, context)?
        .replace('.', "\\.")
        .replace('?', ".")
        .replace('*', ".*");
    let pattern = regress::Regex::new(&format!("^{pattern}$"))
        .map_err(|error| JsNativeError::syntax().with_message(error.to_string()))?;
    let text = super::util::string(args, 0, context)?;
    Ok(pattern.find(&text).is_some().into())
}
