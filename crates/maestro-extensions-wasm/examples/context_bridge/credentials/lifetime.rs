//! Captured resolvers observe replacement and final native resource release.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AuthStorage, AuthStorageData, ExtensionAPI, ExtensionContext, ExtensionResult, ModelRegistry,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

thread_local! {
    /// The sole author alias, released separately from its callback captures.
    static OWNER: RefCell<Option<AuthStorage>> = const { RefCell::new(None) };
    /// A separately acquired catalog keeps its original credentials alive.
    static CATALOG: RefCell<Option<ModelRegistry>> = const { RefCell::new(None) };
    /// Each resolver's destructor records its identity and a reentrant native result.
    static DROPS: RefCell<Vec<(usize, bool)>> = const { RefCell::new(Vec::new()) };
}

/// A resolver capture whose destruction reenters an unrelated live native owner.
struct Capture(usize, AuthStorage);
impl Capture {
    /// Reads the retained probe before returning this resolver's distinct key.
    fn key(&self) -> ExtensionResult<Option<String>> {
        if !self.1.has_auth("probe")? {
            return Err("resolver capture lost its native owner".into());
        }
        Ok(Some(format!("resolver-{}", self.0)))
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let configured = self.1.has_auth("probe").unwrap_or(false);
        DROPS.with(|drops| drops.borrow_mut().push((self.0, configured)));
    }
}

/// Replaces one resolver at a time, then releases every alias of its credential owner.
pub(super) fn control(
    api: &ExtensionAPI,
    context: &ExtensionContext,
    request: &str,
) -> ExtensionResult<Value> {
    if request == "counts" {
        return Ok(DROPS.with(|drops| json!(*drops.borrow())));
    }
    if request == "keep-catalog" {
        let catalog = context.model_registry()?;
        CATALOG.with(|slot| slot.replace(Some(catalog)));
        return Ok(json!("retained"));
    }
    if request == "release-catalog" {
        CATALOG.with(RefCell::take);
        return Ok(json!("released"));
    }
    if request == "release" {
        OWNER.with(RefCell::take);
        return Ok(json!("released"));
    }
    let id = match request {
        "create" | "session" => {
            DROPS.with(|drops| drops.borrow_mut().clear());
            let owner = if request == "session" {
                context.model_registry()?.auth_storage()?
            } else {
                AuthStorage::in_memory(api, AuthStorageData::default())?
            };
            OWNER.with(|slot| slot.replace(Some(owner)));
            1
        }
        "replace" => 2,
        _ => return Err("unknown resolver lifetime operation".into()),
    };
    let probe = AuthStorage::in_memory(api, AuthStorageData::default())?;
    probe.set_runtime_api_key("probe", "kept")?;
    let capture = Capture(id, probe);
    let owner = OWNER
        .with(|owner| owner.borrow().clone())
        .ok_or("missing resolver owner")?;
    owner.set_fallback_resolver(Rc::new(move |_| capture.key()))?;
    Ok(json!(owner.has_auth("no-env-lifetime")?))
}
