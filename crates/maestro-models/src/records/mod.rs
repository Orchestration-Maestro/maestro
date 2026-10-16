//! Model invocation records and their owning utilities.

pub mod event_stream;
pub mod hash;
pub mod headers;
pub mod typebox_helpers;

pub mod diagnostics;

pub mod session_resources;

pub mod types;

pub mod api_registry;
pub mod stream;

/// Generate access to an independently owned native or browser-local store.
macro_rules! platform_store_accessor {
    ($(#[$doc:meta])* $name:ident, $store:ident, $state:ty) => {
        $(#[$doc])*
        fn $name<T>(operation: impl FnOnce(&mut $state) -> T) -> T {
            #[cfg(not(target_arch = "wasm32"))]
            {
                operation(&mut $store.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
            }
            #[cfg(target_arch = "wasm32")]
            {
                $store.with_borrow_mut(operation)
            }
        }
    };
}
pub(crate) use platform_store_accessor;
