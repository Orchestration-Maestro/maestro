//! Document envelopes with separate program resources; authored HTML is otherwise unchanged.
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Deliver UTF-8 source as a separate resource without HTML interpretation.
pub(super) fn script_resource(code: &str) -> String {
    format!(
        "data:text/javascript;charset=utf-8;base64,{}",
        STANDARD.encode(code)
    )
}

/// Insert after the first head prefix, then html prefix, or prepend.
pub(super) fn insert_runtime(code: &str, runtime: &str) -> String {
    let lower = code.to_ascii_lowercase();
    let index = ["<head", "<html"]
        .into_iter()
        .find_map(|tag| {
            let start = lower.find(tag)?;
            lower[start..].find('>').map(|end| start + end + 1)
        })
        .unwrap_or(0);
    format!("{}{runtime}{}", &code[..index], &code[index..])
}
/// Wrap JavaScript in the asynchronous completion envelope.
pub(super) fn wrap(code: &str, runtime: &str) -> String {
    let program = include_str!("repl.js").replacen("__USER_CODE__", code, 1);
    include_str!("repl.html")
        .replacen("__RUNTIME__", runtime, 1)
        .replacen("__PROGRAM__", &script_resource(&program), 1)
}
/// Select authored HTML injection independently of standalone runtime selection.
pub(super) fn prepare(code: &str, runtime: &str, options: super::PrepareHtmlOptions) -> String {
    if options.is_html_artifact {
        insert_runtime(code, runtime)
    } else {
        wrap(code, runtime)
    }
}
