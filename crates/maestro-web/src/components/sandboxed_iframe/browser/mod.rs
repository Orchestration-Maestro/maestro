//! Native retained frame and event ownership.
use super::{PrepareHtmlOptions, document, runtime};
use crate::browser::{
    event_subscription::EventSubscription,
    sandbox_value::{BrowserSandboxValue, diagnostic},
};
use crate::components::sandbox::runtime_message_router::MessageConsumer;
use js_sys::{JSON, Object, Reflect};
use maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider;
use maestro_app::browser::components::sandboxed_iframe::SandboxUrlProvider;
use maestro_app::{DiagnosticErrorInfo, SharedHandle};
use maestro_theme::BrandPresentation;
use std::{cell::RefCell, fmt::Write as _, rc::Rc};
use wasm_bindgen::{JsValue, closure::Closure};
use web_sys::{HtmlElement, HtmlIFrameElement, MessageEvent};

/// Native provider allocation retained unchanged by the router.
type Provider = SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>;
/// Single native message callable retained by the shared subscription owner.
type Callback = Closure<dyn FnMut(MessageEvent)>;
/// Adapter's one live frame and unsettled URL document.
#[derive(Default)]
struct Load {
    /// Registered sandbox identifier, retained through failed cleanup.
    id: Option<String>,
    /// Current native frame, including the validation document.
    frame: Option<HtmlIFrameElement>,
    /// URL handshake's document, consumed only by an admitted settlement.
    pending: Option<String>,
    /// Whether external links may use the extension tabs API.
    extension: bool,
}
/// Retained HTML sandbox adapter. Explicit removal reports retryable detach failures.
pub struct SandboxIframe {
    /// Read after start callbacks on each mount; absence selects srcdoc.
    pub sandbox_url_provider: Rc<RefCell<Option<SandboxUrlProvider>>>,
    /// The single loading and execution lifecycle owner.
    owner: Rc<Owner>,
}
/// Native resources shared with an outstanding execution future.
struct Owner {
    /// Caller-selected native placement owner.
    parent: HtmlElement,
    /// Whether removal also releases the hidden host's parent.
    owns_parent: bool,
    /// Selected-pack CSS for the loading error document only.
    error_style: String,
    /// Shared URL selection, also exposed by the adapter.
    url: Rc<RefCell<Option<SandboxUrlProvider>>>,
    /// Weakly captured native callback state.
    state: Rc<RefCell<Load>>,
    /// Native callable retained through successful removal.
    subscription: RefCell<Option<EventSubscription<Callback>>>,
    /// Pending execution; callbacks only capture weak owner references.
    execution: RefCell<Option<Rc<execution::Execution>>>,
}
impl SandboxIframe {
    /// Retain a parent and project error-document styling without installing listeners.
    /// # Errors
    /// The current constructor has no fallible native effects.
    pub fn new(
        parent: HtmlElement,
        presentation: &BrandPresentation<'_>,
    ) -> Result<Self, DiagnosticErrorInfo> {
        let mut css = String::new();
        crate::browser::append_brand_properties(&mut css, presentation);
        Ok(Self::retain(parent, css, false, None))
    }
    /// Retain placement without creating another lifecycle.
    fn retain(
        parent: HtmlElement,
        error_style: String,
        owns_parent: bool,
        url: Option<SandboxUrlProvider>,
    ) -> Self {
        let url = Rc::new(RefCell::new(url));
        Self {
            sandbox_url_provider: Rc::clone(&url),
            owner: Rc::new(Owner {
                parent,
                owns_parent,
                error_style,
                url,
                state: Rc::default(),
                subscription: RefCell::new(None),
                execution: RefCell::new(None),
            }),
        }
    }
    /// Prepare without mounting, registration or an implicit console provider.
    /// # Errors
    /// Returns the provider or native serialization diagnostic.
    pub fn prepare_html_document(
        &self,
        sandbox_id: &str,
        user_code: &str,
        providers: &[Provider],
        options: Option<PrepareHtmlOptions>,
    ) -> Result<String, DiagnosticErrorInfo> {
        self.owner
            .prepare_html_document(sandbox_id, user_code, providers, options)
    }
    /// Replace the retained load, registering before the frame can emit messages.
    /// # Errors
    /// Returns the first preparation or native effect failure; cleanup remains retryable.
    pub fn load_content(
        &self,
        sandbox_id: &str,
        html_content: &str,
        providers: Vec<Provider>,
        consumers: Vec<SharedHandle<dyn MessageConsumer<BrowserSandboxValue>>>,
    ) -> Result<(), DiagnosticErrorInfo> {
        self.remove()?;
        self.owner.state.borrow_mut().id = Some(sandbox_id.into());
        let result = self
            .owner
            .load(sandbox_id, html_content, providers, consumers);
        if result.is_err() {
            let _cleanup = self.owner.cleanup();
        }
        result
    }
    /// End outstanding execution and detach this owner's native resources.
    /// # Errors
    /// Returns the first detach failure, retaining failed registrations for retry.
    pub fn remove(&self) -> Result<(), DiagnosticErrorInfo> {
        self.owner.remove()
    }
}
impl Owner {
    /// Abort pending execution before releasing its native resources.
    fn remove(&self) -> Result<(), DiagnosticErrorInfo> {
        let executing = self.execution.borrow().is_some();
        if executing {
            self.finish(Err(DiagnosticErrorInfo::error("Execution aborted")));
        }
        self.cleanup()
    }
    /// Attempt independent releases, retaining failed registrations for retry.
    fn cleanup(&self) -> Result<(), DiagnosticErrorInfo> {
        let subscription = self.subscription.take();
        let mut failure = None;
        if let Some(subscription) = subscription
            && let Err(error) = subscription.detach()
        {
            failure = Some(diagnostic(&error));
            self.subscription.replace(Some(subscription));
        }
        let id = self.state.borrow().id.clone();
        if let Some(id) = id {
            match crate::RUNTIME_MESSAGE_ROUTER.with(|router| router.unregister_sandbox(&id)) {
                Ok(()) => {
                    self.state.borrow_mut().id = None;
                }
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        let (frame, pending) = {
            let mut state = self.state.borrow_mut();
            state.extension = false;
            (state.frame.take(), state.pending.take())
        };
        drop(pending);
        if let Some(frame) = frame {
            frame.remove();
        }
        if self.owns_parent {
            self.parent.remove();
        }
        failure.map_or(Ok(()), Err)
    }
}
impl Drop for SandboxIframe {
    fn drop(&mut self) {
        let _cleanup = self.remove();
    }
}
impl Owner {
    /// Prepare without mounting, registration or an implicit console provider.
    /// # Errors
    /// Returns the provider or native serialization diagnostic.
    pub fn prepare_html_document(
        &self,
        sandbox_id: &str,
        user_code: &str,
        providers: &[Provider],
        options: Option<PrepareHtmlOptions>,
    ) -> Result<String, DiagnosticErrorInfo> {
        let options = options.unwrap_or_default();
        let data = Object::new();
        for provider in providers {
            for (key, value) in provider.get_data()? {
                Reflect::set(&data, &key.into(), &value.0).map_err(|error| diagnostic(&error))?;
            }
        }
        let functions = providers
            .iter()
            .map(|provider| provider.get_runtime())
            .collect::<Result<Vec<_>, _>>()?;
        let mut globals = String::new();
        for key in Object::keys(&data) {
            let value = Reflect::get(&data, &key).map_err(|error| diagnostic(&error))?;
            let value = stringify(&value).map_err(|error| diagnostic(&error))?;
            let key = stringify(&key).map_err(|error| diagnostic(&error))?;
            let _ = writeln!(globals, "window[{key}] = {value};");
        }
        let runtime = runtime::assemble(sandbox_id, &globals, &functions, options.is_standalone)?;
        Ok(document::prepare(user_code, &runtime, options))
    }
}
/// Native compact JSON; undefined is a serialization failure, not a null assignment.
fn stringify(value: &JsValue) -> Result<String, JsValue> {
    JSON::stringify(value)?
        .as_string()
        .ok_or_else(|| js_sys::TypeError::new("Cannot serialize undefined sandbox data").into())
}
mod frame;
mod messages;

mod execution;
mod host;
mod values;
pub use host::BrowserSandboxHost;
