//! Test-side admission counters distinguish one-pass forwarding from history rescans.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{EventStream, ExtensionResult};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

/// Observe actual native admissions without instrumenting the production adapter.
async fn counted(size: usize, rescan: bool) -> ExtensionResult<Value> {
    let classified = Rc::new(Cell::new(0));
    let extracted = Rc::new(Cell::new(0));
    let classify = classified.clone();
    let extract = extracted.clone();
    let stream = EventStream::new(
        move |value: &usize| {
            classify.set(classify.get() + 1);
            *value == size
        },
        move |value| {
            extract.set(extract.get() + 1);
            *value
        },
    )?;
    for value in 1..=size {
        let start = if rescan { 1 } else { value };
        for admitted in start..=value {
            stream.push(admitted)?;
        }
    }
    stream.push(size + 1)?;
    let result = stream.result().await?;
    let mut delivered = 0;
    while stream.next().await?.is_some() {
        delivered += 1;
    }
    Ok(json!({
        "size": size, "classified": classified.get(), "extracted": extracted.get(),
        "delivered": delivered, "result": result,
    }))
}

/// Exercise one-pass forwarding, and a deliberate quadratic negative control that is small
/// because each of its pushes, classifications and deliveries crosses the component boundary.
pub async fn admission() -> ExtensionResult<Value> {
    let mut single_pass = Vec::new();
    for size in [64, 128, 256] {
        single_pass.push(counted(size, false).await?);
    }
    let mut rescanned = Vec::new();
    for size in [16, 32, 64] {
        rescanned.push(counted(size, true).await?);
    }
    Ok(json!({"singlePass": single_pass, "rescanned": rescanned}))
}
