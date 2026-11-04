//! Load-time input transport through the original native extension API.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, FlagOptions, FlagType, FlagValue};

/// Offers the host-selected JSON input only when its controlling flag is present.
pub(super) fn publish(api: &ExtensionAPI) -> Result<(), String> {
    api.register_flag(
        "load-time-inputs",
        FlagOptions {
            description: None,
            type_: FlagType::String,
            default: None,
        },
    )?;
    if let Some(FlagValue::String(payload)) = api.get_flag("load-time-inputs")? {
        api.contribute_load_time(
            serde_json::from_str(&payload).map_err(|error| error.to_string())?,
        )?;
    }
    Ok(())
}
