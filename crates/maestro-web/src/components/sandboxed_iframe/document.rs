//! Document envelopes carrying parser-created inline programs.
use super::lowering;
use super::runtime::{self, Insertion, Program};
use maestro_app::{DiagnosticErrorInfo, Runtime};

/// Encode ampersands, less-than signs and CR for SVG foreign-content program text.
pub(super) fn encode_program(code: &str) -> String {
    code.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('\r', "&#13;")
}

/// Keep the module parser-inserted inside a non-rendering SVG the runtime removes.
fn body_program(code: &str) -> String {
    format!(
        "<svg display=\"none\" data-maestro-sandbox-program><script type=\"module\">{}</script></svg>",
        encode_program(code)
    )
}

/// Reset the font size, then place the runtime insertion.
fn head_runtime(runtime: &str) -> String {
    format!("<style>\nhtml, body {{\n\tfont-size: initial;\n}}\n</style>{runtime}")
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
/// Wrap a program scaffold in a complete document after the runtime insertion.
pub(super) fn wrap(body: &str, runtime: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n\t{runtime}\n</head>\n<body>\n\t{body}\n</body>\n</html>"
    )
}
/// Prepare authored HTML or a supplied asynchronous body with its runtime insertion.
///
/// # Errors
/// Returns a supplied provider the parser refused or a serialization failure.
pub(super) fn prepare(
    code: &str,
    id: &str,
    globals: &[(String, String)],
    runtimes: &[Runtime],
    options: super::PrepareHtmlOptions,
) -> Result<String, DiagnosticErrorInfo> {
    let compiled = (!options.is_html_artifact).then(|| lowering::program(code));
    let program = match &compiled {
        None => Program::None,
        Some(Ok(program)) => Program::Captured(program.capture().to_owned()),
        Some(Err(error)) => Program::Refused(error.to_string()),
    };
    let runtime = head_runtime(&runtime::assemble(&Insertion {
        id,
        globals,
        runtimes,
        standalone: options.is_standalone,
        program: &program,
    })?);
    Ok(match compiled {
        None => insert_runtime(code, &runtime),
        Some(compiled) => {
            let body = compiled
                .map(|program| body_program(program.code()))
                .unwrap_or_default();
            wrap(&body, &runtime)
        }
    })
}
