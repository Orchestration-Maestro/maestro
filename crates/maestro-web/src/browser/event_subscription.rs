//! Owned native event registration shared by browser consumers.
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::EventTarget;

/// Retain the exact registration target and callable until detachment.
pub(crate) struct EventSubscription<C: AsRef<JsValue>> {
    /// Original native event target.
    target: EventTarget,
    /// Registered native event name.
    name: &'static str,
    /// Callable retained through successful removal.
    callback: Option<C>,
}
impl<C: AsRef<JsValue>> EventSubscription<C> {
    /// Attach a retained callable to its native target.
    pub(crate) fn new(
        target: EventTarget,
        name: &'static str,
        callback: C,
    ) -> Result<Self, JsValue> {
        target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        Ok(Self {
            target,
            name,
            callback: Some(callback),
        })
    }
    /// Detach without releasing the callable, allowing a failed removal to be retried.
    pub(crate) fn detach(&self) -> Result<(), JsValue> {
        if let Some(callback) = &self.callback {
            self.target.remove_event_listener_with_callback(
                self.name,
                callback.as_ref().unchecked_ref(),
            )?;
        }
        Ok(())
    }
}
impl<C: AsRef<JsValue>> Drop for EventSubscription<C> {
    fn drop(&mut self) {
        if self.detach().is_err() {
            // A native target may still invoke its callable after failed removal.
            std::mem::forget(self.callback.take());
        }
    }
}

#[cfg(test)]
mod tests;
