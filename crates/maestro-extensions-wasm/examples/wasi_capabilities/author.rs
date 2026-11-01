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
        if let Ok(endpoint) = std::env::var("MAESTRO_WASI_ROOT_OUTPUT") {
            return root_output(&api, endpoint);
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

/// Registers byte output and native execution through the executable's ordinary loader.
fn root_output(api: &ExtensionAPI, endpoint: String) -> Result<(), String> {
    use maestro_extensions_wasm::CommandOptions;
    api.on(
        "session_start",
        Rc::new(|_, context| {
            Box::pin(async move {
                context.ui()?.set_title("ROOT-STDOUT-READY")?;
                Ok(None)
            })
        }),
    )?;
    let native = api.native_io();
    api.register_command(
        "stdout-probe",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |generation, context| {
                let (native, endpoint) = (native.clone(), endpoint.clone());
                Box::pin(async move {
                    if generation == "reload" {
                        return context.reload().await;
                    }
                    let mut bytes = b"\x1b[36m".to_vec();
                    bytes.extend(b"guest\0\xff".repeat(300_000));
                    bytes.extend_from_slice(b"\x1b[0m");
                    let result =
                        stdio::output(&json!({"generation":generation,"bytes":bytes})).await;
                    let reply = result.unwrap_or_else(|error| json!({"error":error}));
                    acknowledge(&*native, endpoint, reply).await?;
                    context.ui()?.set_title("ROOT-STDOUT-DONE")
                })
            }),
        },
    )
}

/// Executes the supplied checked Rust child through the real native capability.
async fn acknowledge(
    native: &dyn maestro_extensions_wasm::io::NativeIo,
    endpoint: String,
    reply: Value,
) -> Result<(), String> {
    use maestro_extensions_wasm::child_process::{SpawnOptions, Stdio};
    let command = std::env::var("MAESTRO_WASI_ROOT_COMMAND").map_err(|error| error.to_string())?;
    let command: Value = serde_json::from_str(&command).map_err(|error| error.to_string())?;
    let program = command["command"]
        .as_str()
        .ok_or("missing native program")?;
    let args =
        serde_json::from_value(command["args"].clone()).map_err(|error| error.to_string())?;
    let prologue: Vec<u8> =
        serde_json::from_value(command["prologue"].clone()).map_err(|error| error.to_string())?;
    let child = native
        .spawn(
            program.into(),
            args,
            SpawnOptions {
                env: Some(vec![
                    ("MAESTRO_STDOUT_ENDPOINT".into(), endpoint),
                    ("MAESTRO_STDOUT_REPLY".into(), reply.to_string()),
                ]),
                stdio: [Stdio::Ignore, Stdio::Pipe, Stdio::Ignore],
                ..Default::default()
            },
        )
        .await?;
    let stdout = child.stdout().ok_or("missing native output")?;
    let mut output = Vec::new();
    while let Some(bytes) = stdout.read().await? {
        output.extend(bytes);
    }
    if !output.starts_with(&prologue) || child.wait().await?.code != Some(0) {
        return Err("native acknowledgement failed".into());
    }
    Ok(())
}
