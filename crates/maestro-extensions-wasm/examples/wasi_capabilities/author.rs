//! Standard-interface observations through ordinary authored resource callbacks.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
};
use serde_json::{Value, json};
use std::rc::Rc;

use maestro_extensions_wasm::wasi;
#[path = "network.rs"]
mod network;
use std::io::Write;

/// Installs the ordinary callback and reports its observations through the existing event bus.
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let reporter = api.clone();
        api.on(
            "resources_discover",
            Rc::new(move |event, context| {
                let reporter = reporter.clone();
                Box::pin(dispatch(event, context, reporter))
            }),
        )
    })
}

/// Runs one observation without hiding asynchronous work in registration closures.
async fn dispatch(
    event: &mut ExtensionEvent,
    context: ExtensionContext,
    reporter: ExtensionAPI,
) -> Result<Option<ExtensionEventResult>, String> {
    let result = resources(event, context).await?;
    let ExtensionEvent::ResourcesDiscover(request) = event else {
        return Err("unexpected callback".into());
    };
    let request: Value = serde_json::from_str(&request.cwd).map_err(|error| error.to_string())?;
    let value = match request["op"].as_str() {
        Some("pending") => network::connect(&request, &reporter.events()).await?,
        Some("basics") => basics(&request)?,
        _ => observe(),
    };
    reporter.events().emit("trace", value).await?;
    Ok(result)
}

/// Keeps the real context getter live beside the component's standard imports.
fn resources(
    _: &mut ExtensionEvent,
    context: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let _ = context.get_system_prompt()?;
        Ok(None)
    })
}

/// Reads only explicitly named fixture variables, never the ambient credential set.
fn observe() -> Value {
    let keys = ["MAESTRO_WASI_SENTINEL", "MAESTRO_EXTENSION_PATH"];
    let ordinary: Vec<_> = keys.iter().map(|key| std::env::var(key).ok()).collect();
    let environment = wasi::cli::environment::get_environment();
    let asynchronous: Vec<Vec<_>> = keys
        .iter()
        .map(|key| {
            environment
                .iter()
                .filter(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
                .collect()
        })
        .collect();
    json!({"p2": ordinary, "p3": asynchronous, "neighbor": std::env::var("MAESTRO_WASI_NEIGHBOR").ok()})
}

/// Standard-library operations complete while another callback awaits native I/O.
fn basics(request: &Value) -> Result<Value, String> {
    let path = request["path"].as_str().ok_or("missing path")?;
    let before = std::fs::read(path).map_err(|error| error.to_string())?;
    std::fs::write(path, b"updated\0\xff").map_err(|error| error.to_string())?;
    std::io::stdout()
        .write_all(b"MIXED\0\xff\x1b[32m")
        .map_err(|error| error.to_string())?;
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    Ok(
        json!({"before":before,"after":std::fs::read(path).map_err(|error| error.to_string())?,"env":observe()}),
    )
}
