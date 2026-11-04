//! Authored streams carrying closures rather than JSON documents.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{EventStream, ExtensionResult};
use std::{cell::Cell, rc::Rc};

/// An event whose callable cannot be serialized or cloned.
struct Event {
    /// Captured caller-local computation.
    value: Box<dyn Fn() -> i32>,
}
/// A different result type retaining its own caller-local computation.
struct Answer {
    /// Captured final-result computation.
    text: Box<dyn Fn() -> String>,
}

/// Exercise real classifier, extractor, event and repeated-result ownership.
pub async fn generic_values() -> ExtensionResult<String> {
    let classified = Rc::new(Cell::new(0));
    let extracted = Rc::new(Cell::new(0));
    let classify_count = classified.clone();
    let extract_count = extracted.clone();
    let stream = EventStream::new(
        move |event: &Event| {
            classify_count.set(classify_count.get() + 1);
            (event.value)() < 0
        },
        move |event: &Event| {
            extract_count.set(extract_count.get() + 1);
            let number = (event.value)();
            Answer {
                text: Box::new(move || format!("answer:{number}:雪")),
            }
        },
    )?;
    let local = Rc::new(Cell::new(7));
    let value = local.clone();
    stream.push(Event {
        value: Box::new(move || value.get()),
    })?;
    local.set(8);
    stream.push(Event {
        value: Box::new(|| -1),
    })?;
    if (classified.get(), extracted.get()) != (2, 1) {
        return Err("callbacks did not finish synchronously".into());
    }
    let result = stream.result().await?;
    let repeated = stream.result().await?;
    let first = stream.next().await?.ok_or("missing first event")?;
    let terminal = stream.next().await?.ok_or("missing terminal event")?;
    let eof = stream.next().await?.is_none();
    Ok(serde_json::json!({
        "first": (first.value)(), "terminal": (terminal.value)(),
        "result": (result.text)(), "sameOwner": Rc::ptr_eq(&result, &repeated),
        "eof": eof, "classified": classified.get(), "extracted": extracted.get(),
    })
    .to_string())
}

#[path = "assistant_streams.rs"]
mod assistant;
#[path = "stream_reads.rs"]
pub(super) mod reads;
#[path = "stream_reentry.rs"]
mod reentry;
#[path = "stream_work.rs"]
mod work;

/// Dispatch one bounded authored-stream observation.
pub async fn command(request: &str) -> ExtensionResult<String> {
    let report = match request {
        "generic" => return generic_values().await,
        "admission" => work::admission().await?,
        "assistant-events" => assistant::events().await?,
        "assistant-results" => assistant::results().await?,
        "readers" => reads::readers().await?,
        "terminal-readers" => reads::terminal_readers().await?,
        "optional-results" => reads::optional_results().await?,
        "first-terminal" => reads::first_terminal().await?,
        "reentry" => serde_json::json!([
            reentry::observe(false).await?,
            reentry::observe(true).await?
        ]),
        _ => return Err("unknown stream observation".into()),
    };
    Ok(report.to_string())
}
