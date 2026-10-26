//! `IndexedDB` opening and native-value record operations.
#[cfg(any(target_arch = "wasm32", test))]
use maestro_app::StoreConfig;

/// Observe names lazily so successful creations affect subsequent selections.
#[cfg(any(target_arch = "wasm32", test))]
fn missing_stores(
    stores: &[StoreConfig],
    mut contains: impl FnMut(&str) -> bool,
) -> impl Iterator<Item = &StoreConfig> {
    stores.iter().filter(move |store| !contains(&store.name))
}

/// Choose a separate key only for out-of-line stores.
#[cfg(any(target_arch = "wasm32", test))]
fn explicit_key(inline: bool, key: &str) -> Option<&str> {
    (!inline).then_some(key)
}

/// Represent platform exception categories as native I/O failure kinds.
#[cfg(any(target_arch = "wasm32", test))]
fn error_kind(name: &str) -> std::io::ErrorKind {
    use std::io::ErrorKind;
    match name {
        "VersionError" | "DataError" => ErrorKind::InvalidInput,
        "NotFoundError" => ErrorKind::NotFound,
        "ReadOnlyError" | "SecurityError" => ErrorKind::PermissionDenied,
        "ConstraintError" => ErrorKind::AlreadyExists,
        "QuotaExceededError" => ErrorKind::StorageFull,
        "AbortError" => ErrorKind::Interrupted,
        "DataCloneError" => ErrorKind::InvalidData,
        _ => ErrorKind::Other,
    }
}

/// Settle the request bridge before releasing its platform callbacks.
#[cfg(any(target_arch = "wasm32", test))]
struct RequestTeardown {
    /// Complete a pending bridge; already completed promises ignore settlement.
    settle: Box<dyn FnMut()>,
    /// Remove the platform's callback roots.
    detach: Box<dyn FnMut()>,
}
#[cfg(any(target_arch = "wasm32", test))]
impl Drop for RequestTeardown {
    fn drop(&mut self) {
        (self.settle)();
        (self.detach)();
    }
}

#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
pub(crate) use browser::storage_support;
#[cfg(target_arch = "wasm32")]
pub use browser::{IndexedDBStorageBackend, IndexedDBStorageTransaction};

/// Native request handles and their callback lifetimes.
#[cfg(target_arch = "wasm32")]
mod browser;

/// Select the browser's inclusive prefix endpoints without normalizing keys.
#[cfg(any(target_arch = "wasm32", test))]
fn prefix_bounds(prefix: Option<&str>) -> Option<(&str, String)> {
    prefix
        .filter(|prefix| !prefix.is_empty())
        .map(|prefix| (prefix, format!("{prefix}\u{ffff}")))
}

/// Select non-unique native cursor traversal.
#[cfg(any(target_arch = "wasm32", test))]
fn cursor_direction(
    direction: maestro_app::browser::storage::IndexDirection,
) -> web_sys::IdbCursorDirection {
    match direction {
        maestro_app::browser::storage::IndexDirection::Asc => web_sys::IdbCursorDirection::Next,
        maestro_app::browser::storage::IndexDirection::Desc => web_sys::IdbCursorDirection::Prev,
    }
}

/// Preserve finite native numbers, including the sign bit of zero.
#[cfg(any(target_arch = "wasm32", test))]
fn json_number(value: f64) -> std::io::Result<serde_json::Value> {
    use num_traits::ToPrimitive;
    // Integral JSON values must remain readable by typed application stores.
    if value.fract() == 0.0 && !(value == 0.0 && value.is_sign_negative()) {
        if let Some(integer) = value.to_i64() {
            return Ok(integer.into());
        }
        if let Some(integer) = value.to_u64() {
            return Ok(integer.into());
        }
    }
    serde_json::Number::from_f64(value)
        .map(serde_json::Value::Number)
        .ok_or_else(|| std::io::ErrorKind::InvalidData.into())
}

/// Preserve independent estimate fallbacks and division-before-scaling order.
#[cfg(any(target_arch = "wasm32", test))]
fn quota_info(usage: Option<f64>, quota: Option<f64>) -> maestro_app::browser::storage::QuotaInfo {
    let usage = usage.unwrap_or(0.0);
    let quota = quota.unwrap_or(0.0);
    maestro_app::browser::storage::QuotaInfo {
        usage,
        quota,
        percent: if quota == 0.0 {
            0.0
        } else {
            (usage / quota) * 100.0
        },
    }
}
