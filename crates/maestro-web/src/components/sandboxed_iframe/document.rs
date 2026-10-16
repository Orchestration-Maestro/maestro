//! Document envelopes carrying parser-created inline programs.

/// Encode ampersands, less-than signs and CR for SVG foreign-content program text.
fn encode_program(code: &str) -> String {
    code.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('\r', "&#13;")
}

/// Move the admitted classic script out of an inert template and remove it after execution.
pub(super) fn head_program(code: &str) -> String {
    format!(
        "<style>\nhtml, body {{\n\tfont-size: initial;\n}}\n</style>\n<template><svg><script>{}</script></svg></template><script>{{const t=document.currentScript.previousElementSibling,s=t.content.firstChild.firstChild;t.replaceWith(s);s.remove()}}</script>",
        encode_program(code)
    )
}

/// Keep the module parser-inserted while removing its non-rendering SVG scaffold.
fn body_program(code: &str) -> String {
    format!(
        "<svg display=\"none\"><script type=\"module\">{}</script></svg><script>document.currentScript.previousElementSibling.remove()</script>",
        encode_program(code)
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
    let program = [
        include_str!("repl-prefix.js"),
        "\n\t\t\t\t\t",
        code,
        include_str!("repl-suffix.js"),
    ]
    .concat();
    let body = body_program(&program);
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n\t{runtime}\n</head>\n<body>\n\t{body}\n</body>\n</html>"
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
