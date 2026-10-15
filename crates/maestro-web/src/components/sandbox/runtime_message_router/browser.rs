//! Browser subscriptions, raw responses and registered-window admission.
use super::{Context, MessageConsumer, Providers, Registry};
use crate::browser::sandbox_value::{BrowserSandboxValue, diagnostic};
use js_sys::{Array, Function, Object, Promise, Reflect};
use maestro_app::DiagnosticErrorInfo;
use maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRespond;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    sync::Arc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::future_to_promise;
use web_sys::{HtmlIFrameElement, MessageEvent};

/// Browser-local shared registry.
type State = Registry<BrowserSandboxValue, HtmlIFrameElement>;
/// Browser-local captured context.
type Sandbox = Rc<Context<BrowserSandboxValue, HtmlIFrameElement>>;
/// Both browser hosts supply at most three callback arguments.
type Callback = Closure<dyn FnMut(JsValue, JsValue, JsValue) -> JsValue>;
/// Exact subscription target retained until successful removal.
struct Listener {
    /// Window or extension event source.
    target: JsValue,
    /// Callback whose lifetime follows registration.
    callback: Callback,
    /// Window event name; extension events have dedicated methods.
    event: Option<&'static str>,
}
impl Listener {
    /// Apply the channel's registration or removal operation.
    fn update(&self, add: bool) -> Result<(), JsValue> {
        let name = match (self.event, add) {
            (Some(_), true) => "addEventListener",
            (Some(_), _) => "removeEventListener",
            (None, true) => "addListener",
            (None, _) => "removeListener",
        };
        let args = Array::new();
        if let Some(event) = self.event {
            args.push(&event.into());
        }
        args.push(self.callback.as_ref());
        Reflect::get(&self.target, &name.into())?
            .dyn_into::<Function>()?
            .apply(&self.target, &args)?;
        Ok(())
    }
}
/// Central owner of live sandbox contexts and channel subscriptions.
/// Explicitly unregister sandboxes to detach listeners when the registry empties.
#[derive(Default)]
pub struct RuntimeMessageRouter {
    /// State captured weakly by global listeners.
    state: Rc<State>,
    /// One listener per available channel.
    listeners: RefCell<Vec<Listener>>,
}
impl RuntimeMessageRouter {
    /// Create an empty router without installing listeners.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Retain live providers and identity-deduplicated consumers before iframe creation.
    /// # Errors
    /// Returns a channel registration failure.
    pub fn register_sandbox(
        &self,
        sandbox_id: String,
        providers: Providers<BrowserSandboxValue>,
        consumers: Vec<Arc<dyn MessageConsumer<BrowserSandboxValue>>>,
    ) -> Result<(), DiagnosticErrorInfo> {
        self.state.register(sandbox_id, providers, consumers);
        self.install().map_err(|error| diagnostic(&error))
    }
    /// Replace the current context's iframe slot; unknown identifiers are ignored.
    pub fn set_sandbox_iframe(&self, sandbox_id: &str, iframe: HtmlIFrameElement) {
        self.state.set_frame(sandbox_id, iframe);
    }
    /// Remove the entry, detaching listeners when no sandboxes remain.
    /// # Errors
    /// Returns a channel removal failure, retaining that callback for retry.
    pub fn unregister_sandbox(&self, sandbox_id: &str) -> Result<(), DiagnosticErrorInfo> {
        self.state.unregister(sandbox_id);
        if self.state.is_empty() {
            self.detach().map_err(|error| diagnostic(&error))?;
        }
        Ok(())
    }
    /// Add an allocation once in insertion order; unknown identifiers are ignored.
    pub fn add_consumer(
        &self,
        sandbox_id: &str,
        consumer: Arc<dyn MessageConsumer<BrowserSandboxValue>>,
    ) {
        let context = self.state.sandboxes.borrow().get(sandbox_id).cloned();
        if let Some(context) = context {
            context.add(consumer);
        }
    }
    /// Remove a live allocation without shifting in-flight visitation positions.
    pub fn remove_consumer(
        &self,
        sandbox_id: &str,
        consumer: &Arc<dyn MessageConsumer<BrowserSandboxValue>>,
    ) {
        let context = self.state.sandboxes.borrow().get(sandbox_id).cloned();
        if let Some(context) = context {
            context.remove(consumer);
        }
    }
    /// Install missing channels synchronously before returning registration.
    fn install(&self) -> Result<(), JsValue> {
        let mut listeners = self.listeners.take();
        let result = self.install_missing(&mut listeners);
        self.listeners.replace(listeners);
        result
    }
    /// Register each currently available channel once.
    fn install_missing(&self, listeners: &mut Vec<Listener>) -> Result<(), JsValue> {
        for event in [Some("message"), None] {
            if listeners.iter().any(|listener| listener.event == event) {
                continue;
            }
            let target = if event.is_some() {
                Some(js_sys::global().into())
            } else {
                extension_target()?
            };
            let Some(target) = target else {
                continue;
            };
            let callback = callback(Rc::downgrade(&self.state), event);
            let listener = Listener {
                target,
                callback,
                event,
            };
            listener.update(true)?;
            listeners.push(listener);
        }
        Ok(())
    }
    /// Remove each callback from its original target outside any borrow.
    fn detach(&self) -> Result<(), JsValue> {
        let mut listeners = self.listeners.take();
        while let Some(listener) = listeners.pop() {
            if let Err(error) = listener.update(false) {
                listeners.push(listener);
                self.listeners.replace(listeners);
                return Err(error);
            }
        }
        Ok(())
    }
}
impl Drop for RuntimeMessageRouter {
    fn drop(&mut self) {
        if self.detach().is_err() {
            for listener in self.listeners.take() {
                listener.callback.forget();
            }
        }
    }
}
/// Route host callback arguments without retaining the registry strongly.
fn callback(weak: Weak<State>, event: Option<&'static str>) -> Callback {
    Closure::new(move |message: JsValue, _, reply| {
        if event.is_some() {
            window_message(&weak, message.unchecked_ref())
                .unwrap_or_else(|error| Promise::reject(&error).into())
        } else {
            extension_message(&weak, message, reply)
                .unwrap_or_else(|error| wasm_bindgen::throw_val(error))
        }
    })
}
/// Resolve the optional extension event API rather than merely the chrome global.
fn extension_target() -> Result<Option<JsValue>, JsValue> {
    let mut target: JsValue = js_sys::global().into();
    for field in ["chrome", "runtime", "onUserScriptMessage"] {
        if target.is_null() || target.is_undefined() {
            return Ok(None);
        }
        target = Reflect::get(&target, &field.into())?;
    }
    Ok((!target.is_null() && !target.is_undefined()).then_some(target))
}
/// Read the raw identifier; context selection accepts only native strings.
fn sandbox_id(message: &JsValue) -> Result<JsValue, JsValue> {
    if !message.is_object() {
        return Ok(JsValue::UNDEFINED);
    }
    Reflect::get(message, &"sandboxId".into())
}
/// Admit the actual registered source, independently of origin text.
fn window_message(weak: &Weak<State>, event: &MessageEvent) -> Result<JsValue, JsValue> {
    let Some(state) = weak.upgrade() else {
        return Ok(JsValue::UNDEFINED);
    };
    let message = event.data();
    let id = sandbox_id(&message)?;
    let context = state.window_context(
        id.as_string().as_deref(),
        event.source().map(JsValue::from).as_ref(),
        |frame| frame.content_window().map(JsValue::from),
    );
    let Some(context) = context else {
        return Ok(JsValue::UNDEFINED);
    };
    let slot = context.frame.clone();
    let message_id = Reflect::get(&message, &"messageId".into())?;
    let respond = Box::new(move |response: BrowserSandboxValue| {
        let frame = slot.borrow().clone();
        if let Some(window) = frame.and_then(|frame| frame.content_window()) {
            let reply = Object::new();
            Reflect::set(&reply, &"type".into(), &"runtime-response".into())
                .map_err(|error| diagnostic(&error))?;
            Reflect::set(&reply, &"messageId".into(), &message_id)
                .map_err(|error| diagnostic(&error))?;
            Reflect::set(&reply, &"sandboxId".into(), &id).map_err(|error| diagnostic(&error))?;
            spread(&reply, &response.0).map_err(|error| diagnostic(&error))?;
            window
                .post_message(&reply, "*")
                .map_err(|error| diagnostic(&error))?;
        }
        Ok(())
    });
    Ok(deliver(context, message, Arc::from(respond as Box<dyn Fn(_) -> _>)).into())
}
/// Retain the extension's synchronous admission and response precedence.
fn extension_message(
    weak: &Weak<State>,
    message: JsValue,
    reply: JsValue,
) -> Result<JsValue, JsValue> {
    let Some(state) = weak.upgrade() else {
        return Ok(false.into());
    };
    let id = sandbox_id(&message)?;
    let Some(context) = state.context(id.as_string().as_deref()) else {
        return Ok(false.into());
    };
    let reply: Function = reply.dyn_into()?;
    let respond = Box::new(move |response: BrowserSandboxValue| {
        let value = Object::new();
        spread(&value, &response.0).map_err(|error| diagnostic(&error))?;
        Reflect::set(&value, &"sandboxId".into(), &id).map_err(|error| diagnostic(&error))?;
        reply
            .call1(&JsValue::UNDEFINED, &value)
            .map_err(|error| diagnostic(&error))?;
        Ok(())
    });
    let _promise = deliver(context, message, Arc::from(respond as Box<dyn Fn(_) -> _>));
    Ok(true.into())
}
/// Copy enumerable fields without invoking the target's prototype setters.
fn spread(target: &Object, source: &JsValue) -> Result<(), JsValue> {
    let prototype = Object::get_prototype_of(target);
    Object::set_prototype_of(target, JsValue::NULL.unchecked_ref());
    Object::try_assign(target, source.unchecked_ref())?;
    Object::set_prototype_of(target, &prototype);
    Ok(())
}
/// Preserve callback rejection as an asynchronous error, not a fabricated reply.
fn deliver(
    context: Sandbox,
    message: JsValue,
    respond: SandboxRespond<BrowserSandboxValue>,
) -> Promise {
    future_to_promise(async move {
        context
            .dispatch(BrowserSandboxValue(message), respond)
            .await
            .map_err(|error| {
                let exception = js_sys::Error::new(&error.message);
                if let Some(name) = error.name {
                    exception.set_name(&name);
                }
                JsValue::from(exception)
            })?;
        Ok(JsValue::UNDEFINED)
    })
}
thread_local! {
    /// Shared browser-local runtime message router.
    pub static RUNTIME_MESSAGE_ROUTER: RuntimeMessageRouter = RuntimeMessageRouter::new();
}
