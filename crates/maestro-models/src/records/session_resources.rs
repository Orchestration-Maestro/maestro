//! Identity-based live cleanup callbacks.
use super::diagnostics::ThrownValue;
use std::sync::Arc;
/// Caller-supplied session cleanup callback; absence differs from an empty id.
#[cfg(not(target_arch = "wasm32"))]
pub type SessionResourceCleanup =
    Arc<dyn Fn(Option<&str>) -> Result<(), ThrownValue> + Send + Sync>;
/// Caller-supplied session cleanup callback; absence differs from an empty id.
#[cfg(target_arch = "wasm32")]
pub type SessionResourceCleanup = Arc<dyn Fn(Option<&str>) -> Result<(), ThrownValue>>;
#[cfg(not(target_arch = "wasm32"))]
type Unregister = Box<dyn Fn() + Send + Sync>;
#[cfg(target_arch = "wasm32")]
type Unregister = Box<dyn Fn()>;
/// Ordered errors collected after sweeping all current callbacks.
#[derive(Debug)]
pub struct AggregateError {
    /// Original supplied thrown values, in visitation order.
    pub errors: Vec<ThrownValue>,
}
impl std::fmt::Display for AggregateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Failed to cleanup session resources")
    }
}
impl std::error::Error for AggregateError {}
#[cfg(not(target_arch = "wasm32"))]
fn with_callbacks<R>(f: impl FnOnce(&mut Vec<Option<SessionResourceCleanup>>) -> R) -> R {
    static CALLBACKS: std::sync::Mutex<Vec<Option<SessionResourceCleanup>>> =
        std::sync::Mutex::new(Vec::new());
    f(&mut CALLBACKS.lock().unwrap_or_else(|p| p.into_inner()))
}
#[cfg(target_arch = "wasm32")]
fn with_callbacks<R>(f: impl FnOnce(&mut Vec<Option<SessionResourceCleanup>>) -> R) -> R {
    thread_local! { static CALLBACKS: std::cell::RefCell<Vec<Option<SessionResourceCleanup>>> = const { std::cell::RefCell::new(Vec::new()) }; }
    CALLBACKS.with(|r| f(&mut r.borrow_mut()))
}
/// Register once by callback identity; dropping the unregister function does nothing.
pub fn register_session_resource_cleanup(cleanup: SessionResourceCleanup) -> Unregister {
    with_callbacks(|r| {
        if !r.iter().flatten().any(|c| Arc::ptr_eq(c, &cleanup)) {
            r.push(Some(cleanup.clone()));
        }
    });
    Box::new(move || {
        let removed = with_callbacks(|r| {
            r.iter_mut()
                .find(|c| c.as_ref().is_some_and(|c| Arc::ptr_eq(c, &cleanup)))
                .and_then(Option::take)
        });
        drop(removed);
    })
}
/// Sweep the live set outside locks, collecting all returned errors before failing.
pub fn cleanup_session_resources(session_id: Option<&str>) -> Result<(), AggregateError> {
    let mut position = 0;
    let mut errors = vec![];
    loop {
        let next = with_callbacks(|r| {
            if position < r.len() {
                let value = r[position].clone();
                position += 1;
                Some(value)
            } else {
                None
            }
        });
        match next {
            Some(Some(callback)) => {
                if let Err(e) = callback(session_id) {
                    errors.push(e);
                }
            }
            Some(None) => {}
            None => break,
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AggregateError { errors })
    }
}
