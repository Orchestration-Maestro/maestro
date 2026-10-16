//! Lexical document envelopes; authored HTML is otherwise unchanged.
/// Protect inline programs from HTML's case-insensitive script terminator.
pub(super) fn escape_script(code: &str) -> String {
    let lower = code.to_ascii_lowercase();
    let mut result = String::with_capacity(code.len());
    let mut offset = 0;
    for (start, matched) in lower.match_indices("</script") {
        result.push_str(&code[offset..start]);
        result.push_str("<\\");
        result.push_str(&code[start + 1..start + matched.len()]);
        offset = start + matched.len();
    }
    result.push_str(&code[offset..]);
    result
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
    let (before, after) = include_str!("repl.html")
        .split_once("__USER_CODE__")
        .unwrap_or_default();
    format!(
        "{}{}{after}",
        before.replacen("__RUNTIME__", runtime, 1),
        escape_script(code)
    )
}
/// Select authored HTML injection independently of standalone runtime selection.
pub(super) fn prepare(code: &str, runtime: &str, options: super::PrepareHtmlOptions) -> String {
    if options.is_html_artifact {
        insert_runtime(code, runtime)
    } else {
        wrap(code, runtime)
    }
}
