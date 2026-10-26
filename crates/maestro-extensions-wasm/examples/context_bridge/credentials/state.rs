//! Accepted-state observations under authored acquisition and persistence failures.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ApiKeyCredential, AsyncLockUpdate, AuthCredential, AuthStorage, AuthStorageBackend,
    ExtensionAPI, ExtensionFuture, ExtensionResult, GetApiKeyOptions, ModelRegistry,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Independently controlled raw text and failure phase.
struct Backend {
    /// Text changes only after a successful replacement.
    raw: RefCell<Option<String>>,
    /// Failure before update, after update, or no failure.
    failure: RefCell<&'static str>,
}
impl Backend {
    /// Acquisition failures prevent the native update from running.
    fn current(&self) -> ExtensionResult<Option<String>> {
        let failure = *self.failure.borrow();
        if failure.starts_with("acquire") || failure.starts_with("reload") {
            return Err(failure.into());
        }
        Ok(self.raw.borrow().clone())
    }
    /// A failed write never publishes its supplied replacement.
    fn replace(&self, next: Option<String>) -> ExtensionResult<()> {
        if let Some(next) = next {
            let failure = *self.failure.borrow();
            if failure.starts_with("write") {
                return Err(failure.into());
            }
            self.raw.replace(Some(next));
        }
        Ok(())
    }
}
impl AuthStorageBackend for Backend {
    fn with_lock(
        &self,
        update: &mut dyn FnMut(Option<&str>) -> ExtensionResult<Option<String>>,
    ) -> ExtensionResult<()> {
        let current = self.current()?;
        self.replace(update(current.as_deref())?)
    }
    fn with_lock_async<'a>(&'a self, update: AsyncLockUpdate<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move { self.replace(update(self.current()?).await?) })
    }
}

/// A literal credential without ambient command or environment interpretation.
fn key(value: &str) -> AuthCredential {
    AuthCredential::ApiKey(ApiKeyCredential { key: value.into() })
}

/// Records ordered errors and accepted state before recovery from each failed persistence.
pub(super) async fn control(api: &ExtensionAPI) -> ExtensionResult<Value> {
    let backend = Rc::new(Backend {
        raw: RefCell::new(Some(
            r#"{"old":{"type":"api_key","key":"original"}}"#.into(),
        )),
        failure: RefCell::new(""),
    });
    let owner = AuthStorage::from_storage(api, backend.clone())?;
    let catalog = ModelRegistry::in_memory(api, owner.clone())?;
    let alias = catalog.auth_storage()?;
    backend.failure.replace("write-first");
    owner.set("new", key("accepted"))?;
    let accepted = json!({"list":alias.list()?,"all":alias.get_all()?,"has":alias.has("new")?,"stored":alias.get("new")?,"key":catalog.get_api_key_for_provider("new").await?});
    backend.failure.replace("acquire-second");
    alias.remove("old")?;
    backend.failure.replace("reload-third");
    alias.reload()?;
    let retained = alias.list()?;
    let errors = alias
        .drain_errors()?
        .into_iter()
        .map(|error| error.message)
        .collect::<Vec<_>>();
    let drained = owner.drain_errors()?;
    let raw = backend.raw.borrow().clone();
    backend.failure.replace("");
    owner.reload()?;
    let recovered = owner.list()?;
    alias.logout("old")?;
    let logged_out = owner.list()?;
    let precedence = precedence(&alias).await?;
    Ok(
        json!({"accepted":accepted,"retained":retained,"errors":errors,"drained":drained,"raw":raw,"recovered":recovered,"logout":logged_out,"precedence":precedence}),
    )
}

/// Empty runtime overrides retain presence but do not override a selected stored key.
async fn precedence(owner: &AuthStorage) -> ExtensionResult<Value> {
    let provider = "credential-precedence-no-env";
    owner.set_fallback_resolver(Rc::new(|_| Ok(Some("fallback".into()))))?;
    let mut keys = vec![owner.get_api_key(provider, None).await?];
    owner.set_runtime_api_key(provider, "")?;
    let empty = json!({"has":owner.has_auth(provider)?,"status":owner.get_auth_status(provider)?,"key":owner.get_api_key(provider,None).await?});
    owner.set_runtime_api_key(provider, "runtime")?;
    owner.set(provider, key("stored"))?;
    keys.push(owner.get_api_key(provider, None).await?);
    owner.set_runtime_api_key(provider, "")?;
    keys.push(owner.get_api_key(provider, None).await?);
    owner.remove_runtime_api_key(provider)?;
    keys.push(owner.get_api_key(provider, None).await?);
    owner.remove(provider)?;
    keys.push(owner.get_api_key(provider, None).await?);
    keys.push(
        owner
            .get_api_key(
                provider,
                Some(GetApiKeyOptions {
                    include_fallback: Some(false),
                }),
            )
            .await?,
    );
    Ok(json!({"keys":keys,"empty":empty,"errors":owner.drain_errors()?}))
}
