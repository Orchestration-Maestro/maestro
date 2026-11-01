//! Authored raw storage and fallback callbacks used by the production host controls.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{oauth, providers};
use maestro_extensions_wasm::{
    AsyncLockUpdate, AuthStorage, AuthStorageBackend, ExtensionAPI, ExtensionFuture,
    ExtensionResult,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
#[path = "credentials/lifetime.rs"]
mod lifetime;
#[path = "credentials/state.rs"]
mod state;

/// Observes accepted native state under independent backend failure phases.
pub(super) async fn state(api: &ExtensionAPI) -> ExtensionResult<Value> {
    state::control(api).await
}

/// Routes separately admitted resolver operations through the same author callbacks.
pub(super) fn lifetime(
    api: &ExtensionAPI,
    context: &maestro_extensions_wasm::ExtensionContext,
    request: &str,
) -> ExtensionResult<Value> {
    lifetime::control(api, context, request)
}

/// Raw text retained by the author's backend, not a host-owned credential snapshot.
struct Backend {
    /// The author's live document.
    raw: Rc<RefCell<Option<String>>>,
    /// Pending controls observe the actual native backend's final release.
    _capture: Option<PendingCapture>,
    /// An awaited acquisition or write may fail independently of synchronous reload.
    failure: &'static str,
}

/// A native-owned callback capture, counted when its author object is released.
struct PendingCapture(&'static str);
impl Drop for PendingCapture {
    fn drop(&mut self) {
        PENDING_DROPS.with(|drops| drops.borrow_mut().push(self.0));
    }
}
impl AuthStorageBackend for Backend {
    fn with_lock(
        &self,
        update: &mut dyn FnMut(Option<&str>) -> ExtensionResult<Option<String>>,
    ) -> ExtensionResult<()> {
        let current = self.raw.borrow().clone();
        let next = update(current.as_deref())?;
        if let Some(next) = next {
            self.raw.replace(Some(next));
        }
        Ok(())
    }
    fn with_lock_async<'a>(&'a self, update: AsyncLockUpdate<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if self.failure == "awaited acquisition failed" {
                return Err(self.failure.into());
            }
            let current = self.raw.borrow().clone();
            let next = update(current.clone()).await?;
            let owner = PENDING.with(|slot| slot.borrow().clone());
            let accepted = owner.map(|owner| owner.get("author-pending")).transpose()?;
            PENDING_UPDATES.with(|updates| {
                updates
                    .borrow_mut()
                    .push(json!({"current":current,"next":next,"accepted":accepted}));
            });
            if self.failure == "awaited write failed" && next.is_some() {
                return Err(self.failure.into());
            }
            if let Some(next) = next {
                self.raw.replace(Some(next));
            }
            Ok(())
        })
    }
}

/// Reenters the native borrowed update on its original pump during construction and reload.
pub(super) fn backend(api: &ExtensionAPI) -> ExtensionResult<Value> {
    let raw = Rc::new(RefCell::new(Some(
        r#"{"stored":{"type":"api_key","key":"first"},"untyped":null,"string":"opaque","number":1.5,"boolean":true,"object":{"type":"api_key","key":false},"array":[null,false],"oauth":{"type":"oauth","refresh":"r","access":"a","expires":42.5,"extra":["x","x"]}}"#.into(),
    )));
    let owner = AuthStorage::from_storage(
        api,
        Rc::new(Backend {
            raw: raw.clone(),
            _capture: None,
            failure: "",
        }),
    )?;
    owner.reload()?;
    let before = owner.get("stored")?;
    owner.set(
        "other",
        maestro_extensions_wasm::AuthCredential::ApiKey(
            maestro_extensions_wasm::ApiKeyCredential {
                env: None,
                key: "second".into(),
            },
        ),
    )?;
    let domains = ["untyped", "string", "number", "boolean", "object", "array"];
    let typed = domains
        .iter()
        .map(|provider| owner.get(provider))
        .collect::<Result<Vec<_>, _>>()?;
    let present = domains
        .iter()
        .map(|provider| owner.has(provider))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        json!({"before":before,"has":owner.has("untyped")?,"typed":owner.get("untyped")?,
        "raw":serde_json::from_str::<Value>(&raw.borrow().clone().ok_or("raw text missing")?).map_err(|e|e.to_string())?,
        "typedDomains":typed,"presentDomains":present,"list":owner.list()?,"all":owner.get_all()?,"missing":[json!(owner.has("missing")?),json!(owner.get("missing")?)],"errors":owner.drain_errors()?}),
    )
}

