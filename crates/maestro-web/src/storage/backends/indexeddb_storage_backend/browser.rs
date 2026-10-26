//! Native request handles and their callback lifetimes.
use super::{
    RequestTeardown, cursor_direction, error_kind, explicit_key, missing_stores, prefix_bounds,
    quota_info,
};
use cursor::Progress;
use maestro_app::browser::storage::{IndexDirection, QuotaInfo, TransactionMode};
use std::future::Future;

/// JSON application faces on the same native owners.
mod application;
/// Cursor progress on the shared request owner.
mod cursor;
/// Retained native transaction operations.
mod transaction;
use js_sys::{Promise, Reflect};
use maestro_app::IndexedDBConfig;
use std::{cell::RefCell, fmt, io, rc::Rc};
pub use transaction::IndexedDBStorageTransaction;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    IdbDatabase, IdbFactory, IdbIndexParameters, IdbObjectStore, IdbObjectStoreParameters,
    IdbOpenDbRequest, IdbRequest, IdbTransactionMode,
};

/// Owned exception diagnostics usable as an I/O error cause.
#[derive(Debug)]
struct BrowserError {
    /// Original exception category.
    name: String,
    /// Original exception message.
    message: String,
}
impl fmt::Display for BrowserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.message)
    }
}
impl std::error::Error for BrowserError {}
/// Preserve native exception diagnostics at the I/O boundary.
fn browser_error(value: &JsValue) -> io::Error {
    let diagnostic = if let Some(error) = value.dyn_ref::<web_sys::DomException>() {
        BrowserError {
            name: error.name(),
            message: error.message(),
        }
    } else if let Some(error) = value.dyn_ref::<js_sys::Error>() {
        BrowserError {
            name: error.name().into(),
            message: error.message().into(),
        }
    } else {
        BrowserError {
            name: "Error".into(),
            message: format!("{value:?}"),
        }
    };
    io::Error::new(error_kind(&diagnostic.name), diagnostic)
}
/// Remove platform roots before any captured request handles are released.
fn detach(request: &IdbRequest) {
    request.set_onsuccess(None);
    request.set_onerror(None);
    if let Some(open) = request.dyn_ref::<IdbOpenDbRequest>() {
        open.set_onupgradeneeded(None);
    }
}
/// Request callbacks remain owned even if a retained opening waiter is dropped.
struct Request {
    /// Shared result, including rejection.
    promise: Promise,
    /// Final-owner settlement runs before releasing the Rust callbacks.
    _teardown: Option<RequestTeardown>,
    /// Rust closures retained until the request owner is released.
    _callbacks: Vec<Closure<dyn FnMut()>>,
}
impl Request {
    /// Install callbacks synchronously, before returning a future to the caller.
    fn new(request: &IdbRequest, config: Option<&Rc<IndexedDBConfig>>) -> Self {
        Self::with_progress(request, config, Progress::Single)
    }
    /// Install the common settlement and teardown owner for either request shape.
    fn with_progress(
        request: &IdbRequest,
        config: Option<&Rc<IndexedDBConfig>>,
        mut progress: Progress,
    ) -> Self {
        let mut callbacks = Vec::new();
        let mut teardown = None;
        let promise = Promise::new(&mut |resolve, reject| {
            let handle = request.clone();
            let finish = resolve.clone();
            teardown = Some(RequestTeardown {
                settle: Box::new(move || {
                    let _ = finish.call1(&JsValue::UNDEFINED, &JsValue::UNDEFINED);
                }),
                detach: Box::new(move || detach(&handle)),
            });
            let settle = settlement_callback(
                request.clone(),
                resolve,
                reject.clone(),
                std::mem::replace(&mut progress, Progress::Single),
            );
            request.set_onsuccess(Some(settle.as_ref().unchecked_ref()));
            request.set_onerror(Some(settle.as_ref().unchecked_ref()));
            callbacks.push(settle);
            if let Some((config, open)) = config.zip(request.dyn_ref::<IdbOpenDbRequest>()) {
                let upgrade = upgrade_callback(request.clone(), config.clone(), reject);
                open.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
                callbacks.push(upgrade);
            }
        });
        Self {
            promise,
            _teardown: teardown,
            _callbacks: callbacks,
        }
    }
    /// Await the request result, not the transaction's commit.
    async fn result(&self) -> io::Result<JsValue> {
        JsFuture::from(self.promise.clone())
            .await
            .map_err(|error| browser_error(&error))
    }
}
/// Resolve or reject from the request itself, not the notification event.
fn settlement_callback(
    request: IdbRequest,
    resolve: js_sys::Function,
    reject: js_sys::Function,
    mut progress: Progress,
) -> Closure<dyn FnMut()> {
    Closure::new(move || {
        let result = request
            .error()
            .and_then(|error| error.map_or_else(|| request.result(), |error| Err(error.into())));
        let result = result.and_then(|value| progress.advance(value));
        if matches!(result, Ok(None)) {
            return;
        }
        detach(&request);
        match result {
            Ok(Some(value)) => {
                let _ = resolve.call1(&JsValue::UNDEFINED, &value);
            }
            Err(error) => {
                let _ = reject.call1(&JsValue::UNDEFINED, &error);
            }
            Ok(None) => {}
        }
    })
}
/// Abort and reject an opening if schema setup fails.
fn upgrade_callback(
    request: IdbRequest,
    config: Rc<IndexedDBConfig>,
    reject: js_sys::Function,
) -> Closure<dyn FnMut()> {
    Closure::new(move || {
        if let Err(error) = upgrade(&request, &config) {
            detach(&request);
            let _ = reject.call1(&JsValue::UNDEFINED, &error);
            if let Some(transaction) = request.transaction() {
                let _ = transaction.abort();
            }
        }
    })
}
/// Create only new stores, observing earlier creations during the same upgrade.
fn upgrade(request: &IdbRequest, config: &IndexedDBConfig) -> Result<(), JsValue> {
    let db: IdbDatabase = request.result()?.dyn_into()?;
    for schema in missing_stores(&config.stores, |name| {
        db.object_store_names().contains(name)
    }) {
        create_store(&db, schema)?;
    }
    Ok(())
}
/// Forward optional store and index settings without changing existing stores.
fn create_store(db: &IdbDatabase, schema: &maestro_app::StoreConfig) -> Result<(), JsValue> {
    let options = IdbObjectStoreParameters::new();
    if let Some(path) = &schema.key_path {
        options.set_key_path_opt_str(Some(path));
    }
    if let Some(auto) = schema.auto_increment {
        options.set_auto_increment(auto);
    }
    let store = db.create_object_store_with_optional_parameters(&schema.name, &options)?;
    for index in schema.indices.iter().flatten() {
        let options = IdbIndexParameters::new();
        if let Some(unique) = index.unique {
            options.set_unique(unique);
        }
        store.create_index_with_str_and_optional_parameters(
            &index.name,
            &index.key_path,
            &options,
        )?;
    }
    Ok(())
}
/// Browser-native database opening and raw structured-clone record operations.
///
/// Methods settle at request success, not transaction commit. This concrete
/// adapter also implements the application's JSON storage traits.
pub struct IndexedDBStorageBackend {
    /// Existing application-owned schema records.
    config: Rc<IndexedDBConfig>,
    /// None is unopened; Err caches a synchronous open failure.
    opening: RefCell<Option<Result<Rc<Request>, JsValue>>>,
}
impl IndexedDBStorageBackend {
    /// Retain configuration without opening the database.
    #[must_use]
    pub fn new(config: IndexedDBConfig) -> Self {
        Self {
            config: Rc::new(config),
            opening: RefCell::new(None),
        }
    }
    /// Start opening with the retained configuration.
    fn open(&self) -> Result<Rc<Request>, JsValue> {
        let factory: IdbFactory =
            Reflect::get(&js_sys::global(), &"indexedDB".into())?.dyn_into()?;
        let request = factory.open_with_f64(&self.config.db_name, self.config.version)?;
        Ok(Rc::new(Request::new(&request, Some(&self.config))))
    }
    /// Release the state borrow before platform calls or awaiting a result.
    async fn get_db(&self) -> io::Result<IdbDatabase> {
        let cached = self.opening.borrow().clone();
        let request = if let Some(cached) = cached {
            cached
        } else {
            let opened = self.open();
            *self.opening.borrow_mut() = Some(opened.clone());
            opened
        }
        .map_err(|error| browser_error(&error))?;
        request
            .result()
            .await?
            .dyn_into()
            .map_err(|error| browser_error(&error))
    }
    /// Start the requested collection's own transaction.
    async fn store(&self, name: &str, mode: IdbTransactionMode) -> io::Result<IdbObjectStore> {
        self.get_db()
            .await?
            .transaction_with_str_and_mode(name, mode)
            .and_then(|transaction| transaction.object_store(name))
            .map_err(|error| browser_error(&error))
    }
    /// Read a raw browser value, treating undefined and null as absent.
    /// # Errors
    /// Returns database, transaction, collection or request failures.
    pub async fn get(&self, store_name: &str, key: &str) -> io::Result<Option<JsValue>> {
        let store = self.store(store_name, IdbTransactionMode::Readonly).await?;
        read(&store, key).await
    }
    /// Put a native value using the live store's inline/out-of-line key policy.
    /// # Errors
    /// Returns database, transaction, key-path, clone or request failures.
    pub async fn set(&self, store_name: &str, key: &str, value: JsValue) -> io::Result<()> {
        let store = self
            .store(store_name, IdbTransactionMode::Readwrite)
            .await?;
        put(&store, key, value).await
    }
    /// Delete the supplied key, including an absent key.
    /// # Errors
    /// Returns database, transaction, collection or request failures.
    pub async fn delete(&self, store_name: &str, key: &str) -> io::Result<()> {
        let store = self
            .store(store_name, IdbTransactionMode::Readwrite)
            .await?;
        remove(&store, key).await
    }
}

