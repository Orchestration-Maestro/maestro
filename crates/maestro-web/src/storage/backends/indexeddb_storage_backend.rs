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

#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
pub use browser::IndexedDBStorageBackend;

/// Native request handles and their callback lifetimes.
#[cfg(target_arch = "wasm32")]
mod browser;