/// Reenters the host from each synchronous fallback result and replaces only its resolver.
pub(super) async fn fallback(api: &ExtensionAPI) -> ExtensionResult<Value> {
    let owner = AuthStorage::in_memory(api, maestro_extensions_wasm::AuthStorageData::default())?;
    let catalog = maestro_extensions_wasm::ModelRegistry::in_memory(api, owner.clone())?;
    let mut model = catalog
        .get_all()?
        .into_iter()
        .next()
        .ok_or("missing shipped model")?;
    model.provider = "no-env-fallback".into();
    let probe = AuthStorage::in_memory(api, maestro_extensions_wasm::AuthStorageData::default())?;
    probe.set_runtime_api_key("probe", "runtime")?;
    let mut values = Vec::new();
    for value in [None, Some(""), Some("fallback-key")] {
        let probe = probe.clone();
        owner.set_fallback_resolver(Rc::new(move |_| {
            if !probe.has_auth("probe")? {
                return Err("nested native read lost its value".into());
            }
            Ok(value.map(str::to_owned))
        }))?;
        if catalog.has_configured_auth(model.clone())? != owner.has_auth("no-env-fallback")?
            || catalog.get_provider_auth_status("no-env-fallback")?
                != owner.get_auth_status("no-env-fallback")?
        {
            return Err("catalog fallback metadata detached".into());
        }
        values.push(json!({"has":owner.has_auth("no-env-fallback")?,"status":owner.get_auth_status("no-env-fallback")?,"key":owner.get_api_key("no-env-fallback", None).await?,"without":owner.get_api_key("no-env-fallback", Some(maestro_extensions_wasm::GetApiKeyOptions { include_fallback:Some(false) })).await?}));
    }
    owner.set_fallback_resolver(Rc::new(|_| Err("literal fallback failure".into())))?;
    let error = owner
        .has_auth("no-env-fallback")
        .err()
        .ok_or("fallback failure was lost")?;
    owner.set_fallback_resolver(Rc::new(|_| Ok(None)))?;
    Ok(json!({"values":values,"error":error,"recovered":owner.has_auth("no-env-fallback")?}))
}

thread_local! {
    /// Owner kept beyond the callback that constructed it.
    static PENDING: RefCell<Option<AuthStorage>> = const { RefCell::new(None) };
    /// Raw backend text remains independently observable after its credential resource retires.
    static RAW: RefCell<Option<Rc<RefCell<Option<String>>>>> = const { RefCell::new(None) };
    /// Backend and fallback captures retire only when the admitted request settles.
    static PENDING_DROPS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    /// Actual borrowed-update results and accepted state before a backend replies.
    static PENDING_UPDATES: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
}

