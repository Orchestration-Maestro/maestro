//! Canonical capability reads returned to the native integration caller.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionContext, ReadonlySessionManager, SessionTreeNode};
use serde_json::{Value, json};

/// Exercises the actual imports behind both ordinary and command context ports.
pub(super) fn observe(context: &ExtensionContext) -> Result<Value, String> {
    let _ui = context.ui()?;
    let history = context.session_manager()?;
    let catalog = context.model_registry()?;
    let all = catalog.get_all()?;
    let model = all.first().ok_or("missing canonical model")?;
    let found = catalog.find(&model.provider, &model.id)?;
    let missing = catalog.find(&model.provider, "missing model")?;
    let signal = context.signal()?.ok_or("missing canonical signal")?;
    context.compact(Some(maestro_extensions_wasm::CompactOptions {
        custom_instructions: Some("literal compact instructions".into()),
        on_complete: Some(std::rc::Rc::new(|result, emitter| {
            emitter.emit(
                "canonical-compaction",
                serde_json::to_value(result).map_err(|error| error.to_string())?,
            )
        })),
        on_error: Some(std::rc::Rc::new(|error, emitter| {
            emitter.emit("canonical-compaction", json!({"error": error}))
        })),
    }))?;
    context.abort()?;
    context.shutdown()?;
    let usage = context.get_context_usage()?.map(|usage| {
        json!({
            "tokens": usage.tokens, "contextWindow": usage.context_window, "percent": usage.percent,
        })
    });
    Ok(json!({
        "cwd": context.cwd()?, "prompt": context.get_system_prompt()?,
        "hasUi": context.has_ui()?, "idle": context.is_idle()?,
        "pending": context.has_pending_messages()?, "aborted": signal.aborted(),
        "usage": usage, "model": context.model()?,
        "history": read_history(&history)?,
        "catalog": {"all": all, "available": catalog.get_available()?, "found": found, "missing": missing},
    }))
}

/// Reads every history method with both present and missing identities.
fn read_history(history: &ReadonlySessionManager) -> Result<Value, String> {
    let entries = history.get_entries()?;
    let first = entries.first().ok_or("missing canonical entry")?;
    let first = serde_json::to_value(first).map_err(|error| error.to_string())?;
    let root = first["id"].as_str().ok_or("missing canonical entry id")?;
    Ok(json!({
        "cwd": history.get_cwd()?, "dir": history.get_session_dir()?,
        "id": history.get_session_id()?, "file": history.get_session_file()?,
        "leafId": history.get_leaf_id()?, "leaf": history.get_leaf_entry()?,
        "entry": history.get_entry(root)?, "missingEntry": history.get_entry("missing entry")?,
        "label": history.get_label(root)?, "missingLabel": history.get_label("missing entry")?,
        "branch": history.get_branch(None)?, "rootBranch": history.get_branch(Some(root))?,
        "header": history.get_header()?, "entries": entries,
        "name": history.get_session_name()?, "tree": read_tree(history.get_tree()?)?,
    }))
}

/// Walks the actual retained node resources, including empty child lists and labels.
fn read_tree(nodes: Vec<SessionTreeNode>) -> Result<Vec<Value>, String> {
    nodes.into_iter().map(|node| Ok(json!({
        "entry": node.entry()?, "label": node.label()?, "labelTimestamp": node.label_timestamp()?,
        "children": read_tree(node.children()?)?,
    }))).collect()
}
