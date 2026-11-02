//! Command-local capabilities survive suspension independently of guarded reads.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionCommandContext, ExtensionResult};
use serde_json::json;

/// Holds actual acquired owners across the admitted wait, then catches the stale import.
pub(super) async fn observe(
    api: ExtensionAPI,
    context: ExtensionCommandContext,
) -> ExtensionResult<()> {
    let ui = context.ui()?;
    let history = context.session_manager()?;
    let catalog = context.model_registry()?;
    let signal = context.signal()?.ok_or("missing retained signal")?;
    let model = context.model()?.ok_or("missing owned model")?;
    context.wait_for_idle().await?;
    let stale = context.cwd().err().ok_or("guarded read remained active")?;
    api.events().emit("retained-caught", json!(stale)).await?;
    let result = json!({"stale":stale,"history":history.get_session_name()?,"cwd":history.get_cwd()?,
        "catalog":catalog.get_all()?.into_iter().map(|model| model.id).collect::<Vec<_>>(),
        "aborted":signal.aborted(),"model":model.name});
    drop(ui);
    Err(result.to_string())
}

/// Finishes independently admitted same-Store work without another guarded read.
pub(super) async fn healthy(context: ExtensionCommandContext) -> ExtensionResult<()> {
    let cwd = context.cwd()?;
    context.reload().await?;
    if cwd != "/retained-original" {
        return Err("healthy operation selected another owner".into());
    }
    Ok(())
}
