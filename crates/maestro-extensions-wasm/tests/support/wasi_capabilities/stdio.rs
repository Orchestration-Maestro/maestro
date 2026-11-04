//! Separate native standard descriptors and exact byte-stream output.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::wasi;
use serde_json::{Value, json};
use std::io::{IsTerminal, Read, Write};

/// Runs the selected generation against the component's actual inherited descriptors.
pub(super) async fn inherited(generation: &str) -> Result<Value, String> {
    let bytes = if generation == "p2" {
        let mut bytes = Vec::new();
        std::io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        bytes
    } else {
        let (body, completion) = wasi::cli::stdin::read_via_stream();
        let bytes = body.collect().await;
        completion.await.map_err(|error| format!("{error:?}"))?;
        bytes
    };
    let mut out = b"OUT[".to_vec();
    out.extend_from_slice(&bytes);
    out.push(b']');
    let mut err = b"ERR[".to_vec();
    err.extend(bytes.iter().rev());
    err.push(b']');
    if generation == "p2" {
        std::io::stdout()
            .write_all(&out)
            .map_err(|error| error.to_string())?;
        std::io::stdout()
            .flush()
            .map_err(|error| error.to_string())?;
        std::io::stderr()
            .write_all(&err)
            .map_err(|error| error.to_string())?;
        std::io::stderr()
            .flush()
            .map_err(|error| error.to_string())?;
    } else {
        write_stream(&out, "stdout").await?;
        write_stream(&err, "stderr").await?;
    }
    Ok(json!({"input":bytes}))
}

/// Writes through the ordinary or async view without taking ownership of the host sink.
pub(super) async fn output(request: &Value) -> Result<Value, String> {
    let generation = request["generation"].as_str().ok_or("missing generation")?;
    let bytes: Vec<u8> =
        serde_json::from_value(request["bytes"].clone()).map_err(|error| error.to_string())?;
    let terminal = if generation == "p2" {
        std::io::stdout()
            .write_all(&[])
            .map_err(|error| error.to_string())?;
        std::io::stdout()
            .write_all(&bytes)
            .map_err(|error| error.to_string())?;
        std::io::stdout()
            .flush()
            .map_err(|error| error.to_string())?;
        std::io::stdout().is_terminal()
    } else {
        write_stream(&[], "stdout").await?;
        write_stream(&bytes, "stdout").await?;
        wasi::cli::terminal_stdout::get_terminal_stdout().is_some()
    };
    Ok(json!({"terminal":terminal}))
}

/// Uses the standard stream conversion and closes this byte-stream view after delivery.
async fn write_stream(bytes: &[u8], destination: &str) -> Result<(), String> {
    let (mut writer, reader) = wasi::wit_stream::new::<u8>();
    let completion = if destination == "stdout" {
        wasi::cli::stdout::write_via_stream(reader)
    } else {
        wasi::cli::stderr::write_via_stream(reader)
    };
    let remaining = writer.write_all(bytes.to_vec()).await;
    drop(writer);
    completion.await.map_err(|error| format!("{error:?}"))?;
    if !remaining.is_empty() {
        return Err("standard output consumer closed early".into());
    }
    Ok(())
}
