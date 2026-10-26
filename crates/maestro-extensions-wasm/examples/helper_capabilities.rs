//! An author fixture that observes helpers through the installed host.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{CommandOptions, ExtensionAPI, ExtensionFuture, ExtensionResult};
use serde_json::Value;
use std::rc::Rc;
#[path = "helper_capabilities/frontmatter.rs"]
mod frontmatter;
#[path = "helper_capabilities/fuzzy.rs"]
mod fuzzy;
#[path = "helper_capabilities/keys.rs"]
mod keys;
#[path = "helper_capabilities/text.rs"]
mod text;

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
    if request["operation"]
        .as_str()
        .is_some_and(|op| op.starts_with("k."))
    {
        return keys::observe(api, request);
    }
    if request["operation"]
        .as_str()
        .is_some_and(|op| op.starts_with("f."))
    {
        return fuzzy::observe(api, request);
    }
    if request["operation"]
        .as_str()
        .is_some_and(|op| op.starts_with("fm."))
    {
        return frontmatter::observe(api, request);
    }
    let args = &request["args"];
    text::observe(api, request["operation"].as_str().unwrap_or_default(), args)
}

/// Reads a supplied cell count without narrowing it silently.
fn number(value: &Value) -> ExtensionResult<usize> {
    usize::try_from(value.as_u64().ok_or("missing cell count")?).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