/// Retains, consumes or releases credentials independently of an already-admitted key request.
pub(super) async fn pending(api: &ExtensionAPI, request: &str) -> ExtensionResult<Value> {
    match request {
        "create" | "create-acquire" | "create-write" | "create-reread" => {
            retain_pending(api, request)
        }
        "key" => {
            let owner = PENDING
                .with(|slot| slot.borrow().clone())
                .ok_or("missing pending owner")?;
            let key = owner.get_api_key("author-pending", None).await?;
            Ok(json!({"key":key,"errors":owner.drain_errors()?}))
        }
        "release" => {
            PENDING.with(RefCell::take);
            Ok(json!("released"))
        }
        "counts" => Ok(PENDING_DROPS.with(|drops| json!(*drops.borrow()))),
        "updates" => Ok(PENDING_UPDATES.with(|updates| json!(*updates.borrow()))),
        "stored" => {
            let owner = PENDING
                .with(|slot| slot.borrow().clone())
                .ok_or("missing pending owner")?;
            Ok(json!(owner.get("author-pending")?))
        }
        "text" => {
            let raw = RAW
                .with(|slot| slot.borrow().clone())
                .ok_or("missing raw backend")?;
            Ok(json!(*raw.borrow()))
        }
        "raw" => {
            let raw = RAW
                .with(|slot| slot.borrow().clone())
                .ok_or("missing raw backend")?;
            let text = raw.borrow().clone().ok_or("missing raw text")?;
            serde_json::from_str(&text).map_err(|error| error.to_string())
        }
        _ => Err("unknown pending credential operation".into()),
    }
}

/// Constructs the retained backend before selecting an independent awaited failure.
fn retain_pending(api: &ExtensionAPI, request: &str) -> ExtensionResult<Value> {
    let raw = Rc::new(RefCell::new(Some(r#"{"author-pending":{"type":"oauth","refresh":"original-refresh","access":"old","expires":0},"unknown":[null,false,2]}"#.into())));
    PENDING_DROPS.with(|drops| drops.borrow_mut().clear());
    PENDING_UPDATES.with(|updates| updates.borrow_mut().clear());
    let owner = AuthStorage::from_storage(
        api,
        Rc::new(Backend {
            raw: raw.clone(),
            _capture: Some(PendingCapture("backend")),
            failure: match request {
                "create-acquire" => "awaited acquisition failed",
                "create-write" => "awaited write failed",
                _ => "",
            },
        }),
    )?;
    let capture = PendingCapture("fallback");
    owner.set_fallback_resolver(Rc::new(move |_| {
        let _capture = &capture;
        Ok(None)
    }))?;
    PENDING.with(|slot| slot.replace(Some(owner)));
    if request == "create-reread" {
        raw.replace(Some(" \n{\"unknown\":[null,false,2]} \n".into()));
    }
    RAW.with(|slot| slot.replace(Some(raw)));
    Ok(json!("retained"))
}

/// Runs the shared retained-owner composition through either host transport.
pub(super) async fn composition(api: &ExtensionAPI, request: &str) -> ExtensionResult<Value> {
    state::composition(api, request).await
}

/// Runs native credential login over the controlled raw backend.
pub(super) async fn login(
    api: &ExtensionAPI,
    mode: &str,
    callbacks: Rc<maestro_extensions_wasm::OAuthLoginCallbacks>,
) -> ExtensionResult<Value> {
    state::login(api, mode, callbacks).await
}

/// Dispatches exact credential observations after parameterized routes are selected.
pub(super) async fn observation(
    api: &ExtensionAPI,
    context: &maestro_extensions_wasm::ExtensionContext,
    request: &str,
) -> Result<serde_json::Value, String> {
    match request {
        request if request.starts_with("credential oauth login ") => {
            oauth::login(
                api,
                context,
                request.trim_start_matches("credential oauth login "),
            )
            .await
        }
        "credential oauth pending create" => oauth::pending_create(context),
        "credential oauth pending complete" => oauth::pending_complete().await,
        "credential oauth providers" => oauth::providers(context),
        "credential oauth registration" => oauth::registration(api, context).await,
        "credential oauth operations" => oauth::operations(context).await,
        "credential oauth retain" | "credential oauth key" | "credential oauth release" => {
            oauth::retained(context, request.trim_start_matches("credential oauth "))
        }
        "credential fallback" => fallback(api).await,
        "credential backend" => backend(api),
        "credential state" => state(api).await,
        _ => providers::command(api, context, request).await,
    }
}
