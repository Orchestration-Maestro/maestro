//! Page hooks and dynamic page calls shared by browser witnesses.
#![cfg(target_arch = "wasm32")]
use std::any::Any;
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};

/// A replaced page property, restored to its own previous definition when dropped.
pub(crate) struct Patch {
    /// The object that owns the replaced property.
    target: js_sys::Object,
    /// The replaced property name.
    key: JsValue,
    /// The target's own descriptor before the patch, or undefined when it was inherited.
    previous: JsValue,
    /// The callbacks the replacement calls, kept alive while it is installed.
    hooks: Vec<Box<dyn Any>>,
}

/// A replacement method: the original function, the receiver and the arguments.
type Hook = dyn Fn(js_sys::Function, JsValue, js_sys::Array) -> Result<JsValue, JsValue>;

impl Patch {
    /// Replace the method `name` of `target` with `hook`, assigned as the page itself would.
    pub(crate) fn method(
        target: &JsValue,
        name: &str,
        hook: impl Fn(js_sys::Function, JsValue, js_sys::Array) -> Result<JsValue, JsValue> + 'static,
    ) -> Self {
        let (target, key, previous) = own(target, name);
        let original = js_sys::Reflect::get(&target, &key).unwrap();
        let apply = Closure::<Hook>::new(hook);
        let handler = object(&[("apply", apply.as_ref())]);
        let proxy = js_sys::Proxy::new(original.unchecked_ref(), &handler);
        assert!(
            js_sys::Reflect::set(&target, &key, &proxy).unwrap(),
            "the page refused the {name} replacement"
        );
        Self {
            target,
            key,
            previous,
            hooks: vec![Box::new(apply)],
        }
    }

    /// Define `name` on `target` as a configurable property that `get` reads for its receiver.
    pub(crate) fn getter(
        target: &JsValue,
        name: &str,
        get: impl Fn(JsValue) -> Result<JsValue, JsValue> + 'static,
    ) -> Self {
        let (get, hook) = receiving(move |this, _| get(this));
        let descriptor = object(&[("configurable", &JsValue::TRUE), ("get", &get)]);
        Self::define(target, name, &descriptor, vec![hook])
    }

    /// Define `name` on `target` as a configurable property that `get` reads and `set` writes
    /// for its receiver.
    pub(crate) fn accessor(
        target: &JsValue,
        name: &str,
        get: impl Fn(JsValue) -> Result<JsValue, JsValue> + 'static,
        set: impl Fn(JsValue, JsValue) -> Result<(), JsValue> + 'static,
    ) -> Self {
        let (get, read) = receiving(move |this, _| get(this));
        let (set, write) =
            receiving(move |this, args| set(this, args.get(0)).map(|()| JsValue::UNDEFINED));
        let descriptor = object(&[
            ("configurable", &JsValue::TRUE),
            ("get", &get),
            ("set", &set),
        ]);
        Self::define(target, name, &descriptor, vec![read, write])
    }

    /// Define `name` on `target` as a configurable data property holding `value`.
    pub(crate) fn value(target: &JsValue, name: &str, value: &JsValue) -> Self {
        let descriptor = object(&[
            ("configurable", &JsValue::TRUE),
            ("writable", &JsValue::TRUE),
            ("value", value),
        ]);
        Self::define(target, name, &descriptor, Vec::new())
    }

    /// Keep `hook` alive as long as the patch, for a replacement that calls it.
    pub(crate) fn keeping(mut self, hook: impl Any) -> Self {
        self.hooks.push(Box::new(hook));
        self
    }

    /// Install `descriptor`, keeping `hooks` alive with the patch.
    fn define(
        target: &JsValue,
        name: &str,
        descriptor: &js_sys::Object,
        hooks: Vec<Box<dyn Any>>,
    ) -> Self {
        let (target, key, previous) = own(target, name);
        js_sys::Reflect::define_property(&target, &key, descriptor).unwrap();
        Self {
            target,
            key,
            previous,
            hooks,
        }
    }
}

impl Drop for Patch {
    fn drop(&mut self) {
        if self.previous.is_undefined() {
            js_sys::Reflect::delete_property(&self.target, &self.key).unwrap();
        } else {
            js_sys::Reflect::define_property(
                &self.target,
                &self.key,
                self.previous.unchecked_ref(),
            )
            .unwrap();
        }
    }
}

/// A page function calling `hook` with its receiver and arguments, and the callback it keeps.
fn receiving(
    hook: impl Fn(JsValue, js_sys::Array) -> Result<JsValue, JsValue> + 'static,
) -> (JsValue, Box<dyn Any>) {
    let apply = Closure::<Hook>::new(move |_, this, args| hook(this, args));
    let handler = object(&[("apply", apply.as_ref())]);
    // Any callable proxy target works; the trap alone decides the call.
    let callable = get(&global("Function"), "prototype");
    (
        js_sys::Proxy::new(&callable, &handler).into(),
        Box::new(apply),
    )
}