/// Read through a caller-selected store on its existing transaction.
async fn read(store: &IdbObjectStore, key: &str) -> io::Result<Option<JsValue>> {
    let request = store
        .get(&key.into())
        .map_err(|error| browser_error(&error))?;
    let value = Request::new(&request, None).result().await?;
    Ok((!value.is_null() && !value.is_undefined()).then_some(value))
}
/// Write using the live store's inline-key policy.
async fn put(store: &IdbObjectStore, key: &str, value: JsValue) -> io::Result<()> {
    let path = store.key_path().map_err(|error| browser_error(&error))?;
    let request = match explicit_key(path.is_truthy(), key) {
        Some(key) => store.put_with_key(&value, &key.into()),
        None => store.put(&value),
    }
    .map_err(|error| browser_error(&error))?;
    Request::new(&request, None).result().await?;
    Ok(())
}
/// Delete through the selected store without opening another transaction.
async fn remove(store: &IdbObjectStore, key: &str) -> io::Result<()> {
    let request = store
        .delete(&key.into())
        .map_err(|error| browser_error(&error))?;
    Request::new(&request, None).result().await?;
    Ok(())
}
impl IndexedDBStorageBackend {
    /// Enumerate native keys in browser order with an optional inclusive prefix.
    /// # Errors
    /// Returns store, range, request or native conversion failures.
    pub async fn keys(&self, store_name: &str, prefix: Option<&str>) -> io::Result<Vec<String>> {
        let store = self.store(store_name, IdbTransactionMode::Readonly).await?;
        let request = match prefix_bounds(prefix) {
            Some((lower, upper)) => web_sys::IdbKeyRange::bound(&lower.into(), &upper.into())
                .and_then(|range| store.get_all_keys_with_key(&range)),
            None => store.get_all_keys(),
        }
        .map_err(|error| browser_error(&error))?;
        let keys = Request::new(&request, None).result().await?;
        js_sys::Array::from(&keys)
            .iter()
            .map(|key| {
                crate::browser::sandbox_value::native_string(&key)
                    .map_err(|error| browser_error(&error))
            })
            .collect()
    }
    /// Collect raw index cursor values, retaining duplicates and native order.
    /// # Errors
    /// Returns store, index, cursor or request failures.
    pub async fn get_all_from_index(
        &self,
        store_name: &str,
        index_name: &str,
        direction: IndexDirection,
    ) -> io::Result<Vec<JsValue>> {
        let store = self.store(store_name, IdbTransactionMode::Readonly).await?;
        let direction = cursor_direction(direction);
        let request = store
            .index(index_name)
            .and_then(|index| index.open_cursor_with_range_and_direction(&JsValue::NULL, direction))
            .map_err(|error| browser_error(&error))?;
        let values = Request::with_progress(&request, None, Progress::Cursor(js_sys::Array::new()))
            .result()
            .await?;
        Ok(js_sys::Array::from(&values).to_vec())
    }
    /// Remove every record in only the supplied collection.
    /// # Errors
    /// Returns transaction, collection or request failures.
    pub async fn clear(&self, store_name: &str) -> io::Result<()> {
        let store = self
            .store(store_name, IdbTransactionMode::Readwrite)
            .await?;
        let request = store.clear().map_err(|error| browser_error(&error))?;
        Request::new(&request, None).result().await?;
        Ok(())
    }
    /// Test presence with a key request, independent of a stored null value.
    /// # Errors
    /// Returns transaction, collection, key or request failures.
    pub async fn has(&self, store_name: &str, key: &str) -> io::Result<bool> {
        let store = self.store(store_name, IdbTransactionMode::Readonly).await?;
        let request = store
            .get_key(&key.into())
            .map_err(|error| browser_error(&error))?;
        Ok(!Request::new(&request, None).result().await?.is_undefined())
    }
    /// Run a callback on one retained native transaction, returning its result.
    ///
    /// Callback completion does not wait for commit or automatically abort on error.
    /// Retaining a handle does not keep the browser transaction active.
    /// # Errors
    /// Returns database, transaction or original callback failures.
    pub async fn transaction<T, F, Fut>(
        &self,
        store_names: &[String],
        mode: TransactionMode,
        operation: F,
    ) -> io::Result<T>
    where
        F: FnOnce(Rc<IndexedDBStorageTransaction>) -> Fut,
        Fut: Future<Output = io::Result<T>>,
    {
        let mode = match mode {
            TransactionMode::Readonly => IdbTransactionMode::Readonly,
            TransactionMode::Readwrite => IdbTransactionMode::Readwrite,
        };
        let names: js_sys::Array = store_names.iter().map(JsValue::from).collect();
        let native = self
            .get_db()
            .await?
            .transaction_with_str_sequence_and_mode(&names, mode)
            .map_err(|error| browser_error(&error))?;
        operation(Rc::new(IndexedDBStorageTransaction::new(native))).await
    }
    /// Estimate bytes without opening the database; unsupported estimation is zero.
    /// # Errors
    /// Returns platform access or estimate promise failures.
    pub async fn get_quota_info(&self) -> io::Result<QuotaInfo> {
        let Some(storage) = storage_support("estimate").map_err(|error| browser_error(&error))?
        else {
            return Ok(quota_info(None, None));
        };
        let promise = storage.estimate().map_err(|error| browser_error(&error))?;
        let estimate: web_sys::StorageEstimate = JsFuture::from(promise)
            .await
            .map_err(|error| browser_error(&error))?
            .unchecked_into();
        Ok(quota_info(estimate.get_usage(), estimate.get_quota()))
    }
    /// Request persistence independently of estimation, without opening the database.
    /// # Errors
    /// Returns platform access or persistence promise failures.
    pub async fn request_persistence(&self) -> io::Result<bool> {
        let Some(storage) = storage_support("persist").map_err(|error| browser_error(&error))?
        else {
            return Ok(false);
        };
        let promise = storage.persist().map_err(|error| browser_error(&error))?;
        Ok(JsFuture::from(promise)
            .await
            .map_err(|error| browser_error(&error))?
            .is_truthy())
    }
}
/// Probe only the requested storage capability, preserving platform failures.
pub(crate) fn storage_support(method: &str) -> Result<Option<web_sys::StorageManager>, JsValue> {
    let navigator = Reflect::get(&js_sys::global(), &"navigator".into())?;
    let storage = Reflect::get(&navigator, &"storage".into())?;
    if storage.is_null()
        || storage.is_undefined()
        || !Reflect::get(&storage, &method.into())?.is_truthy()
    {
        return Ok(None);
    }
    Ok(Some(storage.unchecked_into()))
}
