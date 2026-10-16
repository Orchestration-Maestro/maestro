//! Native retained frame and event ownership.
use super::{PrepareHtmlOptions, document, runtime};
use crate::browser::{
    event_subscription::EventSubscription,
    sandbox_value::{BrowserSandboxValue, diagnostic},
};
use crate::components::sandbox::runtime_message_router::MessageConsumer;
use js_sys::{JSON, Object, Reflect};
use maestro_app::DiagnosticErrorInfo;
use maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider;
use maestro_app::browser::components::sandboxed_iframe::SandboxUrlProvider;
use maestro_theme::BrandPresentation;
use std::{cell::RefCell, fmt::Write as _, rc::Rc, sync::Arc};
use wasm_bindgen::{JsValue, closure::Closure};
use web_sys::{HtmlElement, HtmlIFrameElement, MessageEvent};

/// Native provider allocation retained unchanged by the router.
type Provider = Arc<dyn SandboxRuntimeProvider<BrowserSandboxValue>>;
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
    /// Read on each load; absence selects the srcdoc path.
    pub sandbox_url_provider: Option<SandboxUrlProvider>,
    /// Caller-selected native placement owner.
    parent: HtmlElement,
    /// Owned selected-pack CSS for the host-authored error document only.
    error_style: String,
    /// Weakly captured callback state.
    state: Rc<RefCell<Load>>,
    /// Exact native target and callable retained through successful removal.
    subscription: Option<EventSubscription<Callback>>,
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
        Ok(Self {
            sandbox_url_provider: None,
            parent,
            error_style: css,
            state: Rc::default(),
            subscription: None,
        })
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
    /// Replace the retained load, registering before the frame can emit messages.
    /// # Errors
    /// Returns the first preparation or native effect failure; cleanup remains retryable.
    pub fn load_content(
        &mut self,
        sandbox_id: &str,
        html_content: &str,
        providers: Vec<Provider>,
        consumers: Vec<Arc<dyn MessageConsumer<BrowserSandboxValue>>>,
    ) -> Result<(), DiagnosticErrorInfo> {
        self.remove()?;
        self.state.borrow_mut().id = Some(sandbox_id.into());
        let result = self.load(sandbox_id, html_content, providers, consumers);
        if result.is_err() {
            let _cleanup = self.remove();
        }
        result
    }
    /// Detach the subscription, unregister this context and remove its frame.
    /// # Errors
    /// Returns a native detach failure without releasing the retryable registration.
    pub fn remove(&mut self) -> Result<(), DiagnosticErrorInfo> {
        if let Some(subscription) = &self.subscription {
            subscription.detach().map_err(|error| diagnostic(&error))?;
        }
        self.subscription.take();
        let id = self.state.borrow().id.clone();
        if let Some(id) = id {
            crate::RUNTIME_MESSAGE_ROUTER.with(|router| router.unregister_sandbox(&id))?;
        }
        let old = std::mem::take(&mut *self.state.borrow_mut());
        if let Some(frame) = old.frame {
            frame.remove();
        }
        Ok(())
    }
}
impl Drop for SandboxIframe {
    fn drop(&mut self) {
        let _cleanup = self.remove();
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