/// A page function handing every call's arguments, as one array, to `hook`; it lives as long
/// as the page.
pub(crate) fn variadic(hook: impl Fn(js_sys::Array) -> JsValue + 'static) -> JsValue {
    let apply = Closure::<dyn Fn(JsValue, JsValue, js_sys::Array) -> JsValue>::new(
        move |_: JsValue, _: JsValue, args: js_sys::Array| hook(args),
    );
    let handler = object(&[("apply", &apply.into_js_value())]);
    js_sys::Proxy::new(&get(&global("Function"), "prototype"), &handler).into()
}

/// A page promise and the function that settles it.
pub(crate) fn signal() -> (js_sys::Promise, js_sys::Function) {
    let mut settle = None;
    let promise = js_sys::Promise::new(&mut |resolve, _| settle = Some(resolve));
    (promise, settle.unwrap())
}

/// The target object, its property key and its own descriptor before any change.
fn own(target: &JsValue, name: &str) -> (js_sys::Object, JsValue, JsValue) {
    let target: js_sys::Object = target.clone().unchecked_into();
    let key = JsValue::from(name);
    let previous = js_sys::Reflect::get_own_property_descriptor(&target, &key).unwrap();
    (target, key, previous)
}

/// The page global `name`.
pub(crate) fn global(name: &str) -> JsValue {
    get(&js_sys::global(), name)
}

/// The property `key` of a page value.
pub(crate) fn get(target: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(target, &key.into()).unwrap()
}

/// Set the property `key` of a page value.
pub(crate) fn set(target: &JsValue, key: &str, value: &JsValue) {
    js_sys::Reflect::set(target, &key.into(), value).unwrap();
}

/// Call the method `name` of `target` with `args`, returning what it returns or throws.
pub(crate) fn invoke(target: &JsValue, name: &str, args: &[&JsValue]) -> Result<JsValue, JsValue> {
    let method: js_sys::Function = get(target, name).dyn_into()?;
    method.apply(target, &args.iter().copied().collect::<js_sys::Array>())
}

/// Construct the page class `name` with `args`.
pub(crate) fn construct(name: &str, args: &[&JsValue]) -> JsValue {
    js_sys::Reflect::construct(
        global(name).unchecked_ref::<js_sys::Function>(),
        &args.iter().copied().collect::<js_sys::Array>(),
    )
    .unwrap()
}

/// A plain page object with `entries`.
pub(crate) fn object(entries: &[(&str, &JsValue)]) -> js_sys::Object {
    let object = js_sys::Object::new();
    for (key, value) in entries {
        set(&object, key, value);
    }
    object
}

/// A page `Error` carrying `message`.
pub(crate) fn error(message: &str) -> JsValue {
    js_sys::Error::new(message).into()
}

/// A page `DOMException` carrying `message` and `name`.
pub(crate) fn dom_exception(message: &str, name: &str) -> JsValue {
    web_sys::DomException::new_with_message_and_name(message, name)
        .unwrap()
        .into()
}

/// Uncaught page errors captured during one transaction, which the page itself does not report.
pub(crate) struct Errors {
    /// Captured error messages in arrival order.
    messages: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    /// The page listener, removed when the transaction finishes or is dropped.
    capture: Closure<dyn Fn(web_sys::Event)>,
}

impl Errors {
    /// Capture every uncaught page error until finished.
    pub(crate) fn begin() -> Self {
        let messages: std::rc::Rc<std::cell::RefCell<Vec<String>>> = std::rc::Rc::default();
        let seen = std::rc::Rc::clone(&messages);
        let capture = Closure::<dyn Fn(web_sys::Event)>::new(move |event: web_sys::Event| {
            seen.borrow_mut()
                .push(get(&event, "message").as_string().unwrap_or_default());
            event.prevent_default();
        });
        web_sys::window()
            .unwrap()
            .add_event_listener_with_callback("error", capture.as_ref().unchecked_ref())
            .unwrap();
        Self { messages, capture }
    }

    /// End the transaction, requiring exactly `expected` errors.
    pub(crate) fn finish(self, expected: usize) {
        let actual = self.messages.borrow().len();
        assert_eq!(actual, expected, "wrong native error count: {actual}");
    }
}

impl Drop for Errors {
    fn drop(&mut self) {
        web_sys::window()
            .unwrap()
            .remove_event_listener_with_callback("error", self.capture.as_ref().unchecked_ref())
            .unwrap();
    }
}
