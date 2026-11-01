//! Standard-interface observations through ordinary authored resource callbacks.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    tools::path_utils::guest_path,
};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

use maestro_extensions_wasm::wasi;
#[path = "filesystem.rs"]
mod filesystem;
#[path = "network.rs"]
mod network;
#[path = "stdio.rs"]
mod stdio;
use std::io::Write;

/// Installs the ordinary callback and reports its observations through the existing event bus.
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        if let Ok(request) = std::env::var("MAESTRO_WASI_STARTUP_OUTPUT") {
            let request: Value =
                serde_json::from_str(&request).map_err(|error| error.to_string())?;
            stdio::output(&request).await?;
        }
        let reporter = api.clone();
        let calls = Rc::new(Cell::new(0));
        api.on(
            "resources_discover",
            Rc::new(move |event, context| {
                let reporter = reporter.clone();
                calls.set(calls.get() + 1);
                Box::pin(dispatch(event, context, reporter, calls.get()))
            }),
        )
    })
}

/// Runs one observation without hiding asynchronous work in registration closures.
async fn dispatch(
    event: &mut ExtensionEvent,
    context: ExtensionContext,
    reporter: ExtensionAPI,
    calls: u32,
) -> Result<Option<ExtensionEventResult>, String> {
    let result = resources(event, context).await?;
    let ExtensionEvent::ResourcesDiscover(request) = event else {
        return Err("unexpected callback".into());
    };
    let request: Value = serde_json::from_str(&request.cwd).map_err(|error| error.to_string())?;
    let mut value = match request["op"].as_str() {
        Some("pending" | "connect") => network::connect(&request, &reporter.events()).await?,
        Some("basics") => basics(&request)?,
        Some("close-pending") => network::close_pending(&request, &reporter.events()).await?,
        Some("listen") => network::listen(&request, &reporter.events()).await?,
        Some("file") => {
            let path = request["path"].as_str().ok_or("missing path")?;
            filesystem::operate(&request, &guest_path(&reporter, path)?).await?
        }
        Some("link") => {
            let target = request["target"].as_str().ok_or("missing target")?;
            let link = request["link"].as_str().ok_or("missing link")?;
            filesystem::link(
                &request,
                &guest_path(&reporter, target)?,
                &guest_path(&reporter, link)?,
            )
            .await?
        }
        Some("cwd") => filesystem::cwd(),
        Some("stdio") => {
            stdio::inherited(request["generation"].as_str().ok_or("missing generation")?).await?
        }
        Some("output") => stdio::output(&request).await?,
        _ => observe(),
    };
    value["calls"] = json!(calls);
    reporter.events().emit("trace", value).await?;
    Ok(result)
}

/// Keeps the real context getter live beside the component's standard imports.
fn resources(
    _: &mut ExtensionEvent,
    context: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        if context.get_system_prompt()? != "wasi context" {
            return Err("wrong native context".into());
        }
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
