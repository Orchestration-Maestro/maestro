//! Cancel-on-drop native browser callbacks.
/// One admitted native callback registration.
pub struct Timer {
    /// Revocation also retains the native callback until cancellation.
    cancel: Option<Box<dyn FnOnce()>>,
}
impl Timer {
    /// Retain a cancellation operation supplied by a platform adapter.
    pub fn new(cancel: impl FnOnce() + 'static) -> Self {
        Self {
            cancel: Some(Box::new(cancel)),
        }
    }
    /// Revoke future delivery exactly once.
    pub fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel();
        }
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.cancel();
    }
}
/// Admit a timeout or repeating native interval.
#[cfg(target_arch = "wasm32")]
pub(crate) fn native(
    window: web_sys::Window,
    delay: i32,
    repeat: bool,
    callback: std::rc::Rc<dyn Fn()>,
) -> Result<Timer, wasm_bindgen::JsValue> {
    register(
        window,
        if repeat {
            Schedule::Interval(delay)
        } else {
            Schedule::Timeout(delay)
        },
        callback,
    )
}
/// Register one native animation frame with the same callback lifetime owner.
#[cfg(target_arch = "wasm32")]
pub(crate) fn frame(
    window: web_sys::Window,
    callback: std::rc::Rc<dyn Fn()>,
) -> Result<Timer, wasm_bindgen::JsValue> {
    register(window, Schedule::Frame, callback)
}
/// Native registration and cancellation operation.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
enum Schedule {
    /// One delayed delivery.
    Timeout(i32),
    /// Repeated delayed delivery.
    Interval(i32),
    /// One next-frame delivery.
    Frame,
}
/// Retain and revoke callbacks through one shared admission implementation.
#[cfg(target_arch = "wasm32")]
fn register(
    window: web_sys::Window,
    schedule: Schedule,
    callback: std::rc::Rc<dyn Fn()>,
) -> Result<Timer, wasm_bindgen::JsValue> {
    use std::{cell::Cell, rc::Rc};
    use wasm_bindgen::{JsCast as _, closure::Closure};
    let active = Rc::new(Cell::new(true));
    let admitted = Rc::clone(&active);
    let handler = Closure::wrap(Box::new(move || {
        if admitted.get() {
            admitted.set(matches!(schedule, Schedule::Interval(_)));
            callback();
        }
    }) as Box<dyn FnMut()>);
    let function = handler.as_ref().unchecked_ref();
    let id = match schedule {
        Schedule::Interval(delay) => {
            window.set_interval_with_callback_and_timeout_and_arguments_0(function, delay)?
        }
        Schedule::Timeout(delay) => {
            window.set_timeout_with_callback_and_timeout_and_arguments_0(function, delay)?
        }
        Schedule::Frame => window.request_animation_frame(function)?,
    };
    Ok(Timer::new(move || {
        active.set(false);
        match schedule {
            Schedule::Frame => {
                let _ = window.cancel_animation_frame(id);
            }
            Schedule::Interval(_) | Schedule::Timeout(_) => window.clear_interval_with_handle(id),
        }
        drop(handler);
    }))
}
