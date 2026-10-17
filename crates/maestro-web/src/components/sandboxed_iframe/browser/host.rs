//! Hidden native placement and application trait forwarding.
use super::{BrowserSandboxValue, DiagnosticErrorInfo, SandboxIframe, diagnostic};
use maestro_app::browser::components::sandboxed_iframe::{
    SandboxExecution, SandboxHost, SandboxIframe as ApplicationSandboxIframe,
};
use maestro_app::{BoxFuture, SandboxResult, SandboxUrlProvider};
use wasm_bindgen::JsCast as _;

/// Stateless platform adapter; construction performs no native document effects.
pub struct BrowserSandboxHost;
impl SandboxHost<BrowserSandboxValue> for BrowserSandboxHost {
    fn create_hidden(
        &self,
        url: Option<SandboxUrlProvider>,
    ) -> Result<Box<dyn ApplicationSandboxIframe<BrowserSandboxValue>>, DiagnosticErrorInfo> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| DiagnosticErrorInfo::error("Missing browser document"))?;
        let parent: web_sys::HtmlElement = document
            .create_element("sandbox-iframe")
            .map_err(|error| diagnostic(&error))?
            .dyn_into()
            .map_err(|error| diagnostic(&error))?;
        parent
            .set_attribute("style", "display: none;")
            .map_err(|error| diagnostic(&error))?;
        let body = document
            .body()
            .ok_or_else(|| DiagnosticErrorInfo::error("Missing document body"))?;
        if let Err(error) = body.append_child(&parent) {
            parent.remove();
            return Err(diagnostic(&error));
        }
        Ok(Box::new(SandboxIframe::retain(
            parent,
            String::new(),
            true,
            url,
        )))
    }
}
impl ApplicationSandboxIframe<BrowserSandboxValue> for SandboxIframe {
    fn execute(
        &self,
        request: SandboxExecution<BrowserSandboxValue>,
    ) -> BoxFuture<Result<SandboxResult<BrowserSandboxValue>, DiagnosticErrorInfo>> {
        let execution = Self::execute(self, request, Vec::new(), false);
        Box::pin(async move { execution.await.and_then(super::values::application_result) })
    }
    fn remove(&self) {
        let _cleanup = Self::remove(self);
    }
}
