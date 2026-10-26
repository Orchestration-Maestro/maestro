//! An author fixture that observes helpers through the installed host.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{CommandOptions, ExtensionAPI, ExtensionFuture, ExtensionResult};
use serde_json::{Value, json};
use std::rc::Rc;

/// Registers one observation command without choosing a native adapter.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let helpers = api.clone();
        api.register_command(
            "helpers",
            CommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(move |request, _| {
                    let api = helpers.clone();
                    Box::pin(async move {
                        let request: Value =
                            serde_json::from_str(&request).map_err(|e| e.to_string())?;
                        let value = observe(&api, &request)?;
                        Err(value.to_string())
                    })
                }),
            },
        )
    })
}

/// Returns the complete observation as the command's catchable result.
fn observe(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    let args = &request["args"];
    let text = args[0].as_str().ok_or("missing text")?;
    match request["operation"].as_str() {
        Some("u.visibleWidth") => Ok(json!(maestro_extensions_wasm::utils::visible_width(
            api, text
        )?)),
        Some("u.normalizeTerminalOutput") => Ok(json!(
            maestro_extensions_wasm::utils::normalize_terminal_output(api, text)?
        )),
        Some("u.extractAnsiCode") => {
            let pos = usize::try_from(args[1].as_u64().ok_or("missing position")?)
                .map_err(|e| e.to_string())?;
            Ok(json!(maestro_extensions_wasm::utils::extract_ansi_code(
                api, text, pos
            )?))
        }
        Some("u.isWhitespaceChar") => Ok(json!(
            maestro_extensions_wasm::utils::is_whitespace_char(api, text)?
        )),
        Some("u.isPunctuationChar") => Ok(json!(
            maestro_extensions_wasm::utils::is_punctuation_char(api, text)?
        )),
        Some("composition") => Ok(json!({
            "width": maestro_extensions_wasm::utils::visible_width(api, text)?,
            "normalized": maestro_extensions_wasm::utils::normalize_terminal_output(api, "ำຳ")?,
            "ansi": maestro_extensions_wasm::utils::extract_ansi_code(api, "é\u{1b}[0m", 2)?,
            "whitespace": maestro_extensions_wasm::utils::is_whitespace_char(api, "\u{feff}")?,
            "punctuation": maestro_extensions_wasm::utils::is_punctuation_char(api, "a?")?,
        })),
        _ => Err("unknown helper observation".into()),
    }
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
