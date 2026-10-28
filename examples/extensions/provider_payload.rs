//! Opt-in logging of provider request payloads and response status and headers.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionEvent, ExtensionFuture, ExtensionResult};
use std::{fs::OpenOptions, io::Write, path::Path, rc::Rc};

/// Appends one provider observation without changing the event.
///
/// # Errors
/// Returns JSON serialization or filesystem failures to the host.
fn append_observation(event: &ExtensionEvent, path: &Path) -> ExtensionResult<()> {
    let record = match event {
        ExtensionEvent::BeforeProviderRequest(event) => {
            let payload =
                serde_json::from_str(&event.payload).map_err(|error| error.to_string())?;
            maestro_json::pretty_json(&payload).map_err(|error| error.to_string())?
        }
        ExtensionEvent::AfterProviderResponse(event) => {
            let headers = event
                .headers
                .iter()
                .map(|(name, value)| (name.clone(), value.clone().into()))
                .collect();
            let headers =
                maestro_json::compact_object(&headers).map_err(|error| error.to_string())?;
            format!("[{}] {headers}", event.status)
        }
        _ => return Ok(()),
    };
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(format!("{record}\n\n").as_bytes()))
        .map_err(|error| error.to_string())
}

/// Registers provider observations with the extension host.
///
/// Callbacks report filesystem errors to the host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let log_file = std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(".maestro/provider-payload.log");
        for name in ["before_provider_request", "after_provider_response"] {
            let log_file = log_file.clone();
            api.on(
                name,
                Rc::new(move |event, _| {
                    Box::pin(std::future::ready(
                        append_observation(event, &log_file).map(|()| None),
                    ))
                }),
            )?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
