//! Compiled author fuzzy observations without serializing the items.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult};
use serde_json::{Value, json};
/// Reads one complete fuzzy observation.
pub(super) fn observe(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    match request["operation"].as_str() {
        Some("f.fuzzyMatch") => Ok(json!(maestro_extensions_wasm::fuzzy::fuzzy_match(
            api,
            request["args"][0].as_str().ok_or("missing query")?,
            request["args"][1].as_str().ok_or("missing text")?
        )?)),
        Some("f.fuzzyFilter") => filter(api, request),
        _ => Err("unknown helper observation".into()),
    }
}

/// An original item that has no serialization implementation.
struct Item {
    /// Stable input position.
    id: usize,
    /// Payload the getter reads at its call time.
    text: std::cell::RefCell<String>,
}
/// Verifies every returned reference directly against its original position.
fn identities(selected: &[&Item], items: &[Item]) -> ExtensionResult<()> {
    for item in selected {
        if !std::ptr::eq(*item, std::ptr::from_ref(&items[item.id])) {
            return Err("fuzzy item identity changed".into());
        }
    }
    Ok(())
}
/// Reads ranking and getter order on original non-serializable items.
fn filter(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    if request["test"] == "helper_fuzzy_callbacks_keep_live_edits" {
        return live(api, request);
    }
    let args = &request["args"];
    let (texts, query) = if let Some(texts) = args[0].as_array() {
        (
            texts
                .iter()
                .map(|text| text.as_str().ok_or("missing item text").map(str::to_owned))
                .collect::<Result<Vec<_>, _>>()?,
            args[1].as_str().ok_or("missing query")?,
        )
    } else {
        (
            vec!["a b".into(), "ab".into(), "ab".into(), "x".into()],
            args[0].as_str().ok_or("missing query")?,
        )
    };
    let items: Vec<_> = texts
        .into_iter()
        .enumerate()
        .map(|(id, text)| Item {
            id,
            text: std::cell::RefCell::new(text),
        })
        .collect();
    let mut calls = Vec::new();
    let selected = maestro_extensions_wasm::fuzzy::fuzzy_filter(api, &items, query, |item| {
        calls.push(item.id);
        item.text.borrow().clone()
    })?;
    identities(&selected, &items)?;
    Ok(
        json!({"ids":selected.iter().map(|item|item.id).collect::<Vec<_>>(),"calls":calls,"identity":true}),
    )
}

/// A later projection sees an interior edit, without inventing cancellation policy.
fn live(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    let items = [
        Item {
            id: 0,
            text: std::cell::RefCell::new("a".into()),
        },
        Item {
            id: 1,
            text: std::cell::RefCell::new("b".into()),
        },
    ];
    let edit = request["args"][0].as_str().ok_or("missing edit")?;
    let mut calls = Vec::new();
    let mut aborted = false;
    let selected = maestro_extensions_wasm::fuzzy::fuzzy_filter(api, &items, "a", |item| {
        calls.push(item.id);
        if item.id == 0 {
            if edit == "in-place" {
                *items[1].text.borrow_mut() = "a".into();
            }
            if edit == "abort" {
                aborted = true;
            }
        }
        item.text.borrow().clone()
    })?;
    identities(&selected, &items)?;
    Ok(
        json!({"ids":selected.iter().map(|item|item.id).collect::<Vec<_>>(),"calls":calls,"aborted":aborted,"identity":true}),
    )
}
